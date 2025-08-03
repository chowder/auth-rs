use std::sync::{mpsc::channel, Arc, Mutex};
use log::error;

use tao::{
    dpi::{LogicalPosition, LogicalSize}, event::{Event, WindowEvent}, event_loop::{ControlFlow, EventLoopBuilder}, window::WindowBuilder
};
use url::Url;
use uuid::Uuid;
use wry::{Rect, WebViewBuilder};

use crate::{client::Client, error::{AuthError, Result}};

async fn handle_auth_redirect(
    client: &Client,
    code: String,
    state: String, 
    options: AuthOptions,
    consent_state: Arc<Mutex<Option<String>>>,
) -> Result<CustomEvent> {
    if state != options.state {
        return Err(AuthError::InvalidResponse("Auth state parameter mismatch - possible CSRF attack".to_string()));
    }
    
    let token_response = client.token(&code, &options.verifier).await?;
    let (consent_url, new_consent_state) = create_consent_url(&token_response.tokens.id_token)?;
    
    if let Ok(mut state_guard) = consent_state.lock() {
        *state_guard = Some(new_consent_state);
    }
    
    Ok(CustomEvent::LoadUrl(consent_url))
}

async fn handle_consent_redirect(
    client: &Client,
    id_token: String,
    state: String,
    consent_state: Arc<Mutex<Option<String>>>,
) -> Result<CustomEvent> {
    let expected_state = consent_state.lock().ok().and_then(|guard| guard.clone());
    match expected_state {
        Some(expected) if expected == state => {
            client.create_session(&id_token).await?;
            Ok(CustomEvent::Close)
        }
        Some(_) => Err(AuthError::InvalidResponse("Consent state parameter mismatch - possible CSRF attack".to_string())),
        None => Err(AuthError::InvalidResponse("No consent state found - possible CSRF attack".to_string())),
    }
}

#[derive(Debug, Clone)]
struct AuthOptions {
    state: String,
    challenge: String,
    verifier: String,
}

impl AuthOptions {
    fn new() -> Result<Self> {
        let state = Uuid::new_v4();
        let code_verify = pkce::code_verifier(43);
        let code_challenge = pkce::code_challenge(&code_verify);
        let verifier = String::from_utf8(code_verify)
            .map_err(|e| AuthError::InvalidResponse(format!("Invalid UTF-8 in code verifier: {e}")))?;

        Ok(Self {
            state: state.to_string(),
            challenge: code_challenge,
            verifier,
        })
    }
}

#[derive(Debug, Clone)]
enum Redirects {
    Auth {
        code: String,
        state: String,
    },
    Consent {
        id_token: String,
        state: String,
    }
}


fn parse_redirect(url: &str) -> Option<Redirects> {
    let parsed_url = Url::parse(url).ok()?;
    
    if let Some(auth_redirect) = try_parse_auth_redirect(&parsed_url) {
        return Some(auth_redirect);
    }
    
    if let Some(consent_redirect) = try_parse_consent_redirect(url) {
        return Some(consent_redirect);
    }
    
    None
}

fn try_parse_auth_redirect(url: &Url) -> Option<Redirects> {
    if url.scheme() != "https" {
        return None;
    }
    
    if url.host_str() != Some("secure.runescape.com") {
        return None;
    }
    
    if url.path() != "/m=weblogin/launcher-redirect" {
        return None;
    }
    
    let code = url.query_pairs().find(|q| q.0 == "code")?.1;
    let state = url.query_pairs().find(|q| q.0 == "state")?.1;
    
    Some(Redirects::Auth { 
        code: code.into_owned(), 
        state: state.into_owned() 
    })
}

fn try_parse_consent_redirect(url: &str) -> Option<Redirects> {
    let url_with_query = url.replace("#", "?");
    let parsed_url = Url::parse(&url_with_query).ok()?;
    
    if parsed_url.host_str() != Some("localhost") {
        return None;
    }
    
    let state = parsed_url.query_pairs().find(|q| q.0 == "state")?.1;
    let id_token = parsed_url.query_pairs().find(|q| q.0 == "id_token")?.1;
    
    Some(Redirects::Consent {
        id_token: id_token.into_owned(),
        state: state.into_owned(),
    })
}

#[derive(Debug)]
enum CustomEvent {
    Close,
    LoadUrl(String),
}

#[derive(Debug)]
enum Message {
    AuthRedirect { code: String, state: String, options: AuthOptions },
    ConsentRedirect { id_token: String, state: String },
}

fn create_auth_url() -> Result<(String, AuthOptions)> {
    let auth_options = AuthOptions::new()?;
    let mut url = Url::parse(crate::env::ORIGIN)?
        .join("/oauth2/auth")?;
    let mut query = url.query_pairs_mut();
    query.append_pair("flow", "launcher");
    query.append_pair("response_type", "code");
    query.append_pair("client_id", crate::env::CLIENT_ID);
    query.append_pair("redirect_uri", crate::env::REDIRECT);
    query.append_pair("code_challenge", &auth_options.challenge);
    query.append_pair("code_challenge_method", "S256");
    query.append_pair("prompt", "login");
    query.append_pair(
        "scope",
        "openid offline gamesso.token.create user.profile.read",
    );
    query.append_pair("state", &auth_options.state);
    drop(query);

    Ok((url.as_str().to_owned(), auth_options))
}

fn create_consent_url(id_token: &str) -> Result<(String, String)> {
    let state = Uuid::new_v4().to_string();
    let nonce = Uuid::new_v4().to_string();
    let mut url = Url::parse(crate::env::ORIGIN)?
        .join("/oauth2/auth")?;
    let mut query = url.query_pairs_mut();
    query.append_pair("id_token_hint", id_token);
    query.append_pair("nonce", &nonce);
    query.append_pair("prompt", "consent");
    query.append_pair("response_type", "id_token code");
    query.append_pair("client_id", "1fddee4e-b100-4f4e-b2b0-097f9088f9d2");
    query.append_pair("redirect_uri", "http://localhost");
    query.append_pair("scope", "openid offline");
    query.append_pair("state", &state);
    drop(query);

    Ok((url.as_str().to_owned(), state))
}


fn spawn_message_handler(
    client: Client,
    rx: std::sync::mpsc::Receiver<Message>,
    consent_state: Arc<Mutex<Option<String>>>,
    proxy: tao::event_loop::EventLoopProxy<CustomEvent>,
) {
    tokio::spawn(async move {
        while let Ok(message) = rx.recv() {
            let result = match message {
                Message::AuthRedirect { code, state, options } => {
                    handle_auth_redirect(&client, code, state, options, consent_state.clone()).await
                }
                Message::ConsentRedirect { id_token, state } => {
                    handle_consent_redirect(&client, id_token, state, consent_state.clone()).await
                }
            };

            match result {
                Ok(event) => {
                    if let Err(e) = proxy.send_event(event) {
                        error!("Failed to send event: {e:?}");
                        let _ = proxy.send_event(CustomEvent::Close);
                        break;
                    }
                }
                Err(e) => {
                    error!("Error during authentication: {e}");
                    let _ = proxy.send_event(CustomEvent::Close);
                    break;
                }
            }
        }

        let _ = proxy.send_event(CustomEvent::Close);
    });
}

pub fn authorize(session_name: Option<String>) -> Result<()> {
    let (tx, rx) = channel::<Message>();
    let consent_state: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let event_loop = EventLoopBuilder::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_title("Authorize")
        .with_inner_size(LogicalSize::new(400.0, 700.0))
        .with_minimizable(false)
        .with_maximizable(false)
        .build(&event_loop)
        .map_err(|e| AuthError::InvalidResponse(format!("Failed to create window: {e}")))?;

    let client = Client::new(session_name);
    spawn_message_handler(client, rx, consent_state, proxy.clone());

    let (auth_url, options) = create_auth_url()?;
    let builder = WebViewBuilder::new()
        .with_navigation_handler(move |navigate_to| {            
            if let Some(redirect) = parse_redirect(&navigate_to) {
                match redirect {
                    Redirects::Auth { code, state } => {
                        if let Err(e) = tx.send(Message::AuthRedirect { 
                            code, 
                            state, 
                            options: options.clone() 
                        }) {
                            error!("Failed to send auth redirect message: {e}");
                        }
                    }
                    Redirects::Consent { id_token, state } => {
                        if let Err(e) = tx.send(Message::ConsentRedirect { 
                            id_token, 
                            state 
                        }) {
                            error!("Failed to send consent redirect message: {e}");
                        }
                    }
                }
                false
            } else {
                true
            }
        })
        .with_clipboard(true)
        .with_bounds(Rect {
            position: LogicalPosition::new(0, 0).into(),
            size: LogicalSize::new(400, 700).into()
        })
        .with_url(auth_url);

    #[cfg(not(target_os = "linux"))]
    let webview = builder.build(&window)
        .map_err(|e| AuthError::WebviewError(format!("{}", e)))?;
    #[cfg(target_os = "linux")]
    let webview = {
        use gtk::prelude::*;
        use wry::WebViewBuilderExtUnix;
        use tao::platform::unix::WindowExtUnix;
        
        let vbox = window.default_vbox().unwrap();
        let fixed = gtk::Fixed::new();
        fixed.show_all();
        vbox.pack_start(&fixed, true, true, 0);
        builder.build_gtk(&fixed).map_err(|e| AuthError::WebviewError(format!("{e}")))?
    };

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::WindowEvent { event: WindowEvent::Resized(size), .. } => {
                webview.set_bounds(Rect {
                    position: LogicalPosition::new(0, 0).into(),
                    size: LogicalSize::new(size.width, size.height).into()
                }).unwrap();
            },
            Event::UserEvent(CustomEvent::Close) => *control_flow = ControlFlow::Exit,
            Event::UserEvent(CustomEvent::LoadUrl(url)) => {
                if let Err(e) = webview.load_url(&url) {
                    error!("Failed to load URL: {e}");
                    *control_flow = ControlFlow::Exit;
                }
            }
            _ => (),
        }
    });
}

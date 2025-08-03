

use std::time::SystemTime;

use keyring::Entry;
use serde::{Deserialize, Serialize};
use crate::error::{AuthError, Result};

#[derive(Serialize, Deserialize)]
struct SessionRequest {
    #[serde(rename = "idToken")]
    id_token: String
}

#[derive(Serialize, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub expires_in: usize,
    pub id_token: String,
    pub refresh_token: String,
    pub scope: String,
    pub token_type: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Account {
    #[serde(rename = "accountId")]
    pub account_id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "userHash")]
    pub user_hash: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Session {
    #[serde(rename = "sessionId")]
    pub session_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct AuthState {
    pub time: SystemTime,
    pub tokens: Tokens
}

struct SessionStore;

impl SessionStore {
    const SERVICE: &'static str = "auth-rs";
    const USER: &'static str = "session";
    
    fn get_entry() -> Result<Entry> {
        Entry::new(Self::SERVICE, Self::USER)
            .map_err(AuthError::from)
    }
    
    fn store(session: &Session) -> Result<()> {
        let entry = Self::get_entry()?;
        let session_json = serde_json::to_string(session)?;
        entry.set_password(&session_json)
            .map_err(AuthError::from)
    }
    
    fn load() -> Result<Option<Session>> {
        let entry = Self::get_entry()?;
        match entry.get_password() {
            Ok(session_json) => {
                let session: Session = serde_json::from_str(&session_json)?;
                Ok(Some(session))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AuthError::from(e))
        }
    }
    
    fn clear() -> Result<()> {
        let entry = Self::get_entry()?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AuthError::from(e))
        }
    }
}

pub struct Client {
    client: reqwest::Client,
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub async fn token(&self, code: &str, verifier: &str) -> Result<AuthState> {
        let url = "https://account.jagex.com/oauth2/token";
        let time = SystemTime::now();
        let response = self.client
            .post(url)
            .form(&[
                ("grant_type", "authorization_code"),
                ("client_id", crate::env::CLIENT_ID),
                ("code", code),
                ("code_verifier", verifier),
                ("redirect_uri", crate::env::REDIRECT),
            ])
            .send()
            .await?;

        let tokens: Tokens = response.json().await?;
        let state = AuthState { time, tokens };
        Ok(state)
    }

    pub async fn create_session(&self, token: &str) -> Result<Session> {
        let url = "https://auth.jagex.com/game-session/v1/sessions";
        let body = SessionRequest { id_token: token.to_owned() };
        let response = self.client.post(url)
            .body(serde_json::to_string(&body)?)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .send()
            .await?;
        let session: Session = response.json().await?;
        SessionStore::store(&session)?;
        Ok(session)
    }

    pub fn session(&self) -> Result<Session> {
        SessionStore::load()?.ok_or(AuthError::SessionNotFound)
    }
    
    pub async fn accounts(&self) -> Result<Vec<Account>> {
        let session = self.session()?;
        let url = "https://auth.jagex.com/game-session/v1/accounts";
        let response = self.client.get(url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {}", session.session_id))
            .send()
            .await?;
        let accounts: Vec<Account> = response.json().await?;
        Ok(accounts)
    }

    pub fn logout(&self) -> Result<()> {
        SessionStore::clear()
    }
}
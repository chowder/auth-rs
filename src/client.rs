

use std::{path::PathBuf, time::SystemTime};

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
    
    fn get_entry(session_name: &Option<String>) -> Result<Entry> {
        let key = match session_name {
            Some(session_name) => format!("named-session-{session_name}"),
            None => "session".to_owned(),
        };
        Entry::new(Self::SERVICE, &key)
            .map_err(AuthError::from)
    }
    
    fn store(session_name: &Option<String>, session: &Session) -> Result<()> {
        let entry = Self::get_entry(session_name)?;
        let session_json = serde_json::to_string(session)?;
        entry.set_password(&session_json)
            .map_err(AuthError::from)
    }
    
    fn load(session_name: &Option<String>) -> Result<Option<Session>> {
        let entry = Self::get_entry(session_name)?;
        match entry.get_password() {
            Ok(session_json) => {
                let session: Session = serde_json::from_str(&session_json)?;
                Ok(Some(session))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AuthError::from(e))
        }
    }
    
    fn clear(session_name: &Option<String>) -> Result<()> {
        let entry = Self::get_entry(session_name)?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AuthError::from(e))
        }
    }
}

pub struct Client {
    session_name: Option<String>,
    client: reqwest::Client,
}


impl Client {
    pub fn new(session_name: Option<String>) -> Self {
        Self {
            session_name,
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
        SessionStore::store(&self.session_name, &session)?;
        self.clear_accounts_cache()?;
        Ok(session)
    }

    pub fn session(&self) -> Result<Session> {
        SessionStore::load(&self.session_name)?.ok_or(AuthError::SessionNotFound)
    }
    
    fn clear_accounts_cache(&self) -> Result<()> {
        let path = match self.accounts_cache_dir() {
            Ok(path) => path,
            Err(AuthError::NoCacheDir) => return Ok(()),
            Err(e) => return Err(e),
        };

        if path.exists() {
            return Ok(std::fs::remove_dir_all(path)?);
        }

        Ok(())
    }

    fn accounts_cache_dir(&self) -> Result<PathBuf> {
        let mut path = dirs::cache_dir().ok_or(AuthError::NoCacheDir)?;
        let key = match &self.session_name {
            Some(session_name) => format!("named-session-{session_name}"),
            None => "session".to_owned(),
        };
        path = path.join("auth-rs");
        path = path.join(key);
        Ok(path)
    }

    fn accounts_cache(&self) -> Result<Vec<Account>> {
        let path = self.accounts_cache_dir()?;
        let path = path.join("accounts.json");

        if !path.exists() {
            return Ok(vec![]);
        }

        let file = std::fs::File::open(path)?;
        let accounts: Vec<Account> = serde_json::from_reader(file)?;
        Ok(accounts)
    }

    fn store_accounts(&self, accounts: &Vec<Account>) -> Result<()> {
        let path = self.accounts_cache_dir()?;

        if !path.exists() {
            std::fs::create_dir_all(&path)?;
        }

        let path = path.join("accounts.json");
        let file = std::fs::File::create(path)?;

        serde_json::to_writer(file, accounts)?;

        Ok(())
    }

    pub async fn accounts(&self, offline: bool, store_offline: bool) -> Result<Vec<Account>> {
        let session = self.session()?;

        if offline {
            return self.accounts_cache();
        }

        let url = "https://auth.jagex.com/game-session/v1/accounts";
        let response = self.client.get(url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {}", session.session_id))
            .send()
            .await?;
        let accounts: Vec<Account> = response.json().await?;

        if store_offline {
            self.store_accounts(&accounts)?;
        }

        Ok(accounts)
    }

    pub fn logout(&self) -> Result<()> {
        SessionStore::clear(&self.session_name)?;
        self.clear_accounts_cache()?;

        Ok(())
    }
}
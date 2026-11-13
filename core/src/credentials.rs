//! API-key storage in the OS credential store. The key never leaves Rust.
use crate::error::{AppError, Result};
use std::sync::Mutex;

pub trait CredentialStore: Send + Sync {
    fn set(&self, secret: &str) -> Result<()>;
    fn get(&self) -> Result<Option<String>>;
    fn delete(&self) -> Result<()>;
}

pub struct KeyringStore {
    service: String,
    user: String,
}

impl KeyringStore {
    pub fn new(service: &str, user: &str) -> Self {
        Self {
            service: service.into(),
            user: user.into(),
        }
    }
    fn entry(&self) -> Result<keyring::Entry> {
        keyring::Entry::new(&self.service, &self.user).map_err(cred_err)
    }
}

fn cred_err(e: keyring::Error) -> AppError {
    AppError::Credential(e.to_string())
}

impl CredentialStore for KeyringStore {
    fn set(&self, secret: &str) -> Result<()> {
        self.entry()?.set_password(secret).map_err(cred_err)
    }
    fn get(&self) -> Result<Option<String>> {
        match self.entry()?.get_password() {
            Ok(s) => Ok(Some(s)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(cred_err(e)),
        }
    }
    fn delete(&self) -> Result<()> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(cred_err(e)),
        }
    }
}

/// In-memory store for tests only.
#[derive(Default)]
pub struct MemoryStore(Mutex<Option<String>>);

impl CredentialStore for MemoryStore {
    fn set(&self, s: &str) -> Result<()> {
        *self.0.lock().unwrap() = Some(s.into());
        Ok(())
    }
    fn get(&self) -> Result<Option<String>> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn delete(&self) -> Result<()> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

/// Store and read back; if the store silently dropped or altered the secret, remove it and fail.
/// Guards against mock/no-op credential backends so live AI is disabled instead of unsafe.
pub fn set_verified(store: &dyn CredentialStore, secret: &str) -> Result<()> {
    store.set(secret)?;
    match store.get() {
        Ok(Some(s)) if s == secret => Ok(()),
        other => {
            let _ = store.delete();
            Err(AppError::Credential(match other {
                Err(e) => e.to_string(),
                _ => "credential store did not persist the key".into(),
            }))
        }
    }
}

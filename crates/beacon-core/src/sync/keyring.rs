// SPDX-License-Identifier: GPL-3.0-or-later

//! The app password, kept in the keyring through `oo7`: the Secret portal
//! inside the Flatpak, the Secret Service outside it. Only the app
//! password is kept, never the real one; the server and user are in the
//! database.

use std::collections::HashMap;

use url::Url;

use super::nextcloud::Credentials;
use crate::error::SyncError;

/// What marks Beacon's item, whichever account it is for.
const SERVICE: (&str, &str) = ("service", "beacon-sync");

fn keyring_error(err: oo7::Error) -> SyncError {
    SyncError::Keyring(err.to_string())
}

async fn open() -> Result<oo7::Keyring, SyncError> {
    let keyring = oo7::Keyring::new().await.map_err(keyring_error)?;
    keyring.unlock().await.map_err(keyring_error)?;
    Ok(keyring)
}

/// Keep the account's app password, in place of any kept before, under
/// `label` (translated by the app; Passwords and Keys shows it).
pub async fn save(credentials: &Credentials, label: &str) -> Result<(), SyncError> {
    let keyring = open().await?;
    keyring
        .delete(&HashMap::from([SERVICE]))
        .await
        .map_err(keyring_error)?;
    let attributes = HashMap::from([
        SERVICE,
        ("server", credentials.server.as_str()),
        ("user", credentials.user.as_str()),
    ]);
    keyring
        .create_item(label, &attributes, credentials.app_password.as_str(), true)
        .await
        .map_err(keyring_error)
}

/// The app password kept for `user` on `server`, if any.
pub async fn load(server: &Url, user: &str) -> Result<Option<Credentials>, SyncError> {
    let keyring = open().await?;
    let attributes = HashMap::from([SERVICE, ("server", server.as_str()), ("user", user)]);
    let items = keyring
        .search_items(&attributes)
        .await
        .map_err(keyring_error)?;
    let Some(item) = items.first() else {
        return Ok(None);
    };
    item.unlock().await.map_err(keyring_error)?;
    let secret = item.secret().await.map_err(keyring_error)?;
    let app_password = String::from_utf8(secret.as_bytes().to_vec())
        .map_err(|_| SyncError::Keyring("the kept password is not text".to_owned()))?;
    Ok(Some(Credentials {
        server: server.clone(),
        user: user.to_owned(),
        app_password,
    }))
}

/// Forget every app password Beacon kept.
pub async fn forget() -> Result<(), SyncError> {
    open()
        .await?
        .delete(&HashMap::from([SERVICE]))
        .await
        .map_err(keyring_error)
}

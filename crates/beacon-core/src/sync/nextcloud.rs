// SPDX-License-Identifier: GPL-3.0-or-later

//! nextcloud-gpodder (`docs/specs/sync.md`, The API), and logging in to
//! Nextcloud: Login Flow v2 through the browser, or a password turned into
//! an app password. Requests use basic auth with the app password.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use reqwest::{Method, RequestBuilder, Response, StatusCode};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use url::Url;

use super::{
    Accepted, ActionUpdate, EpisodeAction, SubscriptionChanges, SubscriptionUpdate, SyncBackend,
};
use crate::error::SyncError;

/// Where nextcloud-gpodder answers, under the server's address.
const API: &str = "index.php/apps/gpoddersync/";

/// How long a browser login's token lasts.
pub const LOGIN_LIFETIME: Duration = Duration::from_secs(20 * 60);

/// A Nextcloud account: the server, the user and an app password.
#[derive(Clone, PartialEq, Eq)]
pub struct Credentials {
    pub server: Url,
    pub user: String,
    pub app_password: String,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the password, even in logs.
        f.debug_struct("Credentials")
            .field("server", &self.server.as_str())
            .field("user", &self.user)
            .finish_non_exhaustive()
    }
}

/// The server address as typed: `https://` is assumed without a scheme,
/// and the path ends with `/`, so a Nextcloud in a folder works.
pub fn server_url(input: &str) -> Result<Url, SyncError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(SyncError::InvalidServer);
    }
    let with_scheme = if input.contains("://") {
        input.to_owned()
    } else {
        format!("https://{input}")
    };
    let mut url = Url::parse(&with_scheme).map_err(|_| SyncError::InvalidServer)?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(SyncError::InvalidServer);
    }
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

fn join(server: &Url, path: &str) -> Result<Url, SyncError> {
    server.join(path).map_err(|_| SyncError::InvalidServer)
}

/// Map a response's status to an error: 401 and 403 are a login the server
/// refused, `missing` what a 404 means here.
fn check(response: Response, missing: SyncError) -> Result<Response, SyncError> {
    match response.status() {
        status if status.is_success() => Ok(response),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(SyncError::Unauthorized),
        StatusCode::NOT_FOUND => Err(missing),
        status => Err(SyncError::Status(status.as_u16())),
    }
}

async fn read<T: DeserializeOwned>(response: Response) -> Result<T, SyncError> {
    let body = response.bytes().await?;
    serde_json::from_slice(&body).map_err(|err| SyncError::Parse(err.to_string()))
}

/// nextcloud-gpodder on one account.
#[derive(Debug, Clone)]
pub struct NextcloudBackend {
    http: reqwest::Client,
    credentials: Credentials,
}

impl NextcloudBackend {
    pub fn new(http: reqwest::Client, credentials: Credentials) -> Self {
        Self { http, credentials }
    }

    pub fn credentials(&self) -> &Credentials {
        &self.credentials
    }

    fn request(
        &self,
        method: Method,
        path: &str,
        since: Option<i64>,
    ) -> Result<RequestBuilder, SyncError> {
        let mut url = join(&self.credentials.server, &format!("{API}{path}"))?;
        if let Some(since) = since {
            url.query_pairs_mut()
                .append_pair("since", &since.to_string());
        }
        Ok(self
            .http
            .request(method, url)
            .basic_auth(&self.credentials.user, Some(&self.credentials.app_password)))
    }

    async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        since: Option<i64>,
    ) -> Result<T, SyncError> {
        let response = self.request(Method::GET, path, since)?.send().await?;
        read(check(response, SyncError::AppMissing)?).await
    }

    async fn post(&self, path: &str, body: Vec<u8>) -> Result<i64, SyncError> {
        let response = self
            .request(Method::POST, path, None)?
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await?;
        let accepted: Accepted = read(check(response, SyncError::AppMissing)?).await?;
        Ok(accepted.timestamp)
    }
}

fn to_json(value: &impl serde::Serialize) -> Vec<u8> {
    serde_json::to_vec(value).expect("the payloads are plain data")
}

impl SyncBackend for NextcloudBackend {
    async fn subscriptions(&self, since: Option<i64>) -> Result<SubscriptionUpdate, SyncError> {
        self.get("subscriptions", since).await
    }

    async fn push_subscriptions(&self, changes: &SubscriptionChanges) -> Result<i64, SyncError> {
        self.post("subscription_change/create", to_json(changes))
            .await
    }

    async fn episode_actions(&self, since: Option<i64>) -> Result<ActionUpdate, SyncError> {
        self.get("episode_action", since).await
    }

    async fn push_episode_actions(&self, actions: &[EpisodeAction]) -> Result<i64, SyncError> {
        self.post("episode_action/create", to_json(&actions)).await
    }
}

/// A browser login under way: open `login` in the browser, then
/// [`wait_for_browser_login`].
#[derive(Debug, Clone)]
pub struct BrowserLogin {
    pub login: Url,
    endpoint: Url,
    token: String,
    /// When the token runs out.
    expires: Instant,
}

#[derive(Deserialize)]
struct LoginFlow {
    poll: LoginPoll,
    login: String,
}

#[derive(Deserialize)]
struct LoginPoll {
    token: String,
    endpoint: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginApproved {
    server: String,
    login_name: String,
    app_password: String,
}

/// Start Login Flow v2 on `server`.
pub async fn start_browser_login(
    http: &reqwest::Client,
    server: &Url,
) -> Result<BrowserLogin, SyncError> {
    let response = http
        .post(join(server, "index.php/login/v2")?)
        .send()
        .await?;
    let flow: LoginFlow = read(check(response, SyncError::Status(404))?).await?;
    let parse = |url: &str| Url::parse(url).map_err(|err| SyncError::Parse(err.to_string()));
    Ok(BrowserLogin {
        login: parse(&flow.login)?,
        endpoint: parse(&flow.poll.endpoint)?,
        token: flow.poll.token,
        expires: Instant::now() + LOGIN_LIFETIME,
    })
}

/// Poll a browser login every `interval` until the user approves it in the
/// browser, `cancel` is set, or the token expires.
pub async fn wait_for_browser_login(
    http: &reqwest::Client,
    login: &BrowserLogin,
    interval: Duration,
    cancel: &AtomicBool,
) -> Result<Credentials, SyncError> {
    let body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("token", &login.token)
        .finish();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(SyncError::Cancelled);
        }
        if Instant::now() > login.expires {
            return Err(SyncError::LoginExpired);
        }
        let response = http
            .post(login.endpoint.clone())
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(body.clone())
            .send()
            .await;
        match response {
            Ok(response) if response.status().is_success() => {
                let approved: LoginApproved = read(response).await?;
                return Ok(Credentials {
                    server: server_url(&approved.server)?,
                    user: approved.login_name,
                    app_password: approved.app_password,
                });
            }
            // Not approved yet.
            Ok(response) if response.status() == StatusCode::NOT_FOUND => {}
            Ok(response) => return Err(SyncError::Status(response.status().as_u16())),
            // A dropped connection while the user is in the browser is
            // worth another try, until the token runs out.
            Err(_) => {}
        }
        tokio::time::sleep(interval).await;
    }
}

#[derive(Deserialize)]
struct Ocs<T> {
    ocs: OcsBody<T>,
}

#[derive(Deserialize)]
struct OcsBody<T> {
    data: T,
}

#[derive(Deserialize)]
struct AppPassword {
    apppassword: String,
}

/// Log in with a password: the server turns it into an app password,
/// which is what Beacon keeps. One that already is an app password (the
/// server answers 403) is kept as it is.
pub async fn log_in_with_password(
    http: &reqwest::Client,
    server: &Url,
    user: &str,
    password: &str,
) -> Result<Credentials, SyncError> {
    let mut url = join(server, "ocs/v2.php/core/getapppassword")?;
    url.query_pairs_mut().append_pair("format", "json");
    let response = http
        .get(url)
        .basic_auth(user, Some(password))
        .header("OCS-APIRequest", "true")
        .send()
        .await?;
    let app_password = if response.status() == StatusCode::FORBIDDEN {
        password.to_owned()
    } else {
        let ocs: Ocs<AppPassword> = read(check(response, SyncError::Status(404))?).await?;
        ocs.ocs.data.apppassword
    };
    Ok(Credentials {
        server: server.clone(),
        user: user.to_owned(),
        app_password,
    })
}

/// Log Out: ask the server to revoke the app password. The caller forgets
/// the account whatever this returns, as Nextcloud's docs advise.
pub async fn revoke(http: &reqwest::Client, credentials: &Credentials) -> Result<(), SyncError> {
    let response = http
        .delete(join(&credentials.server, "ocs/v2.php/core/apppassword")?)
        .basic_auth(&credentials.user, Some(&credentials.app_password))
        .header("OCS-APIRequest", "true")
        .send()
        .await?;
    check(response, SyncError::Status(404)).map(|_| ())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;
    use std::time::{Duration, Instant};

    use httpmock::prelude::*;

    use super::*;

    fn login(server: &MockServer, expires: Instant) -> BrowserLogin {
        BrowserLogin {
            login: Url::parse(&server.url("/login/flow")).unwrap(),
            endpoint: Url::parse(&server.url("/login/v2/poll")).unwrap(),
            token: "t0ken".to_owned(),
            expires,
        }
    }

    #[tokio::test]
    async fn a_browser_login_expires_after_its_token() {
        let server = MockServer::start_async().await;
        let poll = server
            .mock_async(|when, then| {
                when.method(POST).path("/login/v2/poll");
                then.status(404);
            })
            .await;
        let expired = login(&server, Instant::now() + Duration::from_millis(300));
        let result = wait_for_browser_login(
            &reqwest::Client::new(),
            &expired,
            Duration::from_millis(10),
            &AtomicBool::new(false),
        )
        .await;
        assert!(matches!(result, Err(SyncError::LoginExpired)));
        assert!(poll.calls_async().await >= 1, "it polled until then");
    }

    #[tokio::test]
    async fn a_browser_login_stops_on_cancel() {
        let server = MockServer::start_async().await;
        let result = wait_for_browser_login(
            &reqwest::Client::new(),
            &login(&server, Instant::now() + LOGIN_LIFETIME),
            Duration::from_millis(10),
            &AtomicBool::new(true),
        )
        .await;
        assert!(matches!(result, Err(SyncError::Cancelled)));
    }
}

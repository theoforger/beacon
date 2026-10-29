// SPDX-License-Identifier: GPL-3.0-or-later

pub mod clock;
pub mod covers;
pub mod feeds;
pub mod library;

/// User agent for every request Beacon makes.
pub const USER_AGENT: &str = concat!(
    "Beacon/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/theoforger/Beacon)"
);

/// One HTTP client shared by the feed and cover services.
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .expect("the client configuration is static and valid")
}

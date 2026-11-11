// SPDX-License-Identifier: GPL-3.0-or-later

//! Sync with a gpodder-style server (`docs/specs/sync.md`): the payloads
//! the protocol shares, the [`SyncBackend`] a server speaks them through,
//! and the [`service`] that merges them with the library.

pub mod keyring;
pub mod nextcloud;
pub mod service;

use std::future::Future;

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::SyncError;

/// Subscription changes, as both sides send them: feed URLs added and
/// removed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionChanges {
    #[serde(default)]
    pub add: Vec<String>,
    #[serde(default)]
    pub remove: Vec<String>,
}

impl SubscriptionChanges {
    pub fn is_empty(&self) -> bool {
        self.add.is_empty() && self.remove.is_empty()
    }
}

/// The server's subscription changes since a time, and its time now.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct SubscriptionUpdate {
    #[serde(flatten)]
    pub changes: SubscriptionChanges,
    pub timestamp: i64,
}

/// The server's episode actions since a time, and its time now.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct ActionUpdate {
    #[serde(default)]
    pub actions: Vec<EpisodeAction>,
    pub timestamp: i64,
}

/// The time a server gives back for changes it accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub(crate) struct Accepted {
    pub timestamp: i64,
}

/// What an episode action says happened. Beacon sends PLAY only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionKind {
    Play,
    Download,
    Delete,
    New,
    /// Anything else (FLATTR, or a future action).
    Other(String),
}

impl Serialize for ActionKind {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self {
            ActionKind::Play => "PLAY",
            ActionKind::Download => "DOWNLOAD",
            ActionKind::Delete => "DELETE",
            ActionKind::New => "NEW",
            ActionKind::Other(other) => other,
        })
    }
}

impl<'de> Deserialize<'de> for ActionKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // The README mixes `PLAY` and `play`.
        let name = String::deserialize(deserializer)?;
        Ok(match name.to_ascii_uppercase().as_str() {
            "PLAY" => ActionKind::Play,
            "DOWNLOAD" => ActionKind::Download,
            "DELETE" => ActionKind::Delete,
            "NEW" => ActionKind::New,
            _ => ActionKind::Other(name),
        })
    }
}

/// One episode action. `podcast` is the feed URL and `episode` the
/// enclosure URL; times are seconds, `None` where the protocol sends -1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EpisodeAction {
    pub podcast: String,
    pub episode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guid: Option<String>,
    pub action: ActionKind,
    #[serde(serialize_with = "write_time", deserialize_with = "read_time")]
    pub timestamp: DateTime<Utc>,
    #[serde(default, serialize_with = "write_secs", deserialize_with = "read_secs")]
    pub started: Option<u64>,
    #[serde(default, serialize_with = "write_secs", deserialize_with = "read_secs")]
    pub position: Option<u64>,
    #[serde(default, serialize_with = "write_secs", deserialize_with = "read_secs")]
    pub total: Option<u64>,
}

/// A play counts as finished within this many seconds of the end.
pub const FINISHED_WITHIN_SECS: u64 = 5;

/// Whether `action` is a play that reached the end.
pub fn finished(action: &EpisodeAction) -> bool {
    match (action.position, action.total) {
        (Some(position), Some(total)) if total > 0 => position + FINISHED_WITHIN_SECS >= total,
        _ => false,
    }
}

/// UTC without a zone, which every nextcloud-gpodder version reads.
fn write_time<S: Serializer>(time: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&time.format("%Y-%m-%dT%H:%M:%S").to_string())
}

/// UTC with or without a zone (servers since 3.11.0 send one), fractions
/// of a second allowed.
fn read_time<'de, D: Deserializer<'de>>(deserializer: D) -> Result<DateTime<Utc>, D::Error> {
    let text = String::deserialize(deserializer)?;
    parse_time(&text).ok_or_else(|| serde::de::Error::custom(format!("not a time: {text}")))
}

pub(crate) fn parse_time(text: &str) -> Option<DateTime<Utc>> {
    let text = text.trim();
    DateTime::parse_from_rfc3339(text)
        .map(|t| t.with_timezone(&Utc))
        .ok()
        .or_else(|| {
            NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f")
                .ok()
                .map(|t| t.and_utc())
        })
        .or_else(|| {
            DateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f%z")
                .map(|t| t.with_timezone(&Utc))
                .ok()
        })
}

#[allow(
    clippy::ref_option,
    reason = "serde's serialize_with passes a reference"
)]
fn write_secs<S: Serializer>(secs: &Option<u64>, serializer: S) -> Result<S::Ok, S::Error> {
    match secs {
        Some(secs) => serializer.serialize_u64(*secs),
        None => serializer.serialize_i64(-1),
    }
}

fn read_secs<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u64>, D::Error> {
    let secs = Option::<f64>::deserialize(deserializer)?;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok(secs.filter(|s| *s >= 0.0).map(|s| s as u64))
}

/// A server that speaks the gpodder payloads. nextcloud-gpodder's is
/// [`nextcloud::NextcloudBackend`]; a gpodder.net-style one can follow.
pub trait SyncBackend: Send + Sync {
    /// Subscription changes since `since` (all of them for `None`).
    fn subscriptions(
        &self,
        since: Option<i64>,
    ) -> impl Future<Output = Result<SubscriptionUpdate, SyncError>> + Send;

    /// Send subscription changes; returns the server's time.
    fn push_subscriptions(
        &self,
        changes: &SubscriptionChanges,
    ) -> impl Future<Output = Result<i64, SyncError>> + Send;

    /// Episode actions since `since` (all of them for `None`).
    fn episode_actions(
        &self,
        since: Option<i64>,
    ) -> impl Future<Output = Result<ActionUpdate, SyncError>> + Send;

    /// Send episode actions; returns the server's time.
    fn push_episode_actions(
        &self,
        actions: &[EpisodeAction],
    ) -> impl Future<Output = Result<i64, SyncError>> + Send;
}

#[cfg(test)]
mod tests;

// SPDX-License-Identifier: GPL-3.0-or-later

//! Deleting old downloads (`docs/specs/auto-delete.md`): off unless turned
//! on in Preferences; then a download is deleted once it is older than the
//! days chosen, counted from when the episode was played, or, with Only
//! Delete Played Episodes off, from when the download finished. Queued
//! episodes, and the one playing, are never deleted.

use chrono::{DateTime, Duration, Utc};

use crate::model::Episode;

/// The Preferences' Downloads group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CleanupPolicy {
    /// Delete Old Downloads.
    pub enabled: bool,
    /// Delete After, in days.
    pub after_days: u32,
    /// Only Delete Played Episodes.
    pub only_played: bool,
}

impl Default for CleanupPolicy {
    /// As Preferences start: off, a week, played episodes only.
    fn default() -> Self {
        Self {
            enabled: false,
            after_days: 7,
            only_played: true,
        }
    }
}

impl CleanupPolicy {
    /// Whether `episode`'s download is old enough to delete at `now`. Says
    /// nothing of the queue, which the caller keeps.
    pub fn expired(&self, episode: &Episode, now: DateTime<Utc>) -> bool {
        let state = &episode.state;
        if !self.enabled || state.download_path.is_none() {
            return false;
        }
        let since = if self.only_played {
            if !state.played {
                return false;
            }
            state.played_at
        } else {
            state.downloaded_at
        };
        since.is_some_and(|since| now - since >= Duration::days(i64::from(self.after_days)))
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::model::{EpisodeId, EpisodeInfo, EpisodeState, ShowId};

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 10, 12, 0, 0).unwrap()
    }

    /// Downloaded `downloaded` days ago, played `played` days ago if at all.
    fn episode(downloaded: i64, played: Option<i64>) -> Episode {
        Episode {
            id: EpisodeId(1),
            show_id: ShowId(1),
            info: EpisodeInfo::default(),
            state: EpisodeState {
                played: played.is_some(),
                played_at: played.map(|days| now() - Duration::days(days)),
                download_path: Some("/downloads/1.mp3".into()),
                downloaded_at: Some(now() - Duration::days(downloaded)),
                ..Default::default()
            },
            added_at: now(),
        }
    }

    fn on(only_played: bool) -> CleanupPolicy {
        CleanupPolicy {
            enabled: true,
            after_days: 7,
            only_played,
        }
    }

    #[test]
    fn nothing_is_deleted_while_off() {
        assert!(!CleanupPolicy::default().expired(&episode(30, Some(30)), now()));
    }

    #[test]
    fn played_episodes_count_from_when_they_were_played() {
        let policy = on(true);
        assert!(
            policy.expired(&episode(30, Some(7)), now()),
            "a week since played"
        );
        assert!(!policy.expired(&episode(30, Some(6)), now()), "six days");
        assert!(!policy.expired(&episode(30, None), now()), "never played");
    }

    #[test]
    fn otherwise_downloads_count_from_when_they_finished() {
        let policy = on(false);
        assert!(policy.expired(&episode(7, None), now()));
        assert!(!policy.expired(&episode(6, Some(30)), now()));
    }

    #[test]
    fn only_downloads_are_deleted() {
        let mut episode = episode(30, Some(30));
        episode.state.download_path = None;
        assert!(!on(true).expired(&episode, now()));
        assert!(!on(false).expired(&episode, now()));
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later

//! The rules of playback, free of any player: where an episode resumes, how
//! far the skip buttons go, the speeds on offer, when the position is saved,
//! the sleep timer and its fade, and which chapter is playing.

use std::time::Duration;

use crate::model::{Chapter, EpisodeState};

/// Back 10 Seconds.
pub const SKIP_BACK: Duration = Duration::from_secs(10);
/// Forward 30 Seconds.
pub const SKIP_FORWARD: Duration = Duration::from_secs(30);

/// The speeds in the speed menu, as in GNOME Podcasts (`player_rate.ui`).
pub const RATES: [f64; 7] = [0.75, 0.90, 1.00, 1.25, 1.50, 1.75, 2.00];

/// How often the position is saved while playing.
pub const SAVE_EVERY: Duration = Duration::from_secs(5);

/// An episode stopped this close to its end counts as finished: resuming it
/// would only play the last few seconds.
pub const NEAR_END: Duration = Duration::from_secs(10);

/// How long playback fades out when the sleep timer runs out.
pub const FADE: Duration = Duration::from_secs(5);

/// Where an episode starts playing: where it was left, unless it was
/// finished, played, or left in its last seconds.
pub fn resume_position(state: &EpisodeState, duration: Option<Duration>) -> Duration {
    if state.played || duration.is_some_and(|d| state.position + NEAR_END >= d) {
        return Duration::ZERO;
    }
    state.position
}

/// Back 10 Seconds, never before the start.
pub fn skip_back(position: Duration) -> Duration {
    position.saturating_sub(SKIP_BACK)
}

/// Forward 30 Seconds, never past the end when it is known.
pub fn skip_forward(position: Duration, duration: Option<Duration>) -> Duration {
    let target = position + SKIP_FORWARD;
    match duration {
        Some(end) => target.min(end),
        None => target,
    }
}

/// Where a relative seek from another app (media keys, the desktop's
/// media controls) lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekBy {
    To(Duration),
    /// Past the end: as MPRIS asks, this acts like Next.
    PastEnd,
}

/// Seek `offset_micros` from `position`, never before the start; past the
/// end when the length is known is `PastEnd`.
pub fn seek_by(position: Duration, offset_micros: i64, duration: Option<Duration>) -> SeekBy {
    let offset = Duration::from_micros(offset_micros.unsigned_abs());
    let target = if offset_micros < 0 {
        position.saturating_sub(offset)
    } else {
        position + offset
    };
    match duration {
        Some(end) if target > end => SeekBy::PastEnd,
        _ => SeekBy::To(target),
    }
}

/// Where a seek to an absolute position from another app lands: nowhere
/// when it is past the end, as MPRIS asks ("If the Position argument is ...
/// greater than the track length, do nothing").
pub fn seek_to(position_micros: i64, duration: Option<Duration>) -> Option<Duration> {
    let position = Duration::from_micros(u64::try_from(position_micros).ok()?);
    match duration {
        Some(end) if position > end => None,
        _ => Some(position),
    }
}

/// The offered speed closest to `rate`, so a stored or odd value never
/// leaves the speed menu with nothing checked.
pub fn nearest_rate(rate: f64) -> f64 {
    RATES
        .into_iter()
        .min_by(|a, b| (a - rate).abs().total_cmp(&(b - rate).abs()))
        .unwrap_or(1.0)
}

/// Whether the position has moved far enough since it was last saved to be
/// saved again, either way (a seek back counts too).
pub fn should_save(saved: Duration, position: Duration) -> bool {
    position.abs_diff(saved) >= SAVE_EVERY
}

/// Whether playback has reached the end, given the player's last position.
pub fn is_finished(position: Duration, duration: Option<Duration>) -> bool {
    duration.is_some_and(|d| !d.is_zero() && position + NEAR_END >= d)
}

/// The sleep timer's choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SleepTimer {
    #[default]
    Off,
    /// Pause this many minutes after it is chosen.
    Minutes(u32),
    /// Pause when the current episode ends, instead of going on to the next.
    EndOfEpisode,
}

impl SleepTimer {
    /// The choices in the sleep timer menu, in order.
    pub const CHOICES: [SleepTimer; 6] = [
        SleepTimer::Off,
        SleepTimer::Minutes(15),
        SleepTimer::Minutes(30),
        SleepTimer::Minutes(45),
        SleepTimer::Minutes(60),
        SleepTimer::EndOfEpisode,
    ];

    /// How long until it runs out, for a timer of minutes.
    pub fn length(self) -> Option<Duration> {
        match self {
            SleepTimer::Minutes(minutes) => Some(Duration::from_secs(u64::from(minutes) * 60)),
            SleepTimer::Off | SleepTimer::EndOfEpisode => None,
        }
    }
}

/// The volume `elapsed` into the fade from `volume`: it falls along a curve
/// that sounds even to the ear (the square of the remaining fraction), and
/// reaches silence at `FADE`.
pub fn fade_volume(volume: f64, elapsed: Duration) -> f64 {
    let left = 1.0 - (elapsed.as_secs_f64() / FADE.as_secs_f64()).clamp(0.0, 1.0);
    volume * left * left
}

/// The chapter playing at `position`: the last to start at or before it.
/// Chapters are in the order they start.
pub fn current_chapter(chapters: &[Chapter], position: Duration) -> Option<usize> {
    chapters
        .iter()
        .rposition(|chapter| chapter.start <= position)
}

#[cfg(test)]
mod tests {
    #[test]
    fn relative_seeks_stop_at_the_start_and_go_next_past_the_end() {
        let at = Duration::from_secs(20);
        let end = Some(Duration::from_secs(60));
        assert_eq!(
            seek_by(at, 10_000_000, end),
            SeekBy::To(Duration::from_secs(30))
        );
        assert_eq!(seek_by(at, -30_000_000, end), SeekBy::To(Duration::ZERO));
        assert_eq!(seek_by(at, 50_000_000, end), SeekBy::PastEnd);
        // Unknown length: no end to pass.
        assert_eq!(
            seek_by(at, 50_000_000, None),
            SeekBy::To(Duration::from_secs(70))
        );
    }

    #[test]
    fn absolute_seeks_outside_the_episode_do_nothing() {
        let end = Some(Duration::from_secs(60));
        assert_eq!(seek_to(5_000_000, end), Some(Duration::from_secs(5)));
        assert_eq!(seek_to(-1, end), None);
        assert_eq!(seek_to(61_000_000, end), None);
        assert_eq!(seek_to(61_000_000, None), Some(Duration::from_secs(61)));
    }

    use super::*;

    fn secs(s: u64) -> Duration {
        Duration::from_secs(s)
    }

    fn state(position: u64, played: bool) -> EpisodeState {
        EpisodeState {
            position: secs(position),
            played,
            ..Default::default()
        }
    }

    #[test]
    fn episodes_resume_where_they_were_left() {
        assert_eq!(
            resume_position(&state(754, false), Some(secs(3000))),
            secs(754)
        );
        assert_eq!(resume_position(&state(754, false), None), secs(754));
        assert_eq!(resume_position(&state(0, false), Some(secs(3000))), secs(0));
    }

    #[test]
    fn played_or_nearly_finished_episodes_start_over() {
        assert_eq!(
            resume_position(&state(754, true), Some(secs(3000))),
            secs(0)
        );
        assert_eq!(
            resume_position(&state(2995, false), Some(secs(3000))),
            secs(0)
        );
        assert_eq!(
            resume_position(&state(2989, false), Some(secs(3000))),
            secs(2989)
        );
    }

    #[test]
    fn skips_stay_inside_the_episode() {
        assert_eq!(skip_back(secs(95)), secs(85));
        assert_eq!(skip_back(secs(4)), secs(0));
        assert_eq!(skip_forward(secs(95), Some(secs(3000))), secs(125));
        assert_eq!(skip_forward(secs(2990), Some(secs(3000))), secs(3000));
        assert_eq!(skip_forward(secs(2990), None), secs(3020));
    }

    #[test]
    fn speeds_snap_to_the_menu() {
        assert_eq!(nearest_rate(1.0), 1.0);
        assert_eq!(nearest_rate(1.3), 1.25);
        assert_eq!(nearest_rate(0.1), 0.75);
        assert_eq!(nearest_rate(9.0), 2.0);
        assert_eq!(nearest_rate(f64::NAN), 0.75);
    }

    #[test]
    fn the_position_is_saved_every_few_seconds() {
        assert!(!should_save(secs(100), secs(104)));
        assert!(should_save(secs(100), secs(105)));
        assert!(should_save(secs(100), secs(40)));
    }

    #[test]
    fn the_end_is_near_the_duration() {
        assert!(is_finished(secs(2991), Some(secs(3000))));
        assert!(!is_finished(secs(2989), Some(secs(3000))));
        assert!(!is_finished(secs(2991), None));
        assert!(!is_finished(secs(0), Some(secs(0))));
    }

    #[test]
    fn the_sleep_timer_offers_the_settled_lengths() {
        let lengths: Vec<_> = SleepTimer::CHOICES.iter().map(|c| c.length()).collect();
        assert_eq!(
            lengths,
            [
                None,
                Some(secs(900)),
                Some(secs(1800)),
                Some(secs(2700)),
                Some(secs(3600)),
                None
            ]
        );
        assert_eq!(SleepTimer::CHOICES[5], SleepTimer::EndOfEpisode);
    }

    #[test]
    fn the_fade_falls_to_silence() {
        assert_eq!(fade_volume(0.8, secs(0)), 0.8);
        assert!((fade_volume(0.8, Duration::from_millis(2500)) - 0.2).abs() < 1e-9);
        assert_eq!(fade_volume(0.8, secs(5)), 0.0);
        assert_eq!(fade_volume(0.8, secs(9)), 0.0);
    }

    #[test]
    fn the_current_chapter_is_the_last_started() {
        let chapter = |start| Chapter {
            start: secs(start),
            title: String::new(),
            url: None,
            image_url: None,
        };
        let chapters = [chapter(0), chapter(60), chapter(300)];
        assert_eq!(current_chapter(&chapters, secs(0)), Some(0));
        assert_eq!(current_chapter(&chapters, secs(59)), Some(0));
        assert_eq!(current_chapter(&chapters, secs(60)), Some(1));
        assert_eq!(current_chapter(&chapters, secs(4000)), Some(2));
        assert_eq!(current_chapter(&[chapter(30)], secs(10)), None);
        assert_eq!(current_chapter(&[], secs(10)), None);
    }
}

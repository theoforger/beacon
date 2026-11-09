// SPDX-License-Identifier: GPL-3.0-or-later

//! Translatable text for episode subtitles and headers.

use std::time::Duration;

use beacon_core::browse::{RecencyGroup, WeekdayName};
use beacon_core::model::{Episode, Season};
use chrono::{DateTime, Local, Utc};
use gettextrs::gettext;
use gtk::glib;

/// "S3 E2 • Oct 27 • 52 min", leaving out what is unknown. Without the
/// show's name, which a row's cover shows where it matters (asked by the
/// user 2026-11-08).
pub fn subtitle(episode: &Episode) -> String {
    let mut parts = Vec::new();
    if let Some(numbers) = season_and_episode(episode) {
        parts.push(numbers);
    }
    if let Some(published) = episode.info.published {
        parts.push(date(published));
    }
    if let Some(duration) = episode.info.duration {
        parts.push(length(duration));
    }
    // Translators: separates the parts of an episode's subtitle; a bullet,
    // with a space either side.
    parts.join(&gettext(" • "))
}

/// A position or length on a clock: "18:42", or "1:02:05" past an hour.
pub fn clock(duration: Duration) -> String {
    let secs = duration.as_secs();
    let (hours, minutes, seconds) = (secs / 3600, secs / 60 % 60, secs % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// A playback speed as the speed menu shows it, as in GNOME Podcasts:
/// "1.25×".
pub fn rate(rate: f64) -> String {
    // Translators: a playback speed, as in "1.25×".
    gettext("{rate}×").replace("{rate}", &format!("{rate:.2}"))
}

/// A subtitle as a screen reader should say it: the bullets between its
/// parts are read aloud ("bullet"), so they become pauses.
pub fn spoken(subtitle: &str) -> String {
    // Translators: separates the parts of an episode's subtitle when a
    // screen reader reads it; a pause, not a symbol.
    subtitle.replace(&gettext(" • "), &gettext(", "))
}

/// The details pane's subtitle: "Season 3, Episode 2 • Oct 27 • 52 min".
pub fn long_subtitle(episode: &Episode) -> String {
    let number = episode.info.number.map(number);
    let numbers = match (&episode.info.season, number) {
        (Some(season), Some(n)) => Some(
            // Translators: as in "Season 3, Episode 2".
            gettext("Season {season}, Episode {episode}")
                .replace("{season}", &season.number.to_string())
                .replace("{episode}", &n),
        ),
        (None, Some(n)) => Some(gettext("Episode {episode}").replace("{episode}", &n)),
        (Some(season), None) => {
            Some(gettext("Season {number}").replace("{number}", &season.number.to_string()))
        }
        (None, None) => None,
    };
    let mut parts: Vec<String> = numbers.into_iter().collect();
    if let Some(published) = episode.info.published {
        parts.push(date(published));
    }
    if let Some(duration) = episode.info.duration {
        parts.push(length(duration));
    }
    parts.join(&gettext(" • "))
}

fn season_and_episode(episode: &Episode) -> Option<String> {
    let number = episode.info.number.map(number);
    match (&episode.info.season, number) {
        (Some(season), Some(n)) => Some(
            // Translators: season and episode numbers, as in "S3 E2".
            gettext("S{season} E{episode}")
                .replace("{season}", &season.number.to_string())
                .replace("{episode}", &n),
        ),
        (None, Some(n)) => Some(
            // Translators: an episode number without a season, as in "E12".
            gettext("E{episode}").replace("{episode}", &n),
        ),
        (Some(season), None) => Some(
            // Translators: a season number without an episode, as in "S3".
            gettext("S{season}").replace("{season}", &season.number.to_string()),
        ),
        (None, None) => None,
    }
}

fn number(n: f32) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// "Oct 27", or "Oct 27, 2025" outside the current year.
pub fn date(published: DateTime<Utc>) -> String {
    let local = published.with_timezone(&Local);
    let this_year = local.format("%Y").to_string() == Local::now().format("%Y").to_string();
    let format = if this_year {
        // Translators: a date in the current year, in GLib's strftime
        // format, as in "Oct 27". Reorder freely.
        gettext("%b %-d")
    } else {
        // Translators: a date in another year, as in "Oct 27, 2025".
        gettext("%b %-d, %Y")
    };
    glib::DateTime::from_unix_local(published.timestamp())
        .and_then(|d| d.format(&format))
        .map(String::from)
        .unwrap_or_default()
}

/// "52 min", or "1 h 6 min".
pub fn length(duration: Duration) -> String {
    let minutes = (duration.as_secs() + 30) / 60;
    let (hours, minutes) = (minutes / 60, minutes % 60);
    if hours == 0 {
        // Translators: an episode length under an hour, as in "52 min".
        gettext("{minutes} min").replace("{minutes}", &minutes.max(1).to_string())
    } else {
        // Translators: an episode length of an hour or more, as in "1 h 6 min".
        gettext("{hours} h {minutes} min")
            .replace("{hours}", &hours.to_string())
            .replace("{minutes}", &minutes.to_string())
    }
}

/// The header above a season on a podcast page.
pub fn season_title(season: Option<&Season>) -> String {
    match season {
        Some(Season {
            number,
            name: Some(name),
        }) => {
            // Translators: a season header with its name, as in "Season 3: Deserts".
            gettext("Season {number}: {name}")
                .replace("{number}", &number.to_string())
                .replace("{name}", name)
        }
        Some(Season { number, name: None }) => {
            gettext("Season {number}").replace("{number}", &number.to_string())
        }
        // Episodes without a season tag in a show that has seasons.
        None => gettext("Other Episodes"),
    }
}

/// The header above a recency group in New.
pub fn recency_title(group: RecencyGroup) -> String {
    match group {
        RecencyGroup::Today => gettext("Today"),
        RecencyGroup::Yesterday => gettext("Yesterday"),
        RecencyGroup::Weekday(key) => match key.weekday {
            WeekdayName::Monday => gettext("Monday"),
            WeekdayName::Tuesday => gettext("Tuesday"),
            WeekdayName::Wednesday => gettext("Wednesday"),
            WeekdayName::Thursday => gettext("Thursday"),
            WeekdayName::Friday => gettext("Friday"),
            WeekdayName::Saturday => gettext("Saturday"),
            WeekdayName::Sunday => gettext("Sunday"),
        },
        RecencyGroup::LastWeek => gettext("Last Week"),
        RecencyGroup::EarlierInTheMonth => gettext("Earlier in the Month"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clocks_show_hours_only_past_an_hour() {
        assert_eq!(clock(Duration::from_secs(0)), "0:00");
        assert_eq!(clock(Duration::from_secs(18 * 60 + 42)), "18:42");
        assert_eq!(clock(Duration::from_secs(3725)), "1:02:05");
        assert_eq!(rate(1.0), "1.00×");
        assert_eq!(rate(0.75), "0.75×");
    }

    #[test]
    fn subtitles_are_spoken_without_their_bullets() {
        assert_eq!(spoken("S3 E2 • Oct 27 • 52 min"), "S3 E2, Oct 27, 52 min");
    }

    #[test]
    fn lengths_round_to_minutes() {
        assert_eq!(length(Duration::from_secs(52 * 60)), "52 min");
        assert_eq!(length(Duration::from_secs(20)), "1 min");
        assert_eq!(length(Duration::from_secs(3600 + 6 * 60 + 29)), "1 h 6 min");
        assert_eq!(length(Duration::from_secs(3600 * 2)), "2 h 0 min");
    }

    #[test]
    fn episode_numbers_drop_needless_decimals() {
        assert_eq!(number(5.0), "5");
        assert_eq!(number(5.5), "5.5");
    }
}

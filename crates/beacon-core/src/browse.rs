// SPDX-License-Identifier: GPL-3.0-or-later

//! How the library views arrange episodes: recency groups for New, seasons
//! for a podcast page, and sorting. Pure functions, so the rules are tested
//! without a database or a display.

use std::cmp::Ordering;

use chrono::{Datelike, NaiveDate, Weekday};

use crate::model::{Episode, Season, SortOrder};

/// The New view's groups, newest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RecencyGroup {
    Today,
    Yesterday,
    /// Two to six days ago, named by weekday.
    Weekday(WeekdayKey),
    /// Seven to thirteen days ago.
    LastWeek,
    /// Fourteen to thirty days ago.
    EarlierInTheMonth,
}

/// Orders weekday groups by how recent they are, not by calendar order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WeekdayKey {
    pub days_ago: u8,
    pub weekday: WeekdayName,
}

/// `chrono::Weekday` without its ordering quirks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WeekdayName {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl From<Weekday> for WeekdayName {
    fn from(day: Weekday) -> Self {
        match day {
            Weekday::Mon => WeekdayName::Monday,
            Weekday::Tue => WeekdayName::Tuesday,
            Weekday::Wed => WeekdayName::Wednesday,
            Weekday::Thu => WeekdayName::Thursday,
            Weekday::Fri => WeekdayName::Friday,
            Weekday::Sat => WeekdayName::Saturday,
            Weekday::Sun => WeekdayName::Sunday,
        }
    }
}

/// The oldest episodes the New view shows, in days before today.
pub const NEW_VIEW_DAYS: i64 = 30;

/// Which New group an episode published on `published` (a local date)
/// belongs to, seen from `today`. `None` if it is older than the New view
/// reaches. Dates in the future count as today.
pub fn recency_group(published: NaiveDate, today: NaiveDate) -> Option<RecencyGroup> {
    let days_ago = (today - published).num_days().max(0);
    Some(match days_ago {
        0 => RecencyGroup::Today,
        1 => RecencyGroup::Yesterday,
        2..=6 => RecencyGroup::Weekday(WeekdayKey {
            days_ago: days_ago as u8,
            weekday: published.weekday().into(),
        }),
        7..=13 => RecencyGroup::LastWeek,
        14..=NEW_VIEW_DAYS => RecencyGroup::EarlierInTheMonth,
        _ => return None,
    })
}

/// Sort episodes by publication date, keeping undated ones last either way.
pub fn sort_episodes(episodes: &mut [Episode], order: SortOrder) {
    episodes.sort_by(|a, b| compare(a, b, order));
}

fn compare(a: &Episode, b: &Episode, order: SortOrder) -> Ordering {
    let by_date = match (a.info.published, b.info.published) {
        (Some(x), Some(y)) => x.cmp(&y).then(a.id.cmp(&b.id)),
        (Some(_), None) => return Ordering::Less,
        (None, Some(_)) => return Ordering::Greater,
        (None, None) => a.id.cmp(&b.id),
    };
    match order {
        SortOrder::OldestFirst => by_date,
        SortOrder::NewestFirst => by_date.reverse(),
    }
}

/// One collapsible season on a podcast page.
#[derive(Debug, Clone, PartialEq)]
pub struct SeasonGroup {
    /// `None` gathers episodes without a season tag in a show that has
    /// seasons.
    pub season: Option<Season>,
    pub episodes: Vec<Episode>,
}

/// Group a show's episodes by season, ordered to match `order` (the newest
/// season first for newest first). Episodes without a season tag come last.
/// A show with no season tags at all gives `None`: its page is a flat list.
pub fn group_by_season(mut episodes: Vec<Episode>, order: SortOrder) -> Option<Vec<SeasonGroup>> {
    if episodes.iter().all(|e| e.info.season.is_none()) {
        return None;
    }
    sort_episodes(&mut episodes, order);

    let mut groups: Vec<SeasonGroup> = Vec::new();
    for episode in episodes {
        let number = episode.info.season.as_ref().map(|s| s.number);
        match groups
            .iter_mut()
            .find(|g| g.season.as_ref().map(|s| s.number) == number)
        {
            Some(group) => {
                // The first named episode names the season.
                if let (Some(season), Some(name)) = (
                    group.season.as_mut(),
                    episode.info.season.as_ref().and_then(|s| s.name.clone()),
                ) && season.name.is_none()
                {
                    season.name = Some(name);
                }
                group.episodes.push(episode);
            }
            None => groups.push(SeasonGroup {
                season: episode.info.season.clone(),
                episodes: vec![episode],
            }),
        }
    }

    groups.sort_by(|a, b| match (&a.season, &b.season) {
        (Some(x), Some(y)) => match order {
            SortOrder::NewestFirst => y.number.cmp(&x.number),
            SortOrder::OldestFirst => x.number.cmp(&y.number),
        },
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    });
    Some(groups)
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::model::{EpisodeId, EpisodeInfo, EpisodeState, ShowId};

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn episode(id: i64, day: Option<u32>, season: Option<u32>) -> Episode {
        Episode {
            id: EpisodeId(id),
            show_id: ShowId(1),
            info: EpisodeInfo {
                guid: id.to_string(),
                published: day.map(|d| Utc.with_ymd_and_hms(2026, 10, d, 8, 0, 0).unwrap()),
                season: season.map(|number| Season { number, name: None }),
                ..Default::default()
            },
            state: EpisodeState::default(),
            added_at: Utc::now(),
        }
    }

    #[test]
    fn recency_groups() {
        let today = date(2026, 10, 29); // a Thursday
        assert_eq!(recency_group(today, today), Some(RecencyGroup::Today));
        assert_eq!(
            recency_group(date(2026, 10, 30), today),
            Some(RecencyGroup::Today)
        );
        assert_eq!(
            recency_group(date(2026, 10, 28), today),
            Some(RecencyGroup::Yesterday)
        );
        assert_eq!(
            recency_group(date(2026, 10, 27), today),
            Some(RecencyGroup::Weekday(WeekdayKey {
                days_ago: 2,
                weekday: WeekdayName::Tuesday
            }))
        );
        assert_eq!(
            recency_group(date(2026, 10, 23), today),
            Some(RecencyGroup::Weekday(WeekdayKey {
                days_ago: 6,
                weekday: WeekdayName::Friday
            }))
        );
        assert_eq!(
            recency_group(date(2026, 10, 22), today),
            Some(RecencyGroup::LastWeek)
        );
        assert_eq!(
            recency_group(date(2026, 10, 16), today),
            Some(RecencyGroup::LastWeek)
        );
        assert_eq!(
            recency_group(date(2026, 10, 15), today),
            Some(RecencyGroup::EarlierInTheMonth)
        );
        assert_eq!(
            recency_group(date(2026, 9, 29), today),
            Some(RecencyGroup::EarlierInTheMonth)
        );
        assert_eq!(recency_group(date(2026, 9, 28), today), None);
    }

    #[test]
    fn recency_groups_order_newest_first() {
        let today = date(2026, 10, 29);
        let mut groups: Vec<_> = [15, 28, 29, 27, 22, 24]
            .iter()
            .filter_map(|&d| recency_group(date(2026, 10, d), today))
            .collect();
        groups.sort();
        assert_eq!(groups[0], RecencyGroup::Today);
        assert_eq!(groups[1], RecencyGroup::Yesterday);
        assert!(matches!(
            groups[2],
            RecencyGroup::Weekday(WeekdayKey { days_ago: 2, .. })
        ));
        assert!(matches!(
            groups[3],
            RecencyGroup::Weekday(WeekdayKey { days_ago: 5, .. })
        ));
        assert_eq!(groups[4], RecencyGroup::LastWeek);
        assert_eq!(groups[5], RecencyGroup::EarlierInTheMonth);
    }

    #[test]
    fn sorting_keeps_undated_episodes_last() {
        let mut episodes = vec![
            episode(1, Some(3), None),
            episode(2, None, None),
            episode(3, Some(9), None),
        ];
        sort_episodes(&mut episodes, SortOrder::NewestFirst);
        assert_eq!(
            episodes.iter().map(|e| e.id.0).collect::<Vec<_>>(),
            [3, 1, 2]
        );
        sort_episodes(&mut episodes, SortOrder::OldestFirst);
        assert_eq!(
            episodes.iter().map(|e| e.id.0).collect::<Vec<_>>(),
            [1, 3, 2]
        );
    }

    #[test]
    fn shows_without_season_tags_are_flat() {
        let episodes = vec![episode(1, Some(1), None), episode(2, Some(2), None)];
        assert_eq!(group_by_season(episodes, SortOrder::NewestFirst), None);
    }

    #[test]
    fn seasons_follow_the_sort_order_and_untagged_episodes_come_last() {
        let episodes = vec![
            episode(1, Some(1), Some(1)),
            episode(2, Some(5), Some(2)),
            episode(3, Some(6), Some(2)),
            episode(4, Some(7), None),
            episode(5, Some(2), Some(1)),
        ];
        let newest = group_by_season(episodes.clone(), SortOrder::NewestFirst).unwrap();
        let summary = |groups: &[SeasonGroup]| {
            groups
                .iter()
                .map(|g| {
                    (
                        g.season.as_ref().map(|s| s.number),
                        g.episodes.iter().map(|e| e.id.0).collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            summary(&newest),
            [
                (Some(2), vec![3, 2]),
                (Some(1), vec![5, 1]),
                (None, vec![4])
            ]
        );
        let oldest = group_by_season(episodes, SortOrder::OldestFirst).unwrap();
        assert_eq!(
            summary(&oldest),
            [
                (Some(1), vec![1, 5]),
                (Some(2), vec![2, 3]),
                (None, vec![4])
            ]
        );
    }

    #[test]
    fn a_season_takes_its_name_from_any_episode() {
        let mut named = episode(2, Some(2), Some(1));
        named.info.season = Some(Season {
            number: 1,
            name: Some("Deserts".into()),
        });
        let groups = group_by_season(
            vec![episode(1, Some(3), Some(1)), named],
            SortOrder::NewestFirst,
        )
        .unwrap();
        assert_eq!(
            groups[0].season.as_ref().unwrap().name.as_deref(),
            Some("Deserts")
        );
    }

    #[test]
    fn new_and_partly_played_rules() {
        let mut state = EpisodeState::default();
        assert!(state.is_new());
        state.seen = true;
        assert!(!state.is_new(), "viewing the details clears the dot");
        state.seen = false;
        state.played = true;
        assert!(!state.is_new(), "played episodes are not new");
        state.played = false;
        state.position = std::time::Duration::from_secs(60);
        assert!(state.is_partly_played());
        state.played = true;
        assert!(!state.is_partly_played());
    }
}

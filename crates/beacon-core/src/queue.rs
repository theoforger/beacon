// SPDX-License-Identifier: GPL-3.0-or-later

//! What plays next, as a music app does it (the mockup's `autoNext` and
//! `advance`): the episodes queued by hand first, then the rest of the list
//! the playing episode was started from. Starting an episode takes it off
//! the queue, so the playing one is never in it.

use crate::model::EpisodeId;

/// The list an episode was started from, whose rest plays after the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    New,
    Show(crate::model::ShowId),
    Favorites,
    Downloaded,
}

impl Source {
    /// The source as a settings value: "new", "show:12", "favorites",
    /// "downloaded".
    pub fn key(self) -> String {
        match self {
            Source::New => "new".into(),
            Source::Show(id) => format!("show:{}", id.0),
            Source::Favorites => "favorites".into(),
            Source::Downloaded => "downloaded".into(),
        }
    }

    /// The source a settings value names, if any.
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "new" => Some(Source::New),
            "favorites" => Some(Source::Favorites),
            "downloaded" => Some(Source::Downloaded),
            _ => key
                .strip_prefix("show:")?
                .parse()
                .ok()
                .map(|id| Source::Show(crate::model::ShowId(id))),
        }
    }
}

/// What follows the playing episode.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpNext {
    /// The manual queue, in order.
    pub queued: Vec<EpisodeId>,
    /// The episodes after the playing one in its source list, leaving out
    /// the queued ones; the whole list when the playing one is no longer in
    /// it.
    pub from_source: Vec<EpisodeId>,
}

impl UpNext {
    /// The episode that plays when the playing one ends, or on Next.
    pub fn first(&self) -> Option<EpisodeId> {
        self.queued.first().or(self.from_source.first()).copied()
    }
}

/// What follows `current`, given the queue and the ids of its source list
/// in the order that list shows them.
pub fn up_next(queue: &[EpisodeId], source: &[EpisodeId], current: Option<EpisodeId>) -> UpNext {
    let queued: Vec<_> = queue
        .iter()
        .copied()
        .filter(|id| Some(*id) != current)
        .collect();
    let rest = match current.and_then(|c| source.iter().position(|id| *id == c)) {
        Some(at) => &source[at + 1..],
        None => source,
    };
    let from_source = rest
        .iter()
        .copied()
        .filter(|id| Some(*id) != current && !queued.contains(id))
        .collect();
    UpNext {
        queued,
        from_source,
    }
}

/// `queue` with `id` moved to `index` (clamped to the end); unchanged when
/// `id` is not queued.
pub fn moved(queue: &[EpisodeId], id: EpisodeId, index: usize) -> Vec<EpisodeId> {
    let mut order = queue.to_vec();
    if let Some(from) = order.iter().position(|q| *q == id) {
        let item = order.remove(from);
        order.insert(index.min(order.len()), item);
    }
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(ids: &[i64]) -> Vec<EpisodeId> {
        ids.iter().copied().map(EpisodeId).collect()
    }

    #[test]
    fn the_queue_comes_first_then_the_rest_of_the_list() {
        let next = up_next(&ids(&[9, 4]), &ids(&[1, 2, 3, 4, 5]), Some(EpisodeId(2)));
        assert_eq!(next.queued, ids(&[9, 4]));
        // After 2 in its list, leaving out 4, which is queued.
        assert_eq!(next.from_source, ids(&[3, 5]));
        assert_eq!(next.first(), Some(EpisodeId(9)));
    }

    #[test]
    fn with_an_empty_queue_the_list_carries_on() {
        let next = up_next(&[], &ids(&[1, 2, 3]), Some(EpisodeId(1)));
        assert_eq!(next.first(), Some(EpisodeId(2)));
        let last = up_next(&[], &ids(&[1, 2, 3]), Some(EpisodeId(3)));
        assert_eq!(last.first(), None, "the end of the list");
    }

    #[test]
    fn an_episode_gone_from_its_list_continues_from_the_top() {
        // Filtered out since it started, as the mockup does.
        let next = up_next(&[], &ids(&[1, 2]), Some(EpisodeId(7)));
        assert_eq!(next.from_source, ids(&[1, 2]));
    }

    #[test]
    fn the_playing_episode_is_never_next() {
        let next = up_next(&ids(&[2, 3]), &ids(&[2, 3]), Some(EpisodeId(2)));
        assert_eq!(next.queued, ids(&[3]));
        assert!(next.from_source.is_empty());
    }

    #[test]
    fn sources_round_trip_through_settings() {
        for source in [
            Source::New,
            Source::Show(crate::model::ShowId(12)),
            Source::Favorites,
            Source::Downloaded,
        ] {
            assert_eq!(Source::from_key(&source.key()), Some(source));
        }
        assert_eq!(Source::from_key(""), None);
        assert_eq!(Source::from_key("show:x"), None);
    }

    #[test]
    fn moving_reorders_the_queue() {
        let queue = ids(&[1, 2, 3, 4]);
        assert_eq!(moved(&queue, EpisodeId(3), 0), ids(&[3, 1, 2, 4]));
        assert_eq!(moved(&queue, EpisodeId(1), 9), ids(&[2, 3, 4, 1]));
        assert_eq!(moved(&queue, EpisodeId(2), 2), ids(&[1, 3, 2, 4]));
        assert_eq!(moved(&queue, EpisodeId(8), 0), queue, "not queued");
    }
}

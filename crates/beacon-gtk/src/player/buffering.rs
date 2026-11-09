// SPDX-License-Identifier: GPL-3.0-or-later

//! How much of an episode the player holds. Streams are buffered to a file
//! as a whole (playbin's download buffering), so playback survives a
//! dropped connection, seeking back is instant, and the seek bar can show
//! how far it got (asked by the user 2026-11-09).

use std::path::Path;

use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_play as gst_play;

/// `GST_FORMAT_PERCENT_MAX`: a whole stream, in the percent format.
const PERCENT_MAX: f64 = 1_000_000.0;

/// Buffer `play`'s streams as a whole, in a file under `dir` rather than
/// the temporary folder, which is often in memory, and an episode can run
/// to a hundred megabytes. The file goes when the stream closes.
pub(super) fn download_to(play: &gst_play::Play, dir: &Path) {
    let pipeline = play.pipeline();
    let flags = pipeline.property_value("flags");
    let Some(class) = gst::glib::FlagsClass::with_type(flags.type_()) else {
        return;
    };
    if let Some(flags) = class
        .builder_with_value(flags)
        .and_then(|builder| builder.set_by_nick("download").build())
    {
        pipeline.set_property_from_value("flags", &flags);
    }
    if let Err(err) = std::fs::create_dir_all(dir) {
        tracing::warn!("could not make the buffer folder: {err}");
        return;
    }
    let template = dir.join("episode-XXXXXX").to_string_lossy().into_owned();
    if let Some(bin) = pipeline.downcast_ref::<gst::Bin>() {
        bin.connect_deep_element_added(move |_, _, element| {
            if element
                .factory()
                .is_some_and(|factory| factory.name() == "queue2")
            {
                element.set_property("temp-template", &template);
            }
        });
    }
}

/// How far into the stream `play` holds, as a share of it: the end of the
/// buffered stretch the position is in. `None` when the stream does not
/// say.
pub(super) fn buffered(play: &gst_play::Play) -> Option<f64> {
    let mut query = gst::query::Buffering::new(gst::Format::Percent);
    if !play.pipeline().query(&mut query) {
        tracing::trace!("the stream does not say how much is buffered");
        return None;
    }
    tracing::trace!(?query, "buffered");
    let share = |value: gst::GenericFormattedValue| value.value() as f64 / PERCENT_MAX;
    let mut ranges: Vec<(f64, f64)> = query
        .ranges()
        .map(|(start, stop)| (share(start), share(stop)))
        .collect();
    if ranges.is_empty() {
        let (start, stop, _) = query.range();
        ranges.push((share(start), share(stop)));
    }
    let at = match (play.position(), play.duration()) {
        (Some(position), Some(duration)) if !duration.is_zero() => {
            position.nseconds() as f64 / duration.nseconds() as f64
        }
        _ => 0.0,
    };
    Some(buffered_to(&ranges, at))
}

/// The end of the stretch in `ranges` (shares of the stream) that holds
/// `at`; `at` itself if none does, as nothing ahead of it is held.
pub(super) fn buffered_to(ranges: &[(f64, f64)], at: f64) -> f64 {
    ranges
        .iter()
        .filter(|(start, stop)| *start <= at + 0.001 && at <= *stop)
        .map(|(_, stop)| *stop)
        .fold(at, f64::max)
        .clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::buffered_to;

    #[test]
    fn the_bar_fills_to_the_end_of_the_stretch_playing() {
        assert_eq!(buffered_to(&[(0.0, 0.4)], 0.1), 0.4);
        // Seeked ahead: the download started again from there.
        assert_eq!(buffered_to(&[(0.0, 0.2), (0.5, 0.7)], 0.55), 0.7);
        assert_eq!(buffered_to(&[(0.0, 0.2)], 0.5), 0.5);
        assert_eq!(buffered_to(&[(0.0, 1.0)], 0.3), 1.0);
        assert_eq!(buffered_to(&[], 0.0), 0.0);
    }
}

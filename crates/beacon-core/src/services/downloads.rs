// SPDX-License-Identifier: GPL-3.0-or-later

//! Downloading episodes, only ever when asked. A download is written to
//! `ID.part` beside where it will end up, and renamed once complete, so a
//! file under its final name is always whole. A download that stops part
//! way (the connection drops, the app closes) leaves its part, and the
//! next download of that episode asks the server for the rest (an HTTP
//! range request), starting over only when the server sends it all again.
//! Cancelling deletes the part.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use futures_util::StreamExt;
use reqwest::StatusCode;
use reqwest::header::{CONTENT_RANGE, CONTENT_TYPE, RANGE};
use tokio::io::AsyncWriteExt;
use url::Url;

use crate::error::DownloadError;
use crate::model::EpisodeId;

/// How far a download got, shared with whoever shows it, and its Cancel.
#[derive(Debug, Default)]
pub struct Progress {
    received: AtomicU64,
    /// 0 while the size is unknown.
    total: AtomicU64,
    cancelled: AtomicBool,
}

impl Progress {
    /// The share received, once the size is known.
    pub fn fraction(&self) -> Option<f64> {
        let total = self.total.load(Ordering::Relaxed);
        (total > 0).then(|| {
            // Bytes as floats: exact enough for a progress bar.
            #[allow(clippy::cast_precision_loss)]
            let share = self.received.load(Ordering::Relaxed) as f64 / total as f64;
            share.min(1.0)
        })
    }

    /// The whole size, once the server has said it.
    pub fn total(&self) -> Option<u64> {
        Some(self.total.load(Ordering::Relaxed)).filter(|total| *total > 0)
    }

    pub fn received(&self) -> u64 {
        self.received.load(Ordering::Relaxed)
    }

    /// Stop the download, deleting what it got.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

/// Fetches episodes into one folder.
#[derive(Debug, Clone)]
pub struct Downloader {
    dir: PathBuf,
    http: reqwest::Client,
}

impl Downloader {
    /// Downloads into `dir`, through `http`, which must not limit how long a
    /// whole request takes (see [`crate::services::download_client`]).
    pub fn new(dir: PathBuf, http: reqwest::Client) -> Self {
        Self { dir, http }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Where an unfinished download of `id` is kept.
    pub fn partial_path(&self, id: EpisodeId) -> PathBuf {
        self.dir.join(format!("{}.part", id.0))
    }

    /// Download `url` as episode `id`, resuming a part left from before.
    /// Returns where the whole file is.
    pub async fn fetch(
        &self,
        id: EpisodeId,
        url: &Url,
        progress: &Progress,
    ) -> Result<PathBuf, DownloadError> {
        tokio::fs::create_dir_all(&self.dir).await?;
        let partial = self.partial_path(id);
        let result = self.fetch_into(&partial, url, progress).await;
        if let Err(DownloadError::Cancelled) = result {
            let _ = tokio::fs::remove_file(&partial).await;
        }
        let content_type = result?;
        let path = self.dir.join(format!(
            "{}.{}",
            id.0,
            extension(url, content_type.as_deref())
        ));
        tokio::fs::rename(&partial, &path).await?;
        Ok(path)
    }

    /// Fill `partial` from `url`, returning the content type.
    async fn fetch_into(
        &self,
        partial: &Path,
        url: &Url,
        progress: &Progress,
    ) -> Result<Option<String>, DownloadError> {
        let mut have = match tokio::fs::metadata(partial).await {
            Ok(meta) => meta.len(),
            Err(_) => 0,
        };
        let mut request = self.http.get(url.clone());
        if have > 0 {
            request = request.header(RANGE, format!("bytes={have}-"));
        }
        let mut response = request.send().await?;
        if response.status() == StatusCode::RANGE_NOT_SATISFIABLE {
            // The part is no use (the file changed, or it is all there but
            // was never renamed): start over.
            have = 0;
            response = self.http.get(url.clone()).send().await?;
        }
        let status = response.status();
        if !status.is_success() {
            return Err(DownloadError::Status(status.as_u16()));
        }
        // Only a partial answer for the range asked carries on the part.
        let resumed = status == StatusCode::PARTIAL_CONTENT
            && response
                .headers()
                .get(CONTENT_RANGE)
                .and_then(|v| v.to_str().ok())
                .and_then(range_start)
                == Some(have);
        if !resumed {
            have = 0;
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        if let Some(rest) = response.content_length() {
            progress.total.store(have + rest, Ordering::Relaxed);
        }
        progress.received.store(have, Ordering::Relaxed);

        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(resumed)
            .truncate(!resumed)
            .open(partial)
            .await?;
        let mut file = tokio::io::BufWriter::new(file);
        let mut body = response.bytes_stream();
        while let Some(chunk) = body.next().await {
            if progress.is_cancelled() {
                return Err(DownloadError::Cancelled);
            }
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            progress
                .received
                .fetch_add(chunk.len() as u64, Ordering::Relaxed);
        }
        file.flush().await?;
        if progress.is_cancelled() {
            return Err(DownloadError::Cancelled);
        }
        Ok(content_type)
    }
}

/// Where a `Content-Range: bytes START-END/SIZE` answer starts.
fn range_start(value: &str) -> Option<u64> {
    value
        .strip_prefix("bytes ")?
        .split('-')
        .next()?
        .trim()
        .parse()
        .ok()
}

/// The file's extension: one for its content type, if it is a known audio
/// type, else the address's, if short, else "audio". The type comes first,
/// as many feeds serve audio from addresses such as `play.php`.
fn extension(url: &Url, content_type: Option<&str>) -> String {
    let mime = content_type
        .and_then(|t| t.split(';').next())
        .map(|t| t.trim().to_ascii_lowercase());
    let from_type = match mime.as_deref() {
        Some("audio/mpeg" | "audio/mp3") => Some("mp3"),
        Some("audio/mp4" | "audio/x-m4a" | "audio/aac") => Some("m4a"),
        Some("audio/ogg" | "audio/vorbis") => Some("ogg"),
        Some("audio/opus") => Some("opus"),
        Some("audio/flac" | "audio/x-flac") => Some("flac"),
        Some("audio/wav" | "audio/x-wav") => Some("wav"),
        Some("video/mp4") => Some("mp4"),
        _ => None,
    };
    if let Some(ext) = from_type {
        return ext.to_owned();
    }
    url.path_segments()
        .and_then(|mut segments| segments.next_back())
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .filter(|ext| {
            (1..=5).contains(&ext.len()) && ext.chars().all(|c| c.is_ascii_alphanumeric())
        })
        .unwrap_or_else(|| "audio".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_come_from_the_address_then_the_type() {
        let url = |s| Url::parse(s).unwrap();
        assert_eq!(
            extension(&url("https://a.example/ep/1.MP3?x=1"), None),
            "mp3"
        );
        assert_eq!(
            extension(&url("https://a.example/ep/1"), Some("audio/x-m4a; q=1")),
            "m4a"
        );
        assert_eq!(
            extension(&url("https://a.example/play.php"), Some("audio/mpeg")),
            "mp3"
        );
        assert_eq!(
            extension(
                &url("https://a.example/1.opus"),
                Some("application/octet-stream")
            ),
            "opus"
        );
        assert_eq!(extension(&url("https://a.example/ep/"), None), "audio");
        assert_eq!(
            extension(&url("https://a.example/ep.with-dash"), Some("audio/ogg")),
            "ogg"
        );
    }

    #[test]
    fn content_ranges_are_read() {
        assert_eq!(range_start("bytes 100-199/200"), Some(100));
        assert_eq!(range_start("bytes */200"), None);
        assert_eq!(range_start("items 1-2/3"), None);
    }

    #[test]
    fn progress_is_a_share_once_the_size_is_known() {
        let progress = Progress::default();
        assert_eq!(progress.fraction(), None);
        progress.total.store(200, Ordering::Relaxed);
        progress.received.store(50, Ordering::Relaxed);
        assert_eq!(progress.fraction(), Some(0.25));
    }
}

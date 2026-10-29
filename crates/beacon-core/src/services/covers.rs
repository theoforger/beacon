// SPDX-License-Identifier: GPL-3.0-or-later

//! Disk cache for cover art, keyed by the image URL.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use url::Url;

use crate::error::CoverError;

#[derive(Debug, Clone)]
pub struct CoverCache {
    dir: PathBuf,
    http: reqwest::Client,
}

impl CoverCache {
    pub fn new(dir: PathBuf, http: reqwest::Client) -> Self {
        Self { dir, http }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Where the cover for `url` is, or will be, stored.
    pub fn path_for(&self, url: &Url) -> PathBuf {
        let digest = Sha256::digest(url.as_str().as_bytes());
        let name: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        self.dir.join(name)
    }

    /// Return the cached file for `url`, downloading it first if needed.
    pub async fn fetch(&self, url: &Url) -> Result<PathBuf, CoverError> {
        let path = self.path_for(url);
        if tokio::fs::try_exists(&path).await? {
            return Ok(path);
        }

        let response = self.http.get(url.clone()).send().await?;
        let status = response.status();
        if !status.is_success() {
            return Err(CoverError::Status(status.as_u16()));
        }
        let bytes = response.bytes().await?;

        tokio::fs::create_dir_all(&self.dir).await?;
        // Write to a temporary name first so a crash never leaves half a file
        // at the final path.
        let partial = path.with_extension("part");
        tokio::fs::write(&partial, &bytes).await?;
        tokio::fs::rename(&partial, &path).await?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_is_stable_and_url_specific() {
        let cache = CoverCache::new(PathBuf::from("/cache"), reqwest::Client::new());
        let a = Url::parse("https://example.com/a.png").unwrap();
        let b = Url::parse("https://example.com/b.png").unwrap();
        assert_eq!(cache.path_for(&a), cache.path_for(&a));
        assert_ne!(cache.path_for(&a), cache.path_for(&b));
        assert!(cache.path_for(&a).starts_with("/cache"));
    }
}

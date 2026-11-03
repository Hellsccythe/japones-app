//! Large downloads (models, voices): resumable, size- and SHA-256-checked.
//!
//! Data goes to `<dest>.part` and is renamed to `dest` only after it checks
//! out, so a file at `dest` is always complete.

use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use reqwest::{header, StatusCode};
use sha2::{Digest, Sha256};

/// What to download and how to check it.
pub struct Asset<'a> {
    pub url: &'a str,
    pub size: u64,
    /// Lowercase hex.
    pub sha256: &'a str,
}

fn part_path(dest: &Path) -> PathBuf {
    let mut p = dest.as_os_str().to_owned();
    p.push(".part");
    p.into()
}

/// Bytes already downloaded towards `dest` (complete or partial).
pub fn downloaded_bytes(dest: &Path) -> u64 {
    if dest.exists() {
        return std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
    }
    std::fs::metadata(part_path(dest)).map(|m| m.len()).unwrap_or(0)
}

/// Downloads `asset` to `dest`, resuming a previous `.part` file when there
/// is one. `progress(done, total)` is called as data arrives.
pub async fn download(asset: &Asset<'_>, dest: &Path, progress: impl Fn(u64, u64)) -> Result<()> {
    if std::fs::metadata(dest).map(|m| m.len() == asset.size).unwrap_or(false) {
        return Ok(());
    }
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let part = part_path(dest);
    let mut have = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    if have > asset.size {
        std::fs::remove_file(&part)?;
        have = 0;
    }

    if have < asset.size {
        let mut req = reqwest::Client::new().get(asset.url);
        if have > 0 {
            req = req.header(header::RANGE, format!("bytes={have}-"));
        }
        let mut resp = req.send().await?.error_for_status()?;
        // A server that ignores Range answers 200 with the whole file; appending
        // that to the partial file is how a download gets corrupted.
        let append = resp.status() == StatusCode::PARTIAL_CONTENT;
        if !append {
            have = 0;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(append)
            .truncate(!append)
            .open(&part)
            .with_context(|| format!("abrir {}", part.display()))?;

        let step = (asset.size / 200).max(1 << 20);
        let mut last = have;
        progress(have, asset.size);
        while let Some(chunk) = resp.chunk().await? {
            file.write_all(&chunk)?;
            have += chunk.len() as u64;
            if have - last >= step {
                progress(have, asset.size);
                last = have;
            }
        }
        file.flush()?;
        progress(have, asset.size);
    }

    if have != asset.size {
        bail!("download incompleto: {have} de {} bytes; tente de novo para continuar", asset.size);
    }
    let path = part.clone();
    let hash = tokio::task::spawn_blocking(move || sha256_file(&path)).await??;
    if !hash.eq_ignore_ascii_case(asset.sha256) {
        std::fs::remove_file(&part)?;
        bail!("arquivo baixado está corrompido (SHA-256 não confere); baixe de novo");
    }
    std::fs::rename(&part, dest)?;
    Ok(())
}

pub fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Unpacks a `.tgz`/`.tar.gz` into `dest_dir`. Blocking.
pub fn extract_tgz(archive: &Path, dest_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dest_dir)?;
    let file = std::fs::File::open(archive)?;
    tar::Archive::new(flate2::read::GzDecoder::new(file))
        .unpack(dest_dir)
        .with_context(|| format!("extrair {}", archive.display()))
}

/// Removes `dest` and any partial download of it.
pub fn remove(dest: &Path) -> Result<()> {
    for p in [dest.to_path_buf(), part_path(dest)] {
        match std::fs::remove_file(&p) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_file() {
        let dir = std::env::temp_dir().join(format!("assets-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.txt");
        std::fs::write(&f, b"abc").unwrap();
        assert_eq!(
            sha256_file(&f).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        remove(&f).unwrap();
        assert!(!f.exists());
    }
}

//! Large downloads (models, voices): resumable, size- and SHA-256-checked.
//!
//! Data goes to `<dest>.part` and is renamed to `dest` only after it checks
//! out, so a file at `dest` is always complete.

use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

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

/// Bumped by [`cancel_downloads`]; a download started under another value stops.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Error text of a download stopped by [`cancel_downloads`] (the `.part` file stays).
pub const PAUSED: &str = "download:paused";

/// Error text after the connection failed [`MAX_FAILURES`] times in a row.
const CONNECTION_LOST: &str = "A conexão caiu durante o download. O que já baixou está salvo: tente de novo para continuar.";

/// Failures in a row, with no new data, before giving up.
const MAX_FAILURES: u32 = 5;

/// A connection silent for this long counts as dropped.
const READ_TIMEOUT: Duration = Duration::from_secs(30);

/// Stops every download in progress; each one fails with [`PAUSED`] and
/// resumes from its `.part` file when started again.
pub fn cancel_downloads() {
    GENERATION.fetch_add(1, Ordering::SeqCst);
}

/// The generation a download starts under.
fn generation() -> u64 {
    GENERATION.load(Ordering::SeqCst)
}

/// Whether downloads were cancelled since `started`.
fn cancelled(started: u64) -> bool {
    GENERATION.load(Ordering::SeqCst) != started
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

/// Whether trying again may help: the connection failed, or the server had a
/// temporary problem. A 404 and the like won't change.
fn is_connection_problem(e: &anyhow::Error) -> bool {
    e.downcast_ref::<reqwest::Error>().is_some_and(|e| {
        e.status().is_none_or(|s| s.is_server_error() || s == StatusCode::TOO_MANY_REQUESTS)
    })
}

/// Wait before retry number `failures`: 2, 4, 8 and 16 s.
fn retry_delay(failures: u32) -> Duration {
    if cfg!(test) {
        return Duration::from_millis(1);
    }
    Duration::from_secs(1 << failures.min(4))
}

/// Appends to `part` from byte `have` until the server stops sending.
async fn fetch(
    client: &reqwest::Client,
    asset: &Asset<'_>,
    part: &Path,
    have: &mut u64,
    started: u64,
    progress: &impl Fn(u64, u64),
) -> Result<()> {
    let mut req = client.get(asset.url);
    if *have > 0 {
        req = req.header(header::RANGE, format!("bytes={have}-"));
    }
    let mut resp = req.send().await?.error_for_status()?;
    // A server that ignores Range answers 200 with the whole file; appending
    // that to the partial file is how a download gets corrupted.
    let append = resp.status() == StatusCode::PARTIAL_CONTENT;
    if !append {
        *have = 0;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(!append)
        .open(part)
        .with_context(|| format!("abrir {}", part.display()))?;

    let step = (asset.size / 200).max(1 << 20);
    let mut last = *have;
    progress(*have, asset.size);
    while let Some(chunk) = resp.chunk().await? {
        if cancelled(started) {
            file.flush()?;
            bail!(PAUSED);
        }
        file.write_all(&chunk)?;
        *have += chunk.len() as u64;
        if *have - last >= step {
            progress(*have, asset.size);
            last = *have;
        }
    }
    file.flush()?;
    progress(*have, asset.size);
    Ok(())
}

/// Downloads `asset` to `dest`, resuming a previous `.part` file when there
/// is one. `progress(done, total)` is called as data arrives.
pub async fn download(asset: &Asset<'_>, dest: &Path, progress: impl Fn(u64, u64)) -> Result<()> {
    let started = generation();
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
        let client = reqwest::Client::builder().read_timeout(READ_TIMEOUT).build()?;
        let mut failures = 0;
        loop {
            let before = have;
            match fetch(&client, asset, &part, &mut have, started, &progress).await {
                Ok(()) if have >= asset.size => break,
                Ok(()) => {}
                Err(e) if !is_connection_problem(&e) => return Err(e),
                Err(_) => {}
            }
            // Only failures in a row with nothing new count: a slow, shaky
            // connection that keeps bringing data still finishes.
            failures = if have > before { 1 } else { failures + 1 };
            if failures >= MAX_FAILURES {
                bail!(CONNECTION_LOST);
            }
            tokio::time::sleep(retry_delay(failures)).await;
            if cancelled(started) {
                bail!(PAUSED);
            }
        }
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
    use std::io::BufRead;
    use std::net::TcpListener;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;

    /// `cancel_downloads` is global: tests that download or cancel run one at a time.
    static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    /// Serves `data` from a local server, honoring Range, sending at most
    /// `per_request` bytes before dropping the connection. Returns the URL and
    /// the number of requests so far.
    fn flaky_server(data: Vec<u8>, per_request: usize) -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/file", listener.local_addr().unwrap());
        let requests = Arc::new(AtomicUsize::new(0));
        let count = requests.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                count.fetch_add(1, Ordering::SeqCst);
                let mut from = 0;
                let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                while reader.read_line(&mut line).unwrap() > 0 && line != "\r\n" {
                    if let Some(range) = line.to_lowercase().strip_prefix("range: bytes=") {
                        from = range.trim().trim_end_matches('-').parse().unwrap();
                    }
                    line.clear();
                }
                let rest = &data[from..];
                let status = if from > 0 { "206 Partial Content" } else { "200 OK" };
                let head = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\n\r\n", rest.len());
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&rest[..rest.len().min(per_request)]);
                // Dropping the stream here cuts the body short.
            }
        });
        (url, requests)
    }

    fn temp_dest(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("assets-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("file.bin");
        remove(&dest).unwrap();
        dest
    }

    #[tokio::test]
    async fn a_dropped_connection_resumes_by_itself() {
        let _serial = SERIAL.lock().await;
        let data = vec![7u8; 1000];
        let (url, requests) = flaky_server(data.clone(), 400);
        let dest = temp_dest("resume");
        let sha = sha256_hex(&data);
        download(&Asset { url: &url, size: 1000, sha256: &sha }, &dest, |_, _| {}).await.unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), data);
        assert_eq!(requests.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn a_connection_making_progress_never_gives_up() {
        let _serial = SERIAL.lock().await;
        let data = vec![9u8; 1000];
        // Many more drops than MAX_FAILURES, but each one brings new data.
        let (url, _) = flaky_server(data.clone(), 50);
        let dest = temp_dest("progress");
        let sha = sha256_hex(&data);
        download(&Asset { url: &url, size: 1000, sha256: &sha }, &dest, |_, _| {}).await.unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), data);
    }

    #[tokio::test]
    async fn a_dead_connection_gives_up_with_a_clear_message_and_keeps_the_part() {
        let _serial = SERIAL.lock().await;
        let data = vec![1u8; 1000];
        let (url, requests) = flaky_server(data.clone(), 0);
        let dest = temp_dest("dead");
        std::fs::write(part_path(&dest), &data[..100]).unwrap();
        let sha = sha256_hex(&data);
        let err = download(&Asset { url: &url, size: 1000, sha256: &sha }, &dest, |_, _| {}).await.unwrap_err();
        assert_eq!(err.to_string(), CONNECTION_LOST);
        assert_eq!(requests.load(Ordering::SeqCst), MAX_FAILURES as usize);
        assert_eq!(downloaded_bytes(&dest), 100);
    }

    #[tokio::test]
    async fn a_missing_file_is_not_retried() {
        let _serial = SERIAL.lock().await;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/file", listener.local_addr().unwrap());
        let requests = Arc::new(AtomicUsize::new(0));
        let count = requests.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                count.fetch_add(1, Ordering::SeqCst);
                let mut line = String::new();
                let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
                while reader.read_line(&mut line).unwrap() > 0 && line != "\r\n" {
                    line.clear();
                }
                let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
            }
        });
        let dest = temp_dest("missing");
        let asset = Asset { url: &url, size: 10, sha256: "00" };
        assert!(download(&asset, &dest, |_, _| {}).await.is_err());
        assert_eq!(requests.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn cancel_stops_downloads_started_before_it_only() {
        let _serial = SERIAL.blocking_lock();
        let before = generation();
        assert!(!cancelled(before));
        cancel_downloads();
        assert!(cancelled(before));
        assert!(!cancelled(generation()));
    }

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

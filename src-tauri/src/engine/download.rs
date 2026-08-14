//! Streaming downloads with progress events, SHA-256 verification and
//! cancellation. Used for the engine build and AI model files.
//!
//! Large files use a few parallel HTTP range requests (the same approach
//! Hugging Face's own `hf_transfer` uses) so a single TCP connection is not
//! the ceiling. If the server does not support ranges, we fall back to one
//! stream. Interrupted `.part` files resume instead of starting over.

use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use reqwest::header::{ACCEPT_ENCODING, CONTENT_RANGE, RANGE};
use reqwest::StatusCode;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::core::Events;

/// Four connections is enough to fill a typical home link and matches what
/// official HF clients do. More starts to look like scraping.
const MAX_PARTS: usize = 4;
const PARALLEL_MIN_BYTES: u64 = 32 * 1024 * 1024;
const STALL_TIMEOUT: Duration = Duration::from_secs(90);
const FIRST_BYTE_TIMEOUT: Duration = Duration::from_secs(60);

pub struct DownloadTicket {
    pub kind: &'static str,
    pub id: String,
    pub name: String,
}

/// Cancel the transfer, or skip SHA-256 after the bytes are on disk.
#[derive(Clone)]
pub struct DownloadControl {
    cancel: Arc<AtomicBool>,
    skip_verify: Arc<AtomicBool>,
}

impl DownloadControl {
    pub fn new() -> Self {
        Self {
            cancel: Arc::new(AtomicBool::new(false)),
            skip_verify: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn request_skip_verify(&self) {
        self.skip_verify.store(true, Ordering::Relaxed);
    }
}

/// Download `url` to `dest` (atomically via a `.part` file), emitting
/// `rebost://download` progress events. Verifies SHA-256 when given.
///
/// Hugging Face CDN signatures are bound to one Range header. Every request
/// therefore starts from the original `/resolve/` URL so each range gets its
/// own redirect. Reusing a signed URL from a 0-0 probe yields 403.
pub async fn download(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    ticket: &DownloadTicket,
    expected_sha256: Option<&str>,
    known_size: Option<u64>,
    events: &Arc<dyn Events>,
    control: &DownloadControl,
) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let part = dest.with_extension("part");
    if part_is_untrusted(&part, known_size) {
        log::warn!("discarding incomplete download {}", part.display());
        tokio::fs::remove_file(&part).await.ok();
    }

    let emit_progress =
        |received: u64, total: Option<u64>, done: bool, error: Option<&str>, phase: &str| {
            events.emit(
                "rebost://download",
                json!({
                    "kind": ticket.kind,
                    "id": ticket.id,
                    "name": ticket.name,
                    "received": received,
                    "total": total,
                    "done": done,
                    "error": error,
                    "phase": phase,
                }),
            );
        };

    let mut attempt = 0;
    loop {
        let part_len = tokio::fs::metadata(&part)
            .await
            .ok()
            .map(|m| m.len())
            .unwrap_or(0);
        emit_progress(part_len, known_size, false, None, "downloading");

        let skip_transfer = known_size.is_some_and(|total| part_len == total);
        if !skip_transfer {
            let parallel_ok =
                part_len == 0 && known_size.is_some_and(|total| total >= PARALLEL_MIN_BYTES);
            let result = if parallel_ok {
                match download_parallel(
                    client,
                    url,
                    &part,
                    known_size.expect("parallel_ok requires a known size"),
                    &emit_progress,
                    &control.cancel,
                )
                .await
                {
                    Ok(()) => Ok(()),
                    Err(error) if is_fatal_download_error(&error) => Err(error),
                    Err(_) => {
                        tokio::fs::remove_file(&part).await.ok();
                        download_sequential(
                            client,
                            url,
                            &part,
                            known_size,
                            0,
                            &emit_progress,
                            &control.cancel,
                        )
                        .await
                    }
                }
            } else {
                download_sequential(
                    client,
                    url,
                    &part,
                    known_size,
                    part_len,
                    &emit_progress,
                    &control.cancel,
                )
                .await
            };
            result?;
        }

        if let Some(expected) = expected_sha256 {
            if control.skip_verify.load(Ordering::Relaxed) {
                log::warn!("SHA-256 check skipped by user");
                break;
            }
            let total = known_size.or(tokio::fs::metadata(&part).await.ok().map(|m| m.len()));
            emit_progress(0, total, false, None, "verifying");
            match verify_sha256(
                &part,
                expected,
                total,
                &emit_progress,
                &control.cancel,
                &control.skip_verify,
            )
            .await
            {
                Ok(()) => break,
                Err(error) if error.to_string() == "cancelled" => return Err(error),
                Err(error) if skip_transfer && attempt == 0 => {
                    log::warn!("existing download failed verification; starting over: {error:#}");
                    tokio::fs::remove_file(&part).await.ok();
                    attempt += 1;
                    continue;
                }
                Err(error) => {
                    tokio::fs::remove_file(&part).await.ok();
                    emit_progress(
                        0,
                        known_size,
                        false,
                        Some("verification failed"),
                        "verifying",
                    );
                    return Err(error);
                }
            }
        } else {
            break;
        }
    }

    let received = tokio::fs::metadata(&part).await?.len();
    tokio::fs::rename(&part, dest).await?;
    emit_progress(
        received,
        known_size.or(Some(received)),
        true,
        None,
        "downloading",
    );
    Ok(())
}

async fn download_sequential(
    client: &reqwest::Client,
    url: &str,
    part: &Path,
    known_total: Option<u64>,
    mut received: u64,
    emit_progress: &impl Fn(u64, Option<u64>, bool, Option<&str>, &str),
    cancel: &Arc<AtomicBool>,
) -> Result<()> {
    if let Some(total) = known_total {
        if received > total {
            tokio::fs::remove_file(part).await.ok();
            received = 0;
        } else if received == total {
            return Ok(());
        }
    }

    let mut retried_resume = false;
    let response = loop {
        let mut request = client.get(url).header(ACCEPT_ENCODING, "identity");
        if received > 0 {
            request = request.header(RANGE, format!("bytes={received}-"));
        }
        let response = first_byte(request.send()).await?;
        let status = response.status();
        rate_limit(status)?;

        let resume = status == StatusCode::PARTIAL_CONTENT && received > 0;
        if received > 0 && !resume {
            tokio::fs::remove_file(part).await.ok();
            received = 0;
            if status == StatusCode::FORBIDDEN && !retried_resume {
                retried_resume = true;
                continue;
            }
            if !status.is_success() {
                return Err(anyhow!("download failed ({status})"));
            }
        } else if !status.is_success() && status != StatusCode::PARTIAL_CONTENT {
            return Err(anyhow!("download failed ({status})"));
        }
        break response;
    };
    let resume = received > 0;

    let total = if resume {
        parse_content_range_total(
            response
                .headers()
                .get(CONTENT_RANGE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or(""),
        )
        .or(known_total)
        .or_else(|| response.content_length().map(|len| received + len))
    } else {
        response.content_length().or(known_total)
    };

    let mut file = if resume {
        tokio::fs::OpenOptions::new()
            .append(true)
            .open(part)
            .await?
    } else {
        tokio::fs::File::create(part).await?
    };

    emit_progress(received, total, false, None, "downloading");
    let mut last_emit = Instant::now() - Duration::from_secs(1);
    let mut stream = response.bytes_stream();
    loop {
        if cancel.load(Ordering::Relaxed) {
            drop(file);
            tokio::fs::remove_file(part).await.ok();
            emit_progress(received, total, false, Some("cancelled"), "downloading");
            return Err(anyhow!("cancelled"));
        }
        let chunk = match next_chunk(&mut stream).await? {
            Some(chunk) => chunk,
            None => break,
        };
        received += chunk.len() as u64;
        file.write_all(&chunk).await?;
        if last_emit.elapsed() >= Duration::from_millis(150) {
            emit_progress(received, total, false, None, "downloading");
            last_emit = Instant::now();
        }
    }
    file.flush().await?;
    Ok(())
}

async fn download_parallel(
    client: &reqwest::Client,
    url: &str,
    part: &Path,
    total: u64,
    emit_progress: &impl Fn(u64, Option<u64>, bool, Option<&str>, &str),
    cancel: &Arc<AtomicBool>,
) -> Result<()> {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(part)?;
    let file = Arc::new(Mutex::new(file));
    let received = Arc::new(AtomicU64::new(0));
    let abort = Arc::new(AtomicBool::new(false));
    let ranges = split_parts(total, MAX_PARTS);

    emit_progress(0, Some(total), false, None, "downloading");

    let mut tasks = Vec::new();
    for (start, end) in ranges {
        let client = client.clone();
        let url = url.to_string();
        let file = file.clone();
        let received = received.clone();
        let cancel = cancel.clone();
        let abort = abort.clone();
        tasks.push(tokio::spawn(async move {
            fetch_range(&client, &url, start, end, &file, &received, &cancel, &abort).await
        }));
    }

    let mut last_emit = Instant::now() - Duration::from_secs(1);
    let mut last_bytes = 0u64;
    let mut last_progress = Instant::now();
    loop {
        if tasks.iter().all(|task| task.is_finished()) {
            break;
        }
        let now = received.load(Ordering::Relaxed);
        if now > last_bytes {
            last_bytes = now;
            last_progress = Instant::now();
        } else if last_progress.elapsed() >= STALL_TIMEOUT {
            abort.store(true, Ordering::Relaxed);
            tokio::fs::remove_file(part).await.ok();
            emit_progress(now, Some(total), false, Some("stalled"), "downloading");
            return Err(stalled());
        }
        if last_emit.elapsed() >= Duration::from_millis(150) {
            emit_progress(now, Some(total), false, None, "downloading");
            last_emit = Instant::now();
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let mut first_error: Option<anyhow::Error> = None;
    for task in tasks {
        match task
            .await
            .unwrap_or_else(|_| Err(anyhow!("download task failed")))
        {
            Ok(()) => {}
            Err(error) if error.to_string() == "cancelled" => {
                abort.store(true, Ordering::Relaxed);
                tokio::fs::remove_file(part).await.ok();
                emit_progress(
                    received.load(Ordering::Relaxed),
                    Some(total),
                    false,
                    Some("cancelled"),
                    "downloading",
                );
                return Err(error);
            }
            Err(error) => {
                abort.store(true, Ordering::Relaxed);
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }
    if let Some(error) = first_error {
        tokio::fs::remove_file(part).await.ok();
        return Err(error);
    }
    {
        let file = crate::core::mutex_lock(&file);
        file.sync_all()?;
    }
    emit_progress(total, Some(total), false, None, "downloading");
    Ok(())
}

async fn fetch_range(
    client: &reqwest::Client,
    url: &str,
    start: u64,
    end: u64,
    file: &Arc<Mutex<std::fs::File>>,
    received: &AtomicU64,
    cancel: &Arc<AtomicBool>,
    abort: &Arc<AtomicBool>,
) -> Result<()> {
    let request = client
        .get(url)
        .header(RANGE, format!("bytes={start}-{}", end - 1))
        .header(ACCEPT_ENCODING, "identity");
    let response = match first_byte(request.send()).await {
        Ok(response) => response,
        Err(error) => {
            abort.store(true, Ordering::Relaxed);
            return Err(error).with_context(|| format!("range {start}-{}", end - 1));
        }
    };
    let status = response.status();
    if let Err(error) = rate_limit(status) {
        abort.store(true, Ordering::Relaxed);
        return Err(error);
    }
    if status != StatusCode::PARTIAL_CONTENT {
        if status.is_success() {
            if response.content_length() != Some(end - start) {
                abort.store(true, Ordering::Relaxed);
                return Err(anyhow!("server ignored range request"));
            }
        } else {
            abort.store(true, Ordering::Relaxed);
            return Err(anyhow!("download failed ({status})"));
        }
    }

    let mut offset = start;
    let mut stream = response.bytes_stream();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(anyhow!("cancelled"));
        }
        if abort.load(Ordering::Relaxed) {
            return Ok(());
        }
        let chunk = match next_chunk(&mut stream).await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(error) => {
                abort.store(true, Ordering::Relaxed);
                return Err(error);
            }
        };
        {
            let mut file = crate::core::mutex_lock(file);
            file.seek(SeekFrom::Start(offset))?;
            file.write_all(&chunk)?;
        }
        offset += chunk.len() as u64;
        received.fetch_add(chunk.len() as u64, Ordering::Relaxed);
    }
    if offset != end {
        abort.store(true, Ordering::Relaxed);
        return Err(anyhow!("incomplete range {start}-{end}"));
    }
    Ok(())
}

/// SHA-256 a file on the calling thread. Used for `REBOST_ENGINE_ARCHIVE`
/// so a local copy is checked the same way as a network download.
pub fn verify_sha256_blocking(path: &Path, expected: &str) -> Result<()> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).with_context(|| path.display().to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 8 * 1024 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    if hex.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(anyhow!("SHA-256 mismatch: got {hex}, expected {expected}"))
    }
}

async fn verify_sha256(
    path: &Path,
    expected: &str,
    total: Option<u64>,
    emit_progress: &impl Fn(u64, Option<u64>, bool, Option<&str>, &str),
    cancel: &Arc<AtomicBool>,
    skip_verify: &Arc<AtomicBool>,
) -> Result<()> {
    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 8 * 1024 * 1024];
    let mut hashed = 0u64;
    let mut last_emit = Instant::now() - Duration::from_secs(1);
    loop {
        if cancel.load(Ordering::Relaxed) {
            emit_progress(hashed, total, false, Some("cancelled"), "verifying");
            return Err(anyhow!("cancelled"));
        }
        if skip_verify.load(Ordering::Relaxed) {
            log::warn!("SHA-256 check skipped by user after {hashed} bytes");
            return Ok(());
        }
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        hashed += n as u64;
        if last_emit.elapsed() >= Duration::from_millis(150) {
            emit_progress(hashed, total, false, None, "verifying");
            last_emit = Instant::now();
        }
    }
    emit_progress(hashed, total.or(Some(hashed)), false, None, "verifying");
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    if hex.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(anyhow!("SHA-256 mismatch: got {hex}, expected {expected}"))
    }
}

async fn first_byte(
    send: impl std::future::Future<Output = reqwest::Result<reqwest::Response>>,
) -> Result<reqwest::Response> {
    match tokio::time::timeout(FIRST_BYTE_TIMEOUT, send).await {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(error)) => Err(error).context("request"),
        Err(_) => Err(stalled()),
    }
}

async fn next_chunk(
    stream: &mut (impl StreamExt<Item = reqwest::Result<bytes::Bytes>> + Unpin),
) -> Result<Option<bytes::Bytes>> {
    match tokio::time::timeout(STALL_TIMEOUT, stream.next()).await {
        Ok(Some(Ok(chunk))) => Ok(Some(chunk)),
        Ok(Some(Err(error))) => Err(error).context("download stream"),
        Ok(None) => Ok(None),
        Err(_) => Err(stalled()),
    }
}

fn stalled() -> anyhow::Error {
    anyhow!("The download stalled. Check your connection and try again.")
}

fn rate_limit(status: StatusCode) -> Result<()> {
    if status == StatusCode::TOO_MANY_REQUESTS {
        Err(anyhow!(
            "The download was rate-limited. Wait a moment and try again."
        ))
    } else {
        Ok(())
    }
}

fn is_fatal_download_error(error: &anyhow::Error) -> bool {
    let message = error.to_string();
    message == "cancelled" || message.contains("rate-limited") || message.contains("stalled")
}

fn parse_content_range_total(header: &str) -> Option<u64> {
    let total = header.rsplit('/').next()?;
    if total == "*" {
        None
    } else {
        total.parse().ok()
    }
}

/// A leftover parallel download preallocates the full size with holes.
/// Treating that as finished shows 100% while hashing empty bytes for minutes.
fn part_is_untrusted(path: &Path, known_size: Option<u64>) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if meta.len() == 0 {
        return false;
    }
    if let Some(total) = known_size {
        if meta.len() > total {
            return true;
        }
    }
    allocated_bytes(&meta) < meta.len().saturating_mul(8) / 10
}

fn allocated_bytes(meta: &std::fs::Metadata) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        meta.blocks().saturating_mul(512)
    }
    #[cfg(not(unix))]
    {
        meta.len()
    }
}

/// Inclusive-exclusive byte ranges covering `total`, split across up to `n` parts.
fn split_parts(total: u64, n: usize) -> Vec<(u64, u64)> {
    if total == 0 {
        return Vec::new();
    }
    let n = (n as u64).min(total).max(1);
    let chunk = total / n;
    (0..n)
        .map(|i| {
            let start = i * chunk;
            let end = if i + 1 == n { total } else { start + chunk };
            (start, end)
        })
        .filter(|(start, end)| start < end)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_parts_even() {
        assert_eq!(
            split_parts(100, 4),
            vec![(0, 25), (25, 50), (50, 75), (75, 100)]
        );
    }

    #[test]
    fn split_parts_remainder_on_last() {
        assert_eq!(split_parts(10, 3), vec![(0, 3), (3, 6), (6, 10)]);
    }

    #[test]
    fn split_parts_fewer_than_requested_when_tiny() {
        assert_eq!(split_parts(2, 8), vec![(0, 1), (1, 2)]);
    }

    #[test]
    fn content_range_total() {
        assert_eq!(parse_content_range_total("bytes 0-0/12345"), Some(12345));
        assert_eq!(parse_content_range_total("bytes 0-0/*"), None);
    }

    #[cfg(unix)]
    #[test]
    fn preallocated_sparse_part_is_untrusted() {
        let path = std::env::temp_dir().join(format!(
            "rebost-sparse-part-{}.gguf.part",
            std::process::id()
        ));
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(10_000_000).unwrap();
        drop(file);
        let untrusted = part_is_untrusted(&path, Some(10_000_000));
        std::fs::remove_file(&path).ok();
        assert!(untrusted);
    }

    #[cfg(unix)]
    #[test]
    fn written_file_is_trusted() {
        let path = std::env::temp_dir().join(format!(
            "rebost-written-part-{}.gguf.part",
            std::process::id()
        ));
        std::fs::write(&path, vec![1u8; 64 * 1024]).unwrap();
        let untrusted = part_is_untrusted(&path, Some(64 * 1024));
        std::fs::remove_file(&path).ok();
        assert!(!untrusted);
    }

    #[test]
    fn blocking_sha256_mismatch_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("engine.tar.gz");
        std::fs::write(&path, b"not-the-archive").unwrap();
        let error = verify_sha256_blocking(&path, &"00".repeat(32)).unwrap_err();
        assert!(error.to_string().contains("mismatch"), "got {error:#}");
    }

    #[tokio::test]
    async fn http_download_verifies_sha256() {
        let body = b"rebost-engine-fixture";
        let expected = {
            let mut hasher = Sha256::new();
            hasher.update(body);
            hasher
                .finalize()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 2048];
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let _ = stream.read(&mut buf).await;
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(header.as_bytes()).await.ok();
            stream.write_all(body).await.ok();
        });

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("engine.bin");
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        struct Noop;
        impl crate::core::Events for Noop {
            fn emit(&self, _event: &str, _payload: serde_json::Value) {}
        }
        let events: Arc<dyn crate::core::Events> = Arc::new(Noop);
        download(
            &client,
            &format!("http://127.0.0.1:{port}/engine.bin"),
            &dest,
            &DownloadTicket {
                kind: "engine",
                id: "test".into(),
                name: "test".into(),
            },
            Some(&expected),
            Some(body.len() as u64),
            &events,
            &DownloadControl::new(),
        )
        .await
        .unwrap();
        assert_eq!(tokio::fs::read(&dest).await.unwrap(), body);
        server.abort();
    }

    #[tokio::test]
    async fn sha256_mismatch_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("weights.bin");
        tokio::fs::write(&path, b"not-the-model").await.unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let skip = Arc::new(AtomicBool::new(false));
        let expected = "00".repeat(32);
        let error = verify_sha256(&path, &expected, None, &|_, _, _, _, _| {}, &cancel, &skip)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("mismatch"), "got {error:#}");
    }

    #[tokio::test]
    async fn sha256_can_be_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("weights.bin");
        tokio::fs::write(&path, b"not-the-model").await.unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let skip = Arc::new(AtomicBool::new(true));
        let expected = "00".repeat(32);
        verify_sha256(&path, &expected, None, &|_, _, _, _, _| {}, &cancel, &skip)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn cancel_wins_over_skip_verify() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("weights.bin");
        tokio::fs::write(&path, b"not-the-model").await.unwrap();
        let cancel = Arc::new(AtomicBool::new(true));
        let skip = Arc::new(AtomicBool::new(true));
        let expected = "00".repeat(32);
        let error = verify_sha256(&path, &expected, None, &|_, _, _, _, _| {}, &cancel, &skip)
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), "cancelled");
    }

    #[tokio::test]
    async fn skipped_verify_keeps_an_already_downloaded_file() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("model.gguf");
        let part = dest.with_extension("part");
        let body = b"already-downloaded";
        tokio::fs::write(&part, body).await.unwrap();
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        struct Noop;
        impl crate::core::Events for Noop {
            fn emit(&self, _event: &str, _payload: serde_json::Value) {}
        }
        let events: Arc<dyn crate::core::Events> = Arc::new(Noop);
        let control = DownloadControl::new();
        control.request_skip_verify();
        download(
            &client,
            "http://127.0.0.1:1/unused",
            &dest,
            &DownloadTicket {
                kind: "model",
                id: "test".into(),
                name: "test".into(),
            },
            Some(&"00".repeat(32)),
            Some(body.len() as u64),
            &events,
            &control,
        )
        .await
        .unwrap();
        assert_eq!(tokio::fs::read(&dest).await.unwrap(), body);
    }
}

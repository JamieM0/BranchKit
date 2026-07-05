//! Resumable, sha256-verified downloads with progress — ARCHITECTURE.md §10, shared by both the
//! `llama-server` binary and the GGUF model download (same technique for each: `Range` header
//! against a `.part` file, sha256 check on completion, progress events with a computed MBps).

use std::io::SeekFrom;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadOutcome {
    Completed,
    Cancelled,
}

/// Lowercase hex, since `sha2`'s digest type doesn't implement `LowerHex` directly.
fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        write!(s, "{b:02x}").expect("writing to a String never fails");
    }
    s
}

/// Downloads `url` to `dest`, resuming from `dest.part` if it already exists (a prior run was
/// cancelled or the app quit mid-download). Verifies `expected_sha256` once the full file is on
/// disk before the atomic rename into `dest`; a mismatch deletes the partial file entirely (no
/// point resuming corrupt bytes) and returns an error. Calls
/// `on_progress(downloaded, total, mbps)` at most a few times a second.
pub async fn download_resumable(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    expected_sha256: &str,
    cancel: Arc<AtomicBool>,
    mut on_progress: impl FnMut(u64, Option<u64>, f64) + Send,
) -> Result<DownloadOutcome, AppError> {
    // Always `<dest>.part`, unambiguous even for extension-less dest paths.
    let part_path = {
        let mut s = dest.as_os_str().to_owned();
        s.push(".part");
        std::path::PathBuf::from(s)
    };

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let already = tokio::fs::metadata(&part_path).await.map(|m| m.len()).unwrap_or(0);

    let mut request = client.get(url);
    if already > 0 {
        request = request.header("Range", format!("bytes={already}-"));
    }
    let mut response = request.send().await?;
    if !response.status().is_success() {
        return Err(AppError::new(
            "Could not download this file",
            format!("HTTP {} from {url}", response.status()),
        ));
    }

    // The server may ignore our Range request (some CDNs do for small/edge cases) and resend the
    // whole body from byte 0 — only treat this as a genuine resume if it answered 206.
    let resumed = already > 0 && response.status().as_u16() == 206;
    let mut file = if resumed {
        let mut f = tokio::fs::OpenOptions::new().append(true).open(&part_path).await?;
        f.seek(SeekFrom::End(0)).await?;
        f
    } else {
        tokio::fs::File::create(&part_path).await?
    };

    let total = response.content_length().map(|len| if resumed { len + already } else { len });
    let mut downloaded = if resumed { already } else { 0 };
    let mut hasher = Sha256::new();
    if resumed {
        // Re-hash the bytes already on disk so the final digest covers the whole file.
        let mut existing = tokio::fs::File::open(&part_path).await?;
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = existing.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
    }

    let mut last_emit = Instant::now();
    let mut last_emit_bytes = downloaded;

    loop {
        if cancel.load(Ordering::SeqCst) {
            return Ok(DownloadOutcome::Cancelled);
        }
        let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| AppError::new("Lost connection while downloading", e.to_string()))?
        else {
            break;
        };
        file.write_all(&chunk).await?;
        hasher.update(&chunk);
        downloaded += chunk.len() as u64;

        if last_emit.elapsed() >= Duration::from_millis(250) {
            let elapsed = last_emit.elapsed().as_secs_f64().max(0.001);
            let mbps = ((downloaded - last_emit_bytes) as f64 / elapsed) / (1024.0 * 1024.0);
            on_progress(downloaded, total, mbps);
            last_emit = Instant::now();
            last_emit_bytes = downloaded;
        }
    }
    on_progress(downloaded, total, 0.0);
    file.flush().await?;
    drop(file);

    let digest = hex_encode(&hasher.finalize());
    if digest != expected_sha256 {
        let _ = tokio::fs::remove_file(&part_path).await;
        return Err(AppError::new(
            "The downloaded file didn't match its expected checksum",
            format!("sha256 mismatch: expected {expected_sha256}, got {digest}"),
        ));
    }

    tokio::fs::rename(&part_path, dest).await?;
    Ok(DownloadOutcome::Completed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// A minimal single-request HTTP/1.1 server that understands `Range: bytes=N-` and serves
    /// `body` (or the suffix from `N`) with `Content-Length`/`Content-Range`/206 as appropriate.
    /// Hand-rolled rather than pulling in a mock-HTTP crate (CLAUDE.md: no new deps without
    /// justification) — the protocol subset this test needs is tiny and stable.
    fn serve_once(body: &'static [u8]) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).unwrap();
            let request = String::from_utf8_lossy(&buf[..n]);
            let range_start = request
                .lines()
                .find_map(|l| l.strip_prefix("Range: bytes="))
                .and_then(|r| r.trim_end_matches('-').parse::<usize>().ok());

            match range_start {
                Some(start) if start < body.len() => {
                    let slice = &body[start..];
                    let header = format!(
                        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len() - 1,
                        body.len(),
                        slice.len()
                    );
                    stream.write_all(header.as_bytes()).unwrap();
                    stream.write_all(slice).unwrap();
                }
                _ => {
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    stream.write_all(header.as_bytes()).unwrap();
                    stream.write_all(body).unwrap();
                }
            }
        });
        format!("http://{addr}/file")
    }

    fn sha256_hex(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex_encode(&hasher.finalize())
    }

    #[tokio::test]
    async fn downloads_full_file_and_verifies_checksum() {
        let body: &'static [u8] = b"hello world, this is the file contents";
        let url = serve_once(body);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("out.bin");

        let outcome = download_resumable(
            &reqwest::Client::new(),
            &url,
            &dest,
            &sha256_hex(body),
            Arc::new(AtomicBool::new(false)),
            |_, _, _| {},
        )
        .await
        .unwrap();

        assert_eq!(outcome, DownloadOutcome::Completed);
        assert_eq!(tokio::fs::read(&dest).await.unwrap(), body);
        let mut part = dest.into_os_string();
        part.push(".part");
        assert!(!std::path::Path::new(&part).exists());
    }

    #[tokio::test]
    async fn rejects_a_checksum_mismatch_and_removes_the_partial_file() {
        let body: &'static [u8] = b"some bytes";
        let url = serve_once(body);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("out.bin");

        let err = download_resumable(
            &reqwest::Client::new(),
            &url,
            &dest,
            "0000000000000000000000000000000000000000000000000000000000000000",
            Arc::new(AtomicBool::new(false)),
            |_, _, _| {},
        )
        .await
        .unwrap_err();

        assert!(err.user_message.contains("checksum"));
        assert!(!dest.exists());
        let mut part = dest.into_os_string();
        part.push(".part");
        assert!(!std::path::Path::new(&part).exists());
    }

    #[tokio::test]
    async fn resumes_from_an_existing_part_file() {
        let full: &'static [u8] = b"0123456789ABCDEFGHIJ";
        let already_have = &full[..10];
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("out.bin");
        let mut part = dest.as_os_str().to_owned();
        part.push(".part");
        tokio::fs::write(&part, already_have).await.unwrap();

        let url = serve_once(full);
        let outcome = download_resumable(
            &reqwest::Client::new(),
            &url,
            &dest,
            &sha256_hex(full),
            Arc::new(AtomicBool::new(false)),
            |_, _, _| {},
        )
        .await
        .unwrap();

        assert_eq!(outcome, DownloadOutcome::Completed);
        assert_eq!(tokio::fs::read(&dest).await.unwrap(), full);
    }

    #[tokio::test]
    async fn cancelling_mid_download_leaves_the_part_file_for_a_later_resume() {
        let body: &'static [u8] = b"some bytes that would be downloaded";
        let url = serve_once(body);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("out.bin");
        let cancel = Arc::new(AtomicBool::new(true)); // cancel before the first chunk is read

        let outcome = download_resumable(
            &reqwest::Client::new(),
            &url,
            &dest,
            &sha256_hex(body),
            cancel,
            |_, _, _| {},
        )
        .await
        .unwrap();

        assert_eq!(outcome, DownloadOutcome::Cancelled);
        assert!(!dest.exists());
    }
}

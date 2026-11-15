//! Model downloads: the only network traffic hams ever produces.

use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use reqwest::header::RANGE;
use reqwest::StatusCode;

/// Models are large and connections drop; each retry carries on from the bytes already saved.
const ATTEMPTS: u32 = 8;

enum Failure {
    /// Worth another try: the connection dropped or the server hiccuped.
    Retry(String),
    Fatal(String),
}

/// Streams `url` to `dest`, reporting progress as a fraction 0...1. The file only
/// appears at `dest` once it is complete; an interrupted download leaves a `.part`
/// file that the next call resumes.
pub fn fetch(url: &str, dest: &Path, on_progress: impl Fn(f64)) -> Result<(), String> {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .timeout(None) // models run to gigabytes
        .build()
        .map_err(|e| e.to_string())?;
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    let partial = dest.with_file_name(name);

    let mut last_error = String::new();
    for attempt in 0..ATTEMPTS {
        if attempt > 0 {
            std::thread::sleep(Duration::from_secs(2));
        }
        match fetch_once(&client, url, &partial, &on_progress) {
            Ok(()) => {
                let _ = fs::remove_file(dest);
                return fs::rename(&partial, dest).map_err(|e| e.to_string());
            }
            Err(Failure::Retry(why)) => last_error = why,
            Err(Failure::Fatal(why)) => {
                let _ = fs::remove_file(&partial);
                return Err(why);
            }
        }
    }
    Err(last_error)
}

fn fetch_once(
    client: &reqwest::blocking::Client,
    url: &str,
    partial: &Path,
    on_progress: &impl Fn(f64),
) -> Result<(), Failure> {
    let have = fs::metadata(partial).map(|m| m.len()).unwrap_or(0);
    let mut request = client.get(url);
    if have > 0 {
        request = request.header(RANGE, format!("bytes={have}-"));
    }
    let mut response = request.send().map_err(|e| Failure::Retry(e.to_string()))?;

    let status = response.status();
    let (mut written, mut file) = match status {
        // The server is continuing from where the partial file ends.
        StatusCode::PARTIAL_CONTENT => {
            let file = OpenOptions::new().append(true).open(partial).map_err(|e| Failure::Fatal(e.to_string()))?;
            (have, file)
        }
        // A full response: start over, whatever was there.
        StatusCode::OK => {
            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(partial)
                .map_err(|e| Failure::Fatal(e.to_string()))?;
            (0, file)
        }
        // "Nothing after that offset": the partial file is not what the server has. Start clean.
        StatusCode::RANGE_NOT_SATISFIABLE => {
            let _ = fs::remove_file(partial);
            return Err(Failure::Retry("download restarted".into()));
        }
        s if s.is_server_error() || s == StatusCode::TOO_MANY_REQUESTS => {
            return Err(Failure::Retry(format!("Server returned {}", s.as_u16())));
        }
        s => return Err(Failure::Fatal(format!("Server returned {}", s.as_u16()))),
    };
    let total = response.content_length().map(|rest| written + rest).unwrap_or(0);

    let mut buffer = vec![0u8; 256 * 1024];
    let mut last_report = Instant::now();
    loop {
        let n = response.read(&mut buffer).map_err(|e| Failure::Retry(e.to_string()))?;
        if n == 0 {
            break;
        }
        file.write_all(&buffer[..n]).map_err(|e| Failure::Fatal(e.to_string()))?;
        written += n as u64;
        if total > 0 && last_report.elapsed() >= Duration::from_millis(150) {
            on_progress(written as f64 / total as f64);
            last_report = Instant::now();
        }
    }
    file.flush().map_err(|e| Failure::Fatal(e.to_string()))?;
    if total > 0 && written != total {
        return Err(Failure::Retry("Download was cut short".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs the network, so it only runs on request: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn resumes_from_a_partial_file() {
        let url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en-q5_1.bin";
        let dir = std::env::temp_dir().join(format!("hams-fetch-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let whole = dir.join("whole.bin");
        let resumed = dir.join("resumed.bin");

        fetch(url, &whole, |_| {}).unwrap();
        let reference = fs::read(&whole).unwrap();
        assert!(reference.len() > 1_000_000);

        // Pretend an earlier attempt died a third of the way in.
        fs::write(dir.join("resumed.bin.part"), &reference[..reference.len() / 3]).unwrap();
        let mut first_report = None;
        let seen = std::cell::Cell::new(None);
        fetch(url, &resumed, |f| {
            if seen.get().is_none() {
                seen.set(Some(f));
            }
        })
        .unwrap();
        first_report = first_report.or(seen.get());
        assert_eq!(fs::read(&resumed).unwrap(), reference, "resumed file differs from a clean download");
        assert!(first_report.is_some_and(|f| f > 0.3), "progress should start past the saved third: {first_report:?}");
        assert!(!dir.join("resumed.bin.part").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}

//! Talks to the `hams-polish` helper process, which cleans up long dictations with a
//! small local language model. See `crates/polish` for why it is a separate program.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;
use std::time::Duration;

use serde::Deserialize;

/// Loading a model from disk; generous because the first load is cold.
const LOAD_TIMEOUT: Duration = Duration::from_secs(120);
/// Past this the dictation is delivered without polish rather than keep the user waiting.
const POLISH_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Deserialize)]
struct Reply {
    #[serde(default)]
    ready: bool,
    text: Option<String>,
    error: Option<String>,
}

struct Running {
    child: Child,
    stdin: ChildStdin,
    replies: Receiver<String>,
    model: String,
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub struct Polisher {
    running: Mutex<Option<Running>>,
}

/// The helper sits next to the hams executable, both in a bundle and in `target/`.
fn helper_path() -> Option<PathBuf> {
    let name = if cfg!(windows) { "hams-polish.exe" } else { "hams-polish" };
    let path = std::env::current_exe().ok()?.parent()?.join(name);
    path.is_file().then_some(path)
}

fn read_reply(replies: &Receiver<String>, timeout: Duration) -> Result<Reply, String> {
    let line = replies.recv_timeout(timeout).map_err(|_| "the cleanup model did not answer in time".to_owned())?;
    serde_json::from_str(&line).map_err(|e| format!("unreadable reply from the cleanup model ({e})"))
}

impl Polisher {
    pub fn new() -> Self {
        Polisher { running: Mutex::new(None) }
    }

    /// Starts the helper with `model_path` and waits until the model is in memory.
    pub fn load(&self, model_path: &str) -> Result<(), String> {
        let mut running = self.running.lock().unwrap_or_else(|e| e.into_inner());
        if running.as_ref().is_some_and(|r| r.model == model_path) {
            return Ok(());
        }
        *running = None; // stop the previous model before loading the next

        let helper = helper_path().ok_or("the hams-polish helper is missing from this build")?;
        let mut child = Command::new(helper)
            .args(["--model", model_path])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("could not start the cleanup model ({e})"))?;
        let stdin = child.stdin.take().ok_or("no pipe to the cleanup model")?;
        let stdout = child.stdout.take().ok_or("no pipe from the cleanup model")?;

        // Reading happens on its own thread so every wait can have a time limit.
        let (lines, replies) = channel::<String>();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if lines.send(line).is_err() {
                    break;
                }
            }
        });

        let started = Running { child, stdin, replies, model: model_path.to_owned() };
        let hello = read_reply(&started.replies, LOAD_TIMEOUT)?;
        if !hello.ready {
            return Err(hello.error.unwrap_or_else(|| "the cleanup model failed to start".into()));
        }
        *running = Some(started);
        Ok(())
    }

    pub fn stop(&self) {
        *self.running.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// Returns the cleaned-up text. Any failure stops the helper so a stuck model
    /// cannot hold up the next dictation; the caller falls back to the unpolished text.
    pub fn polish(&self, text: &str, glossary: &[String]) -> Result<String, String> {
        let mut running = self.running.lock().unwrap_or_else(|e| e.into_inner());
        let active = running.as_mut().ok_or("the cleanup model is not loaded")?;
        let request = serde_json::json!({ "text": text, "glossary": glossary }).to_string();
        let result = writeln!(active.stdin, "{request}")
            .and_then(|()| active.stdin.flush())
            .map_err(|e| format!("lost the cleanup model ({e})"))
            .and_then(|()| read_reply(&active.replies, POLISH_TIMEOUT))
            .and_then(|reply| match (reply.text, reply.error) {
                (Some(text), _) => Ok(text),
                (None, Some(error)) => Err(error),
                (None, None) => Err("empty reply from the cleanup model".into()),
            });
        // An error reply ("too long") leaves the helper healthy; a timeout or broken pipe does not.
        if let Err(why) = &result {
            if why.contains("in time") || why.contains("lost") || why.contains("unreadable") {
                *running = None;
            }
        }
        result
    }

    pub fn is_loaded(&self) -> bool {
        self.running.lock().unwrap_or_else(|e| e.into_inner()).is_some()
    }
}

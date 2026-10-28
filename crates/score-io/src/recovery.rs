//! A single writer owns the recovery file. Ordinary saves never target it.
use super::{FileError, MAX_FILE_BYTES, decode, encode, error};
use score_core::Score;
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveredScore {
    pub score: Score,
    pub original_path: Option<PathBuf>,
}
#[derive(Serialize, Deserialize)]
struct Envelope {
    recovery_version: u32,
    original_path: Option<PathBuf>,
    document: serde_json::Value,
}

enum Message {
    Update(Option<Box<RecoveredScore>>),
    Flush(mpsc::SyncSender<Result<(), String>>),
    Stop,
}
#[derive(Default)]
struct WriteStatus {
    error: Option<String>,
    reported: bool,
}
struct Writer {
    sender: mpsc::Sender<Message>,
    error: Arc<Mutex<WriteStatus>>,
    thread: Mutex<Option<thread::JoinHandle<()>>>,
}
impl Drop for Writer {
    fn drop(&mut self) {
        let _ = self.sender.send(Message::Stop);
        if let Some(thread) = self.thread.get_mut().unwrap().take() {
            let _ = thread.join();
        }
    }
}
#[derive(Clone)]
pub struct Recovery(Arc<Writer>);
impl Recovery {
    /// Lock ownership lasts until the last handle and the writer have stopped.
    pub fn open(path: &Path) -> Result<Self, FileError> {
        let parent = path
            .parent()
            .ok_or_else(|| error("Ruta de recuperación inválida"))?;
        std::fs::create_dir_all(parent).map_err(error)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path.with_extension("lock"))
            .map_err(error)?;
        lock_file(&lock)?;
        let (sender, receiver) = mpsc::channel();
        let status = Arc::new(Mutex::new(WriteStatus::default()));
        let worker_status = status.clone();
        let path = path.to_owned();
        let thread = thread::Builder::new()
            .name("levare-recovery".into())
            .spawn(move || {
                let _lock = lock;
                let mut pending = None;
                let mut first = Instant::now();
                let mut last = first;
                loop {
                    let message = if pending.is_some() {
                        let now = Instant::now();
                        let deadline =
                            (last + Duration::from_millis(750)).min(first + Duration::from_secs(3));
                        receiver.recv_timeout(deadline.saturating_duration_since(now))
                    } else {
                        receiver
                            .recv()
                            .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
                    };
                    match message {
                        Ok(Message::Update(value)) => {
                            if pending.is_none() {
                                first = Instant::now();
                            }
                            last = Instant::now();
                            pending = Some(value.map(|value| *value));
                            // Clearing must follow any older writes, and must not wait for more edits.
                            if matches!(pending, Some(None))
                                || last.duration_since(first) >= Duration::from_secs(3)
                            {
                                persist_pending(&path, &mut pending, &worker_status);
                            }
                        }
                        Ok(Message::Flush(reply)) => {
                            persist_pending(&path, &mut pending, &worker_status);
                            let result = worker_status
                                .lock()
                                .unwrap()
                                .error
                                .clone()
                                .map_or(Ok(()), Err);
                            let _ = reply.send(result);
                        }
                        Ok(Message::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                            persist_pending(&path, &mut pending, &worker_status);
                            break;
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            persist_pending(&path, &mut pending, &worker_status)
                        }
                    }
                }
            })
            .map_err(error)?;
        Ok(Self(Arc::new(Writer {
            sender,
            error: status,
            thread: Mutex::new(Some(thread)),
        })))
    }
    pub fn update(&self, value: Option<RecoveredScore>) {
        let _ = self.0.sender.send(Message::Update(value.map(Box::new)));
    }
    pub fn flush(&self) -> Result<(), FileError> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.0.sender.send(Message::Flush(sender)).map_err(error)?;
        receiver
            .recv_timeout(Duration::from_secs(6))
            .map_err(error)?
            .map_err(error)
    }
    pub fn last_error(&self) -> Option<String> {
        self.0.error.lock().unwrap().error.clone()
    }
    pub fn take_error(&self) -> Option<String> {
        let mut status = self.0.error.lock().unwrap();
        if status.reported {
            return None;
        }
        status.reported = true;
        status.error.clone()
    }
}
fn persist_pending(
    path: &Path,
    pending: &mut Option<Option<RecoveredScore>>,
    status: &Mutex<WriteStatus>,
) {
    let Some(value) = pending.take() else {
        return;
    };
    let result = match value {
        Some(value) => write(path, &value),
        None => match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(error(e)),
        },
    };
    *status.lock().unwrap() = WriteStatus {
        error: result.err().map(|e| e.to_string()),
        reported: false,
    };
}
/// Exclusive and non-blocking, on a separate `.lock` file so the recovery file
/// itself can still be replaced and removed while the lock is held (Windows
/// refuses to rename over a file that is open with a lock). The OS releases it
/// when the handle closes, including when the process crashes.
fn lock_file(file: &File) -> Result<(), FileError> {
    match file.try_lock() {
        Ok(()) => Ok(()),
        Err(std::fs::TryLockError::WouldBlock) => Err(error(IN_USE)),
        Err(std::fs::TryLockError::Error(failure)) => Err(error(failure)),
    }
}
/// Shown after «Recuperación no disponible: ».
const IN_USE: &str = "otra ventana de Levare ya está usando el autoguardado";
fn write(path: &Path, value: &RecoveredScore) -> Result<(), FileError> {
    let document = serde_json::from_slice(&encode(&value.score)?).map_err(error)?;
    let bytes = serde_json::to_vec(&Envelope {
        recovery_version: 1,
        original_path: value.original_path.clone(),
        document,
    })
    .map_err(error)?;
    atomic_write::write(path, &bytes).map_err(error)
}
pub fn load_recovery(path: &Path) -> Result<Option<RecoveredScore>, FileError> {
    let mut bytes = Vec::new();
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(error(e)),
    };
    file.take((MAX_FILE_BYTES * 2 + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(error)?;
    if bytes.len() > MAX_FILE_BYTES * 2 {
        return Err(error("Recuperación demasiado grande"));
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(error)?;
    if value.get("recovery_version").is_none() {
        return decode(&bytes).map(|score| {
            Some(RecoveredScore {
                score,
                original_path: None,
            })
        });
    }
    let envelope: Envelope = serde_json::from_value(value).map_err(error)?;
    if envelope.recovery_version != 1 {
        return Err(error("Versión de recuperación no compatible"));
    }
    let score = decode(&serde_json::to_vec(&envelope.document).map_err(error)?)?;
    Ok(Some(RecoveredScore {
        score,
        original_path: envelope.original_path,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn debounced_writer_recovers_and_clear_cannot_resurrect_an_old_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recovery.partitura");
        let writer = Recovery::open(&path).unwrap();
        let second = Recovery::open(&path).err().expect("one writer at a time");
        assert_eq!(second.to_string(), IN_USE);
        let mut score = Score::default();
        for _ in 0..20 {
            score.append_measure().unwrap();
            writer.update(Some(RecoveredScore {
                score: score.clone(),
                original_path: Some(dir.path().join("original.partitura")),
            }));
        }
        thread::sleep(Duration::from_millis(950));
        let restored = load_recovery(&path).unwrap().unwrap();
        assert_eq!(restored.score, score);
        assert_eq!(
            restored.original_path.unwrap(),
            dir.path().join("original.partitura")
        );
        writer.update(Some(RecoveredScore {
            score,
            original_path: None,
        }));
        writer.update(None);
        writer.flush().unwrap();
        assert_eq!(load_recovery(&path).unwrap(), None);
        drop(writer);
        assert!(Recovery::open(&path).is_ok());
    }
    #[test]
    fn a_killed_process_leaves_an_automatic_recovery_without_a_quit_handler() {
        const ENV: &str = "LEVARE_CRASH_TEST_PATH";
        if let Some(path) = std::env::var_os(ENV) {
            let recovery = Recovery::open(Path::new(&path)).unwrap();
            let mut score = Score::default();
            score.append_measure().unwrap();
            recovery.update(Some(RecoveredScore {
                score,
                original_path: None,
            }));
            loop {
                thread::sleep(Duration::from_secs(1));
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("crash.partitura");
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "recovery::tests::a_killed_process_leaves_an_automatic_recovery_without_a_quit_handler"])
            .env(ENV, &path).stdout(std::process::Stdio::null()).spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !path.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(30));
        }
        child.kill().unwrap();
        child.wait().unwrap();
        let restored = load_recovery(&path)
            .unwrap()
            .expect("periodic recovery before forced exit");
        assert_eq!(
            restored.score.measures().len(),
            Score::default().measures().len() + 1
        );
        // Windows may release a dead process's locks a moment after it exits.
        let deadline = Instant::now() + Duration::from_secs(2);
        let reopened = loop {
            match Recovery::open(&path) {
                Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(30)),
                result => break result,
            }
        };
        assert!(reopened.is_ok(), "crash releases recovery ownership");
    }
    #[test]
    fn legacy_recovery_and_write_failures_preserve_the_previous_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recovery.partitura");
        super::super::save(&path, &Score::default()).unwrap();
        assert_eq!(
            load_recovery(&path).unwrap().unwrap().score,
            Score::default()
        );
        let writer = Recovery::open(&path).unwrap();
        let mut invalid = serde_json::to_value(Score::default()).unwrap();
        invalid["title"] = serde_json::Value::String("x".repeat(2048));
        writer.update(Some(RecoveredScore {
            score: serde_json::from_value(invalid).unwrap(),
            original_path: None,
        }));
        assert!(writer.flush().is_err());
        assert_eq!(
            load_recovery(&path).unwrap().unwrap().score,
            Score::default()
        );
        assert!(writer.take_error().is_some());
    }
}

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
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
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
        std::fs::create_dir_all(parent)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path.with_extension("lock"))?;
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
            })?;
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
        self.0
            .sender
            .send(Message::Flush(sender))
            .map_err(FileError::internal)?;
        receiver
            .recv_timeout(Duration::from_secs(6))
            .map_err(FileError::internal)?
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
            Err(e) => Err(e.into()),
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
        Err(std::fs::TryLockError::Error(failure)) => Err(failure.into()),
    }
}
/// Shown after «Recuperación no disponible: ».
const IN_USE: &str = "otra ventana de Levare ya está usando el autoguardado";
fn write(path: &Path, value: &RecoveredScore) -> Result<(), FileError> {
    let document = serde_json::from_slice(&encode(&value.score)?).map_err(FileError::internal)?;
    let bytes = serde_json::to_vec(&Envelope {
        recovery_version: 1,
        original_path: value.original_path.clone(),
        document,
    })
    .map_err(FileError::internal)?;
    Ok(atomic_write::write(path, &bytes)?)
}
/// The document plus its envelope and original path.
const MAX_RECOVERY_BYTES: usize = MAX_FILE_BYTES + 1024 * 1024;
pub fn load_recovery(path: &Path) -> Result<Option<RecoveredScore>, FileError> {
    let mut bytes = Vec::new();
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    file.take((MAX_RECOVERY_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_RECOVERY_BYTES {
        return Err(FileError::too_large("Recuperación demasiado grande"));
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(FileError::damaged)?;
    if value.get("recovery_version").is_none() {
        return decode(&bytes).map(|score| {
            Some(RecoveredScore {
                score,
                original_path: None,
            })
        });
    }
    let envelope: Envelope = serde_json::from_value(value).map_err(FileError::damaged)?;
    if envelope.recovery_version != 1 {
        return Err(error("Versión de recuperación no compatible"));
    }
    let score = decode(&serde_json::to_vec(&envelope.document).map_err(FileError::internal)?)?;
    Ok(Some(RecoveredScore {
        score,
        original_path: envelope.original_path,
    }))
}

/// Moves a recovery file that could not be restored out of the writer's way,
/// before the next autosave replaces it or opening a score deletes it: it may
/// be the only copy of unsaved work. It stays in the same folder, under a dated
/// name nothing in Levare writes to.
pub fn set_aside_recovery(path: &Path) -> Result<PathBuf, FileError> {
    let folder = path
        .parent()
        .ok_or_else(|| error("Ruta de recuperación inválida"))?;
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("partitura");
    let stamp = timestamp(SystemTime::now());
    let kept = (1..)
        .map(|copy| {
            folder.join(dated_name(
                "Recuperación sin abrir",
                &stamp,
                copy,
                extension,
            ))
        })
        .find(|candidate| !candidate.exists())
        .expect("an unused name");
    std::fs::rename(path, &kept)?;
    Ok(kept)
}

fn dated_name(label: &str, stamp: &str, copy: u32, extension: &str) -> String {
    match copy {
        1 => format!("{label} {stamp}.{extension}"),
        copy => format!("{label} {stamp} ({copy}).{extension}"),
    }
}

/// UTC, as `2026-10-28 15-30-12`: colons are not allowed in Windows file names.
fn timestamp(now: SystemTime) -> String {
    let seconds = now
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let (days, time) = ((seconds / 86_400) as i64, seconds % 86_400);
    // Howard Hinnant's civil_from_days.
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}-{:02}-{:02}",
        time / 3_600,
        time / 60 % 60,
        time % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_unreadable_recovery_is_kept_aside_from_the_next_autosave() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recovery.partitura");
        let unreadable = br#"{"recovery_version": 1, "document": {"#.to_vec();
        std::fs::write(&path, &unreadable).unwrap();
        let writer = Recovery::open(&path).unwrap();
        assert!(load_recovery(&path).is_err());
        let kept = set_aside_recovery(&path).unwrap();
        assert_eq!(kept.parent(), Some(dir.path()));
        // The first edit autosaves, and opening a score clears the autosave.
        writer.update(Some(RecoveredScore {
            score: Score::default(),
            original_path: None,
        }));
        writer.flush().unwrap();
        assert_eq!(
            load_recovery(&path).unwrap().unwrap().score,
            Score::default()
        );
        writer.update(None);
        writer.flush().unwrap();
        assert_eq!(std::fs::read(&kept).unwrap(), unreadable);
        std::fs::write(&path, b"{").unwrap();
        let second = set_aside_recovery(&path).unwrap();
        assert_ne!(second, kept);
        assert_eq!(std::fs::read(&kept).unwrap(), unreadable);
        assert_eq!(std::fs::read(&second).unwrap(), b"{");
    }
    #[test]
    fn set_aside_names_carry_the_utc_date_and_time() {
        assert_eq!(timestamp(UNIX_EPOCH), "1970-01-01 00-00-00");
        let later = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        assert_eq!(timestamp(later), "2023-11-14 22-13-20");
        let leap_day = UNIX_EPOCH + Duration::from_secs(951_782_400);
        assert_eq!(timestamp(leap_day), "2000-02-29 00-00-00");
    }
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
        // Before the recovery envelope, the autosave was an indented native file.
        let legacy = serde_json::to_vec_pretty(&super::super::NativeFile {
            format: "partitura".into(),
            version: super::super::SCHEMA_VERSION,
            score: Score::default(),
        })
        .unwrap();
        std::fs::write(&path, legacy).unwrap();
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

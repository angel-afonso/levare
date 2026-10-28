//! A log and crash reports in Levare's data folder.
//!
//! Release builds have no console on Windows, and on macOS one opened from
//! Finder only reaches the unified log, so stderr alone loses every failure.
//! Lines also go to stderr for whoever does have a terminal.
use crate::app_paths::data_path;
use gpui::{AnyWindowHandle, App, AppContext, PromptLevel};
use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    fs::{self, OpenOptions},
    io::{self, Write},
    panic::PanicHookInfo,
    path::{Path, PathBuf},
    sync::{Mutex, PoisonError},
    time::{SystemTime, UNIX_EPOCH},
};

const LOG: &str = "registro.log";
const OLD_LOG: &str = "registro.1.log";
/// Past this the log becomes `registro.1.log`, replacing the one before.
const LOG_LIMIT: u64 = 1024 * 1024;
const REPORT_PREFIX: &str = "fallo-";
const REPORT_SUFFIX: &str = ".txt";
const KEPT_REPORTS: usize = 10;
/// The newest report the musician was already told about.
const SEEN: &str = "fallos.json";

/// Rotation and appending happen under it, so lines from several threads
/// never interleave and two threads never rotate at once.
static LOG_LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize)]
struct Seen {
    visto: u64,
}

/// Writes a timestamped line to the log and stderr. Never fails or panics:
/// a log that cannot be written must not take Levare down with it.
pub fn log(message: impl Display) {
    let message = message.to_string();
    // Not eprintln!, which panics when stderr is a closed pipe.
    let _ = writeln!(io::stderr(), "levare: {message}");
    // Tests that reach a failure path must not write to the musician's log.
    if cfg!(test) {
        return;
    }
    if let Some(directory) = directory() {
        let _ = append(&directory, &line(now(), &message));
    }
}

/// Installs the panic hook that writes a crash report before the default
/// hook runs. Call it first thing, before GPUI starts.
pub fn install() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        record_panic(info);
        default(info);
    }));
    log(format_args!(
        "Levare {} starts on {}/{}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
}

/// The window could not open (no usable GPU, a broken driver…): keep the
/// reason, tell the musician where a console cannot, and exit with an error.
pub fn window_failed(error: impl Display) -> ! {
    let error = format!("{error:#}");
    log(format_args!("The window could not open: {error}"));
    let report = write_report(&report_text(
        "main",
        &format!("the window could not open: {error}"),
        "Application::open_window",
    ));
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
        use windows::core::HSTRING;
        let mut text = format!("Levare no pudo abrir su ventana.\n\n{error}");
        if let Some(report) = &report {
            text.push_str(&format!(
                "\n\nSe guardó un informe del fallo en {}",
                report.display()
            ));
        }
        // SAFETY: both strings outlive the call; no owner window is needed.
        unsafe {
            MessageBoxW(
                None,
                &HSTRING::from(text),
                &HSTRING::from("Levare"),
                MB_OK | MB_ICONERROR,
            );
        }
    }
    // macOS and Linux: the log and stderr already have the reason.
    #[cfg(not(target_os = "windows"))]
    let _ = report;
    std::process::exit(1);
}

/// Tells the musician once about a crash report newer than the last one they
/// were told about, and offers to show it.
pub fn announce_unseen_crash(window: AnyWindowHandle, cx: &mut App) {
    let Some(directory) = directory() else {
        return;
    };
    let seen = fs::read_to_string(directory.join(SEEN))
        .ok()
        .and_then(|text| serde_json::from_str::<Seen>(&text).ok())
        .map(|seen| seen.visto);
    let Some(time) = unseen(&report_times(&directory), seen) else {
        return;
    };
    // Marked before asking, so a crash while the prompt is open does not
    // bring the same report back.
    if let Ok(text) = serde_json::to_string_pretty(&Seen { visto: time }) {
        let _ = fs::write(directory.join(SEEN), text + "\n");
    }
    let report = directory.join(report_name(time));
    let _ = cx.update_window(window, |_, window, cx| {
        let answer = window.prompt(
            PromptLevel::Warning,
            "Levare se cerró inesperadamente",
            Some(
                "Se guardó un informe del fallo en la carpeta de datos de Levare. Si vuelve \
                 a ocurrir, envíalo con una descripción de lo que estabas haciendo.",
            ),
            &["Mostrar el informe", "Cerrar"],
            cx,
        );
        cx.spawn(async move |cx| {
            if answer.await == Ok(0) {
                let _ = cx.update(|cx| cx.reveal_path(&report));
            }
        })
        .detach();
    });
}

fn directory() -> Option<PathBuf> {
    data_path(LOG).and_then(|log| log.parent().map(Path::to_path_buf))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

fn line(time: u64, message: &str) -> String {
    format!("[{}] {message}\n", utc(time))
}

fn append(directory: &Path, line: &str) -> io::Result<()> {
    let _guard = LOG_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    fs::create_dir_all(directory)?;
    let log = directory.join(LOG);
    rotate(&log, &directory.join(OLD_LOG), LOG_LIMIT)?;
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)?
        .write_all(line.as_bytes())
}

/// Moves a log that reached `limit` bytes to `old`. Returns whether it did.
fn rotate(log: &Path, old: &Path, limit: u64) -> io::Result<bool> {
    match fs::metadata(log) {
        Ok(metadata) if metadata.len() >= limit => fs::rename(log, old).map(|()| true),
        _ => Ok(false),
    }
}

fn record_panic(info: &PanicHookInfo) {
    let thread = std::thread::current();
    let thread = thread.name().unwrap_or("unnamed");
    let message = info
        .payload()
        .downcast_ref::<&str>()
        .map(|message| (*message).to_owned())
        .or_else(|| info.payload().downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "(no message)".to_owned());
    let location = info
        .location()
        .map_or_else(|| "unknown".to_owned(), ToString::to_string);
    let report = write_report(&report_text(thread, &message, &location));
    log(format_args!(
        "Panic in thread '{thread}' at {location}: {message}; report: {}",
        report
            .as_deref()
            .map_or_else(|| "not written".into(), |path| path.display().to_string())
    ));
}

fn report_text(thread: &str, message: &str, location: &str) -> String {
    format!(
        "Informe de fallo de Levare. Si el problema se repite, envía este archivo con una \
         descripción de lo que estabas haciendo.\n\n\
         Version: {}\nSystem: {}/{}\nTime: {}\nThread: {thread}\nLocation: {location}\n\
         Message: {message}\n\nBacktrace:\n{}\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        utc(now()),
        std::backtrace::Backtrace::force_capture(),
    )
}

/// Writes the report and drops the oldest ones past `KEPT_REPORTS`. Two
/// panics in the same second share a report.
fn write_report(text: &str) -> Option<PathBuf> {
    let directory = directory()?;
    fs::create_dir_all(&directory).ok()?;
    let path = directory.join(report_name(now()));
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| file.write_all(text.as_bytes()))
        .ok()?;
    for time in to_prune(report_times(&directory), KEPT_REPORTS) {
        let _ = fs::remove_file(directory.join(report_name(time)));
    }
    Some(path)
}

fn report_name(time: u64) -> String {
    format!("{REPORT_PREFIX}{time}{REPORT_SUFFIX}")
}

fn report_time(name: &str) -> Option<u64> {
    name.strip_prefix(REPORT_PREFIX)?
        .strip_suffix(REPORT_SUFFIX)?
        .parse()
        .ok()
}

fn report_times(directory: &Path) -> Vec<u64> {
    fs::read_dir(directory)
        .into_iter()
        .flatten()
        .filter_map(|entry| report_time(entry.ok()?.file_name().to_str()?))
        .collect()
}

/// The reports past the newest `keep`.
fn to_prune(mut times: Vec<u64>, keep: usize) -> Vec<u64> {
    times.sort_unstable_by(|a, b| b.cmp(a));
    times.split_off(keep.min(times.len()))
}

/// The newest report, when the musician has not been told about it yet.
fn unseen(times: &[u64], seen: Option<u64>) -> Option<u64> {
    let newest = times.iter().copied().max()?;
    seen.is_none_or(|seen| newest > seen).then_some(newest)
}

/// `2026-03-01 14:05:09 UTC`, without a date library: days to a civil date
/// as in Howard Hinnant's `civil_from_days`.
fn utc(time: u64) -> String {
    let (days, seconds) = (time / 86_400, time % 86_400);
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
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
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        seconds / 3_600,
        seconds % 3_600 / 60,
        seconds % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_are_utc_calendar_dates() {
        assert_eq!(utc(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(utc(951_782_400), "2000-02-29 00:00:00 UTC");
        assert_eq!(utc(1_772_373_909), "2026-03-01 14:05:09 UTC");
        assert_eq!(line(0, "hola"), "[1970-01-01 00:00:00 UTC] hola\n");
    }

    #[test]
    fn a_full_log_moves_aside_and_a_new_one_starts() {
        let directory = tempfile::tempdir().unwrap();
        let (log, old) = (directory.path().join(LOG), directory.path().join(OLD_LOG));
        assert!(!rotate(&log, &old, 10).unwrap());
        fs::write(&log, "123456789").unwrap();
        assert!(!rotate(&log, &old, 10).unwrap());
        fs::write(&old, "older").unwrap();
        fs::write(&log, "1234567890").unwrap();
        assert!(rotate(&log, &old, 10).unwrap());
        assert!(!log.exists());
        assert_eq!(fs::read_to_string(&old).unwrap(), "1234567890");
    }

    #[test]
    fn lines_append_to_the_log() {
        let directory = tempfile::tempdir().unwrap();
        let folder = directory.path().join("Levare");
        append(&folder, "uno\n").unwrap();
        append(&folder, "dos\n").unwrap();
        assert_eq!(fs::read_to_string(folder.join(LOG)).unwrap(), "uno\ndos\n");
    }

    #[test]
    fn report_names_carry_their_time() {
        assert_eq!(report_name(1_772_373_909), "fallo-1772373909.txt");
        assert_eq!(report_time("fallo-1772373909.txt"), Some(1_772_373_909));
        for other in ["registro.log", "fallo-.txt", "fallo-12.log", "fallos.json"] {
            assert_eq!(report_time(other), None, "{other}");
        }
    }

    #[test]
    fn only_the_newest_reports_are_kept() {
        assert_eq!(to_prune(vec![3, 1, 4, 5, 2], 3), vec![2, 1]);
        assert_eq!(to_prune(vec![1, 2], 10), Vec::<u64>::new());
    }

    #[test]
    fn a_report_is_announced_once() {
        assert_eq!(unseen(&[], None), None);
        assert_eq!(unseen(&[5, 9, 7], None), Some(9));
        assert_eq!(unseen(&[5, 9, 7], Some(7)), Some(9));
        assert_eq!(unseen(&[5, 9, 7], Some(9)), None);
        // Pruning never brings back an older report.
        assert_eq!(unseen(&[5], Some(9)), None);
    }

    #[test]
    fn reports_in_the_data_folder_are_found_by_name() {
        let directory = tempfile::tempdir().unwrap();
        for name in [
            "fallo-20.txt",
            "fallo-10.txt",
            "registro.log",
            "fallos.json",
        ] {
            fs::write(directory.path().join(name), "").unwrap();
        }
        let mut times = report_times(directory.path());
        times.sort_unstable();
        assert_eq!(times, vec![10, 20]);
    }
}

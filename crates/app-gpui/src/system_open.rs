//! Scores the system asks Levare to open: a double click in Finder, Explorer
//! or the Linux file manager, «Abrir con», or a path on the command line.
//! They go through the same steps as Archivo › Abrir, unsaved changes prompt
//! included.
//!
//! On Windows and Linux a file opened while Levare is running starts Levare
//! again; that launch hands the file to the running Levare ([`hand_off`],
//! [`serve`], through `crate::handoff`) instead of opening a second window
//! without autosave. macOS hands it over itself, as an open event.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures::StreamExt;
use futures::channel::mpsc::{UnboundedReceiver, unbounded};
use gpui::{App, Application, Timer};

use crate::diagnostics;
use crate::handoff::{self, Address, Listener, Outcome};
use crate::{AppView, Next};

/// How often a request waits for the window to be free (a save, a dialog or
/// an agent's turn under way).
const RETRY: Duration = Duration::from_millis(250);

/// Collects the files to open from the command line and, on macOS, from the
/// system's open events, which can arrive before the window exists.
pub fn listen(app: &Application) -> UnboundedReceiver<Vec<PathBuf>> {
    let (sender, receiver) = unbounded();
    let cwd = std::env::current_dir().unwrap_or_default();
    let paths = command_line_paths(std::env::args_os().skip(1), &cwd);
    if !paths.is_empty() {
        let _ = sender.unbounded_send(paths);
    }
    // Finder hands files over as file:// URLs, not as arguments. Windows and
    // Linux start Levare with the path in its arguments instead.
    app.on_open_urls(move |urls| {
        let paths: Vec<_> = urls.iter().filter_map(|url| file_url_path(url)).collect();
        if !paths.is_empty() {
            let _ = sender.unbounded_send(paths);
        }
    });
    receiver
}

/// On Windows and Linux, offers this launch's files to the Levare already
/// running with the same data folder. True when that Levare took them and
/// this process should exit; false to start as usual, which is also what
/// happens if it does not answer within [`handoff::TIMEOUT`].
pub fn hand_off() -> bool {
    if cfg!(target_os = "macos") {
        return false;
    }
    let Some(address) = address() else {
        return false;
    };
    let cwd = std::env::current_dir().unwrap_or_default();
    let paths = command_line_paths(std::env::args_os().skip(1), &cwd);
    match handoff::hand_off(&address, &paths, handoff::TIMEOUT) {
        Outcome::HandedOff => {
            diagnostics::log("Levare is already running: the files went to its window");
            true
        }
        Outcome::NoneRunning => false,
        Outcome::Failed(error) => {
            diagnostics::log(format_args!(
                "Could not hand the files to the running Levare ({error}); starting another"
            ));
            false
        }
    }
}

/// What later launches hand over to this Levare.
pub struct Handoffs {
    listener: Listener,
    requests: UnboundedReceiver<Vec<PathBuf>>,
}

/// On Windows and Linux, starts taking later launches' files. Only the
/// Levare that holds the autosave (`holds_recovery`) listens.
pub fn serve(holds_recovery: bool) -> Option<Handoffs> {
    if cfg!(target_os = "macos") || !holds_recovery {
        return None;
    }
    let address = address()?;
    let (sender, requests) = unbounded();
    match Listener::start(&address, move |paths| sender.unbounded_send(paths).is_ok()) {
        Ok(listener) => Some(Handoffs { listener, requests }),
        Err(error) => {
            diagnostics::log(format_args!(
                "Later launches will open their own window: cannot listen at {address}: {error}"
            ));
            None
        }
    }
}

fn address() -> Option<Address> {
    crate::recovery_path().map(|recovery| Address::for_recovery(&recovery))
}

/// Whether Levare was started with a score to open on its command line, as
/// Windows and Linux do on a double click. macOS hands its files over later.
pub fn launched_with_files() -> bool {
    !command_line_paths(std::env::args_os().skip(1), Path::new("")).is_empty()
}

/// Opens each request in Levare's window once it can take it. Levare holds
/// one score per window, so of several files at once the first one opens.
/// A launch handed over also brings the window to the front. Called once the
/// window is open.
pub fn deliver(
    requests: UnboundedReceiver<Vec<PathBuf>>,
    handoffs: Option<Handoffs>,
    cx: &mut App,
) {
    let Some(window) = cx
        .windows()
        .into_iter()
        .find_map(|window| window.downcast::<AppView>())
    else {
        return;
    };
    let handed = match handoffs {
        Some(Handoffs { listener, requests }) => {
            // A launch that arrives while Levare quits starts on its own.
            cx.on_app_quit(move |_| {
                listener.close();
                async {}
            })
            .detach();
            requests
        }
        None => unbounded().1,
    };
    let mut requests = futures::stream::select(
        requests.map(|paths| (paths, false)),
        handed.map(|paths| (paths, true)),
    );
    cx.spawn(async move |cx| {
        while let Some((paths, raise)) = requests.next().await {
            if raise {
                let _ = window.update(cx, |_, window, _| window.activate_window());
            }
            let Some(path) = paths.into_iter().next() else {
                continue;
            };
            loop {
                let opened = window.update(cx, |view, window, cx| {
                    view.close_about(cx);
                    if view.busy
                        || view.confirming
                        || view.agent_locked()
                        || view.instrument_picker.is_some()
                    {
                        return false;
                    }
                    view.request_next(Next::OpenPath(path.clone()), window, cx);
                    true
                });
                match opened {
                    Ok(true) => break,
                    Ok(false) => {
                        Timer::after(RETRY).await;
                    }
                    Err(_) => return,
                }
            }
        }
    })
    .detach();
}

/// The files among Levare's arguments, absolute. Flags are skipped, such as
/// the `-psn_…` process serial number older macOS versions add; after `--`
/// everything is a file.
fn command_line_paths(args: impl IntoIterator<Item = OsString>, cwd: &Path) -> Vec<PathBuf> {
    let mut only_files = false;
    let mut paths = Vec::new();
    for arg in args {
        if !only_files && arg == "--" {
            only_files = true;
        } else if only_files || !arg.to_string_lossy().starts_with('-') {
            paths.push(cwd.join(arg));
        }
    }
    paths
}

/// The local path of a `file://` URL, percent-decoded; `None` for anything
/// else, a remote host included.
fn file_url_path(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    let path = match rest.find('/') {
        Some(0) => rest,
        Some(slash) if &rest[..slash] == "localhost" => &rest[slash..],
        _ => return None,
    };
    let path = path.split(['?', '#']).next().unwrap_or(path);
    let decoded = String::from_utf8(percent_decode(path)?).ok()?;
    // file:///C:/Partituras/obra.levare is C:\Partituras\obra.levare.
    let decoded = if cfg!(windows) && decoded.as_bytes().get(2) == Some(&b':') {
        decoded[1..].to_owned()
    } else {
        decoded
    };
    Some(PathBuf::from(decoded))
}

fn percent_decode(text: &str) -> Option<Vec<u8>> {
    let mut bytes = Vec::with_capacity(text.len());
    let mut rest = text.bytes();
    while let Some(byte) = rest.next() {
        if byte == b'%' {
            let high = (rest.next()? as char).to_digit(16)?;
            let low = (rest.next()? as char).to_digit(16)?;
            bytes.push((high * 16 + low) as u8);
        } else {
            bytes.push(byte);
        }
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn the_command_line_gives_the_files_and_skips_flags() {
        let cwd = Path::new("/home/ana/obras");
        assert_eq!(
            command_line_paths(
                args(&[
                    "-psn_0_1234567",
                    "obra.levare",
                    "--verbose",
                    "/tmp/coral.musicxml"
                ]),
                cwd
            ),
            vec![
                cwd.join("obra.levare"),
                PathBuf::from("/tmp/coral.musicxml")
            ]
        );
        assert!(command_line_paths(args(&[]), cwd).is_empty());
        assert!(command_line_paths(args(&["-psn_0_1"]), cwd).is_empty());
    }

    #[test]
    fn after_a_double_dash_every_argument_is_a_file() {
        let cwd = Path::new("/home/ana");
        assert_eq!(
            command_line_paths(args(&["--", "-raro.partitura", "--"]), cwd),
            vec![cwd.join("-raro.partitura"), cwd.join("--")]
        );
    }

    #[test]
    fn file_urls_become_local_paths() {
        assert_eq!(
            file_url_path("file:///Users/ana/Mi%20obra%20n%C2%BA%201.levare"),
            Some(PathBuf::from("/Users/ana/Mi obra nº 1.levare"))
        );
        assert_eq!(
            file_url_path("file://localhost/Users/ana/coral.mxl"),
            Some(PathBuf::from("/Users/ana/coral.mxl"))
        );
        assert_eq!(
            file_url_path("file:///Users/ana/vals.partitura?x=1"),
            Some(PathBuf::from("/Users/ana/vals.partitura"))
        );
    }

    #[test]
    fn other_urls_are_not_files() {
        assert_eq!(file_url_path("https://levare.app/obra.levare"), None);
        assert_eq!(file_url_path("file://servidor/obras/vals.levare"), None);
        assert_eq!(file_url_path("file:///roto%2"), None);
        assert_eq!(file_url_path("file:///roto%zz"), None);
        assert_eq!(file_url_path("file:///no-utf8%ff"), None);
    }

    #[cfg(windows)]
    #[test]
    fn a_windows_file_url_keeps_its_drive() {
        assert_eq!(
            file_url_path("file:///C:/Partituras/obra.levare"),
            Some(PathBuf::from("C:/Partituras/obra.levare"))
        );
    }
}

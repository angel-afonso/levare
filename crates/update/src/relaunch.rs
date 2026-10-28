//! Starting the updated app on Windows and Linux.
//!
//! GPUI 0.2.2's own restart pastes the executable's path into a shell
//! script: unquoted in bash on Linux, so a folder with a space breaks it, and
//! inside a double-quoted PowerShell string on Windows, so a `$` or a
//! backtick in the path breaks it. Here the path reaches the waiting script
//! as an argument on Linux and as a single-quoted literal on Windows.

use std::path::Path;
use std::process::{Command, Stdio};

/// Starts `executable` once this process has exited, so the new instance
/// does not find the old one still holding the autosave. The caller quits
/// right after.
pub fn relaunch_after_exit(executable: &Path) -> std::io::Result<()> {
    let mut command = waiter(std::process::id(), executable);
    if let Some(dir) = executable.parent() {
        command.current_dir(dir);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command.spawn().map(drop)
}

#[cfg(windows)]
fn waiter(pid: u32, executable: &Path) -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    // The system's PowerShell, not one that happens to be on PATH or beside
    // Levare.
    let powershell = std::env::var_os("SystemRoot")
        .map(|root| Path::new(&root).join(r"System32\WindowsPowerShell\v1.0\powershell.exe"))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| "powershell.exe".into());
    let mut command = Command::new(powershell);
    command
        .args(["-NoProfile", "-NonInteractive", "-Command"])
        .arg(powershell_script(pid, executable))
        .creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(not(windows))]
fn waiter(pid: u32, executable: &Path) -> Command {
    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg(r#"while kill -0 "$0" 2>/dev/null; do sleep 0.1; done; exec "$1""#)
        .arg(pid.to_string())
        .arg(executable);
    // Its own process group, so closing the terminal Levare was started
    // from does not take the new instance with it.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
}

/// Waits for `pid` to exit, then starts `executable`.
#[cfg_attr(not(windows), allow(dead_code))]
fn powershell_script(pid: u32, executable: &Path) -> String {
    let dir = executable.parent().unwrap_or(executable);
    format!(
        "Wait-Process -Id {pid} -ErrorAction SilentlyContinue; \
         Start-Process -FilePath {} -WorkingDirectory {}",
        powershell_literal(&executable.to_string_lossy()),
        powershell_literal(&dir.to_string_lossy()),
    )
}

/// `text` as a PowerShell single-quoted string, which expands nothing. Its
/// only escape is doubling a quote, and PowerShell also takes the
/// typographic single quotes as quotes.
#[cfg_attr(not(windows), allow(dead_code))]
fn powershell_literal(text: &str) -> String {
    let mut literal = String::with_capacity(text.len() + 2);
    literal.push('\'');
    for c in text.chars() {
        if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}') {
            literal.push(c);
        }
        literal.push(c);
    }
    literal.push('\'');
    literal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_paths_for_powershell() {
        assert_eq!(
            powershell_literal(r"C:\Levare\levare.exe"),
            r"'C:\Levare\levare.exe'"
        );
        assert_eq!(
            powershell_literal(r"C:\Users\O'Brien $x `n\levare.exe"),
            r"'C:\Users\O''Brien $x `n\levare.exe'"
        );
        assert_eq!(powershell_literal("a\u{2019}b"), "'a\u{2019}\u{2019}b'");
    }

    #[cfg(windows)]
    #[test]
    fn the_powershell_script_waits_then_starts_levare() {
        assert_eq!(
            powershell_script(42, Path::new(r"C:\Mis programas\Levare\levare.exe")),
            r"Wait-Process -Id 42 -ErrorAction SilentlyContinue; Start-Process -FilePath 'C:\Mis programas\Levare\levare.exe' -WorkingDirectory 'C:\Mis programas\Levare'"
        );
    }

    /// The waiter starts the program only after the process it waits for has
    /// gone.
    #[cfg(unix)]
    #[test]
    fn starts_the_program_after_the_process_exits() {
        use crate::verify::tests::tempdir;
        let dir = tempdir::Dir::new();
        let marker = dir.path().join("started");
        let program = dir.path().join("new levare");
        std::fs::write(
            &program,
            format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();

        let mut old = Command::new("/bin/sleep").arg("1").spawn().unwrap();
        let mut waiting = waiter(old.id(), &program).spawn().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(!marker.exists(), "started before the old process exited");
        old.wait().unwrap();
        assert!(waiting.wait().unwrap().success());
        assert!(marker.exists());
    }
}

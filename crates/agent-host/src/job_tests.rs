//! The job transport on Windows, with `cmd.exe` and `ping.exe` for agents.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use futures::StreamExt;
use futures::channel::mpsc;

use crate::job::Job;
use crate::{
    AgentEvent, AgentKind, LaunchCommand, McpServer, Runtime, explain_error, start_session,
};

fn cmd() -> PathBuf {
    std::env::var_os("ComSpec")
        .map(PathBuf::from)
        .unwrap_or_else(|| r"C:\Windows\System32\cmd.exe".into())
}

/// Whether the process `pid` ends within `timeout`.
fn ends(pid: usize, timeout: Duration) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
    };
    // SAFETY: opening a process by id has no preconditions.
    let process = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid as u32) };
    if process.is_null() {
        return true;
    }
    // SAFETY: `process` is open until the `CloseHandle` below.
    let waited = unsafe { WaitForSingleObject(process, timeout.as_millis() as u32) };
    // SAFETY: opened above, closed once.
    unsafe { CloseHandle(process) };
    waited == WAIT_OBJECT_0
}

/// How many `ping.exe` aim at `target`, by their command line.
fn pings(target: &str) -> usize {
    let script = format!(
        "@(Get-CimInstance Win32_Process -Filter \"Name = 'PING.EXE'\" | \
         Where-Object {{ $_.CommandLine -like '*{target}*' }}).Count"
    );
    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap()
}

fn next(rt: &Runtime, events: &mut mpsc::UnboundedReceiver<AgentEvent>) -> Option<AgentEvent> {
    rt.handle()
        .block_on(async { tokio::time::timeout(Duration::from_secs(10), events.next()).await })
        .expect("no event within 10 s")
}

#[test]
fn closing_the_job_ends_what_the_child_started() {
    // `cmd.exe` starts `ping.exe` a second after the job takes it in.
    let mut child = Command::new(cmd())
        .args([
            "/d",
            "/c",
            "ping -n 2 127.0.0.1 >nul & ping -n 60 127.0.0.1 >nul",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let job = Job::new().unwrap();
    job.assign(&child).unwrap();
    thread::sleep(Duration::from_secs(3));
    let ids = job.process_ids();
    assert!(ids.len() >= 2, "{ids:?}");

    drop(job);

    for id in ids {
        assert!(ends(id, Duration::from_secs(10)), "{id} still running");
    }
    let _ = child.wait();
}

#[test]
fn process_exit_fails_with_its_stderr() {
    let rt = Runtime::new().unwrap();
    let (tools, _invocations) = mpsc::unbounded();
    let mcp = rt.handle().block_on(McpServer::start(tools)).unwrap();
    let launch = LaunchCommand {
        program: cmd(),
        args: vec![
            "/d".into(),
            "/c".into(),
            "echo npm error code ENOTFOUND 1>&2 & exit 1".into(),
        ],
        env: vec![],
    };

    let (_session, mut events) = start_session(&rt, launch, std::env::temp_dir(), &mcp);

    let Some(AgentEvent::Failed(raw)) = next(&rt, &mut events) else {
        panic!("expected a failure")
    };
    assert!(raw.contains("ENOTFOUND"), "{raw}");
    assert_eq!(
        explain_error(AgentKind::Codex, &raw),
        "Sin conexión para descargar el adaptador."
    );
    assert_eq!(next(&rt, &mut events), None);
}

#[test]
fn dropping_the_handle_ends_every_process_of_the_agent() {
    let rt = Runtime::new().unwrap();
    let (tools, _invocations) = mpsc::unbounded();
    let mcp = rt.handle().block_on(McpServer::start(tools)).unwrap();
    // Never answers `initialize`. Killing only `cmd.exe`, as `AcpAgent`
    // does, would leave the second `ping.exe` running; its odd target
    // tells it apart.
    let target = "127.0.0.73";
    let launch = LaunchCommand {
        program: cmd(),
        args: vec![
            "/d".into(),
            "/c".into(),
            format!("ping -n 2 127.0.0.1 >nul & ping -n 60 {target} >nul"),
        ],
        env: vec![],
    };

    let (session, mut events) = start_session(&rt, launch, std::env::temp_dir(), &mcp);
    let started = Instant::now();
    while pings(target) == 0 {
        assert!(started.elapsed() < Duration::from_secs(20), "never started");
        thread::sleep(Duration::from_millis(200));
    }
    drop(session);

    assert_eq!(next(&rt, &mut events), None);
    let dropped = Instant::now();
    while pings(target) > 0 {
        assert!(dropped.elapsed() < Duration::from_secs(20), "still running");
        thread::sleep(Duration::from_millis(200));
    }
}

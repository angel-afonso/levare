use super::*;
use crate::agents::{NO_LEVARE_TOOLS, read_shell_path};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

fn all_tools() -> ResolvedTools {
    ResolvedTools {
        path_env: "/opt/homebrew/bin:/usr/bin".into(),
        from_shell: true,
        npx: Some("/opt/homebrew/bin/npx".into()),
        node: Some("/opt/homebrew/bin/node".into()),
        claude: Some("/Users/ana/.local/bin/claude".into()),
        codex: Some("/opt/homebrew/bin/codex".into()),
        gemini: Some("/opt/homebrew/bin/gemini".into()),
    }
}

fn file(path: &std::path::Path, mode: u32) {
    fs::write(path, "#!/bin/sh\n").unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

#[test]
fn parse_probe_reads_paths() {
    let output = "Last login: Fri Oct 23\nPATH=/junk\nLEVARE_PATH=/opt/homebrew/bin:/usr/bin\n";

    assert_eq!(
        parse_probe(output).as_deref(),
        Some("/opt/homebrew/bin:/usr/bin")
    );
    assert_eq!(parse_probe("Last login: Fri Oct 23\n"), None);
    // A line cut short by the deadline is not trusted.
    assert_eq!(parse_probe("LEVARE_PATH=/opt/home"), None);
}

#[test]
fn resolve_finds_executables_on_path() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    file(&first.path().join("npx"), 0o755);
    file(&second.path().join("npx"), 0o755);
    file(&first.path().join("node"), 0o755);
    // Not executable, so the one further on is taken.
    file(&first.path().join("claude"), 0o644);
    file(&second.path().join("claude"), 0o755);
    fs::create_dir(first.path().join("codex")).unwrap();
    file(&second.path().join("gemini-real"), 0o755);
    std::os::unix::fs::symlink(
        second.path().join("gemini-real"),
        second.path().join("gemini"),
    )
    .unwrap();
    let path_env = format!(
        "relative/bin::{}:{}",
        first.path().display(),
        second.path().display()
    );

    let tools = resolve_tools(&path_env, true);

    assert_eq!(tools.path_env, path_env);
    assert!(tools.from_shell);
    assert_eq!(tools.npx, Some(first.path().join("npx")));
    assert_eq!(tools.node, Some(first.path().join("node")));
    assert_eq!(tools.claude, Some(second.path().join("claude")));
    assert_eq!(tools.codex, None);
    assert_eq!(tools.gemini, Some(second.path().join("gemini")));
}

#[test]
fn shell_path_is_read_before_the_deadline() {
    // A daemon started by the startup files keeps stdout open.
    let mut held_open = Command::new("/bin/sh");
    held_open.args(["-c", "printf 'LEVARE_PATH=/a:/b\\n'; sleep 30"]);
    let started = Instant::now();
    assert_eq!(
        read_shell_path(held_open, Duration::from_secs(5)).as_deref(),
        Some("/a:/b")
    );
    assert!(started.elapsed() < Duration::from_secs(2));

    let mut silent = Command::new("/bin/sh");
    silent.args(["-c", "sleep 30"]);
    let started = Instant::now();
    assert_eq!(read_shell_path(silent, Duration::from_millis(300)), None);
    assert!(started.elapsed() < Duration::from_secs(2));

    let mut failing = Command::new("/bin/sh");
    failing.args(["-c", "exit 127"]);
    assert_eq!(read_shell_path(failing, Duration::from_secs(5)), None);
}

#[test]
fn launch_requires_tools() {
    let without_npx = ResolvedTools {
        npx: None,
        ..all_tools()
    };
    let error = launch_command(AgentKind::Claude, &without_npx).unwrap_err();
    assert!(error.contains("npx"), "{error}");
    assert!(!error.contains("PATH"), "{error}");

    let gemini = launch_command(AgentKind::Gemini, &all_tools()).unwrap();
    assert_eq!(gemini.program, PathBuf::from("/opt/homebrew/bin/gemini"));
    assert_eq!(gemini.args, ["--acp"]);
    assert!(
        gemini
            .env
            .contains(&("PATH".into(), "/opt/homebrew/bin:/usr/bin".into()))
    );
}

#[test]
fn launch_says_when_the_shell_path_was_not_read() {
    let guessed = ResolvedTools {
        from_shell: false,
        gemini: None,
        ..all_tools()
    };
    let error = launch_command(AgentKind::Gemini, &guessed).unwrap_err();
    assert!(error.contains("gemini"), "{error}");
    assert!(
        error.contains("No se pudo leer el PATH de tu shell"),
        "{error}"
    );
}

#[test]
fn launch_runs_adapters_through_npx() {
    let claude = launch_command(AgentKind::Claude, &all_tools()).unwrap();
    assert_eq!(claude.program, PathBuf::from("/opt/homebrew/bin/npx"));
    assert_eq!(
        claude.args,
        ["-y", "@agentclientprotocol/claude-agent-acp@0.94.0"]
    );

    let codex = launch_command(AgentKind::Codex, &all_tools()).unwrap();
    assert_eq!(codex.args, ["-y", "@agentclientprotocol/codex-acp@2.2.2"]);
    assert!(
        codex
            .env
            .contains(&("CODEX_PATH".into(), "/opt/homebrew/bin/codex".into()))
    );

    let without_codex = ResolvedTools {
        codex: None,
        ..all_tools()
    };
    let error = launch_command(AgentKind::Codex, &without_codex).unwrap_err();
    assert!(error.contains("codex"), "{error}");

    let without_node = ResolvedTools {
        node: None,
        ..all_tools()
    };
    let error = launch_command(AgentKind::Claude, &without_node).unwrap_err();
    assert!(error.contains("node"), "{error}");
}

#[test]
fn explain_auth_error_per_agent() {
    // ACP's `auth_required` as the adapters send it when there is no login.
    for raw in [
        "Authentication required (-32000)",
        "Authentication required. (-32000)",
    ] {
        for (kind, command) in [
            (AgentKind::Claude, "`claude`"),
            (AgentKind::Codex, "`codex login`"),
            (AgentKind::Gemini, "`gemini`"),
        ] {
            assert_eq!(
                explain_error(kind, raw),
                format!("Inicia sesión una vez en la terminal con {command}.")
            );
        }
    }
}

#[test]
fn explain_specific_auth_error_keeps_the_agents_words() {
    let text = explain_error(
        AgentKind::Gemini,
        "Gemini API key is missing or not configured. (-32000)",
    );
    assert_eq!(
        text,
        "Inicia sesión una vez en la terminal con `gemini`. \
         (El agente dice: Gemini API key is missing or not configured.)"
    );
}

#[test]
fn explain_other_32000_errors_keeps_the_message() {
    let raw = "This client is no longer supported for Gemini Code Assist for individuals. \
               To continue using Gemini, please migrate to the Antigravity suite of products: \
               https://antigravity.google (-32000)";

    let text = explain_error(AgentKind::Gemini, raw);

    assert!(!text.contains("Inicia sesión"), "{text}");
    assert!(
        text.starts_with("This client is no longer supported"),
        "{text}"
    );
    assert!(text.ends_with("https://antigravity.google"), "{text}");
}

#[test]
fn explain_caps_long_messages() {
    let raw = format!("{} (-32603)", "x".repeat(2000));

    let text = explain_error(AgentKind::Claude, &raw);

    assert!(text.chars().count() <= 501, "{}", text.chars().count());
    assert!(text.ends_with('…'));
}

#[test]
fn explain_other_errors() {
    let offline = "Process exited with exit status: 1: npm error code ENOTFOUND\nnpm error network request to https://registry.npmjs.org/@agentclientprotocol%2fcodex-acp failed";
    assert_eq!(
        explain_error(AgentKind::Codex, offline),
        "Sin conexión para descargar el adaptador."
    );
    assert_eq!(
        explain_error(AgentKind::Gemini, NO_LEVARE_TOOLS),
        "Este agente no admite las herramientas de Levare."
    );
    assert_eq!(
        explain_error(AgentKind::Claude, "Internal error (-32603)"),
        "Internal error"
    );
    assert_eq!(
        explain_error(
            AgentKind::Claude,
            "Espera a que termine la petición actual."
        ),
        "Espera a que termine la petición actual."
    );
}

#[test]
fn labels() {
    let labels: Vec<_> = AgentKind::ALL.iter().map(|kind| kind.label()).collect();
    assert_eq!(labels, ["Claude Code", "Codex", "Gemini CLI"]);
}

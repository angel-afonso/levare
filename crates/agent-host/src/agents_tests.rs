use super::*;
use crate::agents::NO_LEVARE_TOOLS;
use std::path::PathBuf;

fn all_tools() -> ResolvedTools {
    ResolvedTools {
        path_env: "/opt/homebrew/bin:/usr/bin".into(),
        npx: Some("/opt/homebrew/bin/npx".into()),
        claude: Some("/Users/ana/.local/bin/claude".into()),
        codex: Some("/opt/homebrew/bin/codex".into()),
        gemini: Some("/opt/homebrew/bin/gemini".into()),
    }
}

#[test]
fn parse_probe_reads_paths() {
    let output =
        "Last login: Fri Oct 23\nPATH=/opt/homebrew/bin:/usr/bin\nnpx=/opt/homebrew/bin/npx\n";

    let tools = parse_probe(output);

    assert_eq!(tools.path_env, "/opt/homebrew/bin:/usr/bin");
    assert_eq!(tools.npx, Some(PathBuf::from("/opt/homebrew/bin/npx")));
    assert_eq!(tools.claude, None);
    assert_eq!(tools.codex, None);
    assert_eq!(tools.gemini, None);
}

#[test]
fn parse_probe_ignores_what_is_not_a_path() {
    // `command -v` prints an alias's definition, not a path.
    let tools = parse_probe("PATH=/usr/bin\nclaude=alias claude='claude --verbose'\n");

    assert_eq!(tools.claude, None);
}

#[test]
fn launch_requires_tools() {
    let without_npx = ResolvedTools {
        npx: None,
        ..all_tools()
    };
    let error = launch_command(AgentKind::Claude, &without_npx).unwrap_err();
    assert!(error.contains("npx"), "{error}");

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
}

#[test]
fn explain_auth_error_per_agent() {
    // What every adapter answers when there is no login: ACP's
    // `auth_required`, code -32000, with or without its own message.
    for raw in [
        "Authentication required (-32000)",
        "Gemini API key is missing or not configured. (-32000)",
    ] {
        for (kind, command) in [
            (AgentKind::Claude, "`claude`"),
            (AgentKind::Codex, "`codex login`"),
            (AgentKind::Gemini, "`gemini`"),
        ] {
            let text = explain_error(kind, raw);
            assert!(
                text.starts_with("Inicia sesión una vez en la terminal con"),
                "{text}"
            );
            assert!(text.contains(command), "{kind:?}: {text}");
        }
    }
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
        "Internal error (-32603)"
    );
}

#[test]
fn labels() {
    let labels: Vec<_> = AgentKind::ALL.iter().map(|kind| kind.label()).collect();
    assert_eq!(labels, ["Claude Code", "Codex", "Gemini CLI"]);
}

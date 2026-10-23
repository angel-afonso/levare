use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// The text a session fails with when the agent cannot reach Levare's tools.
pub(crate) const NO_LEVARE_TOOLS: &str = "Este agente no admite las herramientas de Levare.";
const OFFLINE: &str = "Sin conexión para descargar el adaptador.";

/// A login shell can wait on something forever; past this, nothing is found.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
/// Prints the login shell's `PATH` and where each tool is, one `name=path`
/// line per tool found.
const PROBE_SCRIPT: &str = r#"printf 'PATH=%s\n' "$PATH"
for tool in npx claude codex gemini; do
  found=$(command -v "$tool") && printf '%s=%s\n' "$tool" "$found"
done"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentKind {
    Claude,
    Codex,
    Gemini,
}

impl AgentKind {
    pub const ALL: [AgentKind; 3] = [AgentKind::Claude, AgentKind::Codex, AgentKind::Gemini];

    pub fn label(self) -> &'static str {
        match self {
            AgentKind::Claude => "Claude Code",
            AgentKind::Codex => "Codex",
            AgentKind::Gemini => "Gemini CLI",
        }
    }

    /// What the user runs in a terminal to log in.
    fn login_command(self) -> &'static str {
        match self {
            AgentKind::Claude => "claude",
            AgentKind::Codex => "codex login",
            AgentKind::Gemini => "gemini",
        }
    }
}

/// The user's `PATH` and the tools found on it. An app opened from the
/// Finder does not inherit the shell's `PATH`, so it comes from the probe.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResolvedTools {
    pub path_env: String,
    pub npx: Option<PathBuf>,
    pub claude: Option<PathBuf>,
    pub codex: Option<PathBuf>,
    pub gemini: Option<PathBuf>,
}

/// Reads the probe's output. Lines the shell's startup files print are
/// ignored, and so is anything `command -v` gives that is not a path (an
/// alias, a function).
pub fn parse_probe(output: &str) -> ResolvedTools {
    let mut tools = ResolvedTools::default();
    for line in output.lines() {
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        if name == "PATH" {
            tools.path_env = value.to_string();
            continue;
        }
        let slot = match name {
            "npx" => &mut tools.npx,
            "claude" => &mut tools.claude,
            "codex" => &mut tools.codex,
            "gemini" => &mut tools.gemini,
            _ => continue,
        };
        if Path::new(value).is_absolute() {
            *slot = Some(PathBuf::from(value));
        }
    }
    tools
}

/// Runs the user's login shell to find their `PATH` and tools. Blocks for
/// up to five seconds, so it runs off the UI thread.
pub fn probe() -> ResolvedTools {
    let shell = std::env::var("SHELL")
        .ok()
        .filter(|shell| !shell.is_empty())
        .unwrap_or_else(|| "/bin/zsh".into());
    let mut tools = parse_probe(&run_probe(&shell).unwrap_or_default());
    if tools.path_env.is_empty() {
        tools.path_env = std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into());
    }
    tools
}

/// An interactive login shell: tools installed by their own installers
/// (`~/.local/bin` for Claude Code, nvm for Node) are added to `PATH` in
/// `.zshrc`, which a login shell alone does not read.
fn run_probe(shell: &str) -> Option<String> {
    let mut child = Command::new(shell)
        .args(["-ilc", PROBE_SCRIPT])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut output = String::new();
        let _ = stdout.read_to_string(&mut output);
        let _ = sender.send(output);
    });
    let output = receiver.recv_timeout(PROBE_TIMEOUT).ok();
    let _ = child.kill();
    let _ = child.wait();
    output
}

/// How to start an agent's ACP adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

/// The command for `kind`, or what is missing to run it.
pub fn launch_command(kind: AgentKind, tools: &ResolvedTools) -> Result<LaunchCommand, String> {
    let npx = || found(&tools.npx, "npx", "Node.js");
    let mut env = vec![("PATH".to_string(), tools.path_env.clone())];
    let (program, args) = match kind {
        AgentKind::Claude => {
            let (npx, _claude) = both(npx(), found(&tools.claude, "claude", "Claude Code"))?;
            (
                npx,
                vec!["-y", "@agentclientprotocol/claude-agent-acp@0.94.0"],
            )
        }
        AgentKind::Codex => {
            let (npx, codex) = both(npx(), found(&tools.codex, "codex", "Codex"))?;
            env.push(("CODEX_PATH".into(), codex.display().to_string()));
            (npx, vec!["-y", "@agentclientprotocol/codex-acp@2.2.2"])
        }
        AgentKind::Gemini => (found(&tools.gemini, "gemini", "Gemini CLI")?, vec!["--acp"]),
    };
    Ok(LaunchCommand {
        program,
        args: args.into_iter().map(String::from).collect(),
        env,
    })
}

fn found(tool: &Option<PathBuf>, name: &str, install: &str) -> Result<PathBuf, String> {
    tool.clone()
        .ok_or_else(|| format!("No se encontró `{name}`: instala {install}."))
}

/// Both tools, or everything that is missing.
fn both(
    first: Result<PathBuf, String>,
    second: Result<PathBuf, String>,
) -> Result<(PathBuf, PathBuf), String> {
    match (first, second) {
        (Ok(first), Ok(second)) => Ok((first, second)),
        (Err(first), Err(second)) => Err(format!("{first} {second}")),
        (Err(error), _) | (_, Err(error)) => Err(error),
    }
}

/// What to tell the user about a session's failure. `raw` is the text of
/// `AgentEvent::Failed`; what is not recognised is returned as it is.
pub fn explain_error(kind: AgentKind, raw: &str) -> String {
    let lower = raw.to_lowercase();
    // All three adapters refuse a session or a turn without a login with
    // ACP's `auth_required` (-32000).
    let auth = raw.contains("(-32000)")
        || ["auth_required", "authentication required", "not logged in"]
            .iter()
            .any(|signal| lower.contains(signal));
    if auth {
        return format!(
            "Inicia sesión una vez en la terminal con `{}`.",
            kind.login_command()
        );
    }
    if raw.contains(NO_LEVARE_TOOLS) || raw.contains("mcpCapabilities") {
        return NO_LEVARE_TOOLS.into();
    }
    let network = [
        "ENOTFOUND",
        "EAI_AGAIN",
        "ECONNREFUSED",
        "ECONNRESET",
        "ETIMEDOUT",
    ];
    if lower.contains("npm") && network.iter().any(|code| raw.contains(code)) {
        return OFFLINE.into();
    }
    raw.to_string()
}

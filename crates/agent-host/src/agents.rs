use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// The text a session fails with when the agent cannot reach Levare's tools.
pub(crate) const NO_LEVARE_TOOLS: &str = "Este agente no admite las herramientas de Levare.";
const OFFLINE: &str = "Sin conexión para descargar el adaptador.";
const NO_SHELL_PATH: &str =
    "No se pudo leer el PATH de tu shell; se buscó solo en las carpetas habituales.";
/// How much of an agent's own message is shown.
const MESSAGE_LIMIT: usize = 500;

/// A login shell can wait on something forever; past this, the usual
/// folders are searched instead.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
/// Prints the shell's `PATH`; valid in zsh, bash and fish, where a quoted
/// `$PATH` is joined with `:` too.
const PROBE_SCRIPT: &str = r#"printf 'LEVARE_PATH=%s\n' "$PATH""#;
const PROBE_KEY: &str = "LEVARE_PATH=";
/// Where installers put these tools, searched when the shell's `PATH`
/// could not be read. Relative ones are under the home folder.
const USUAL_FOLDERS: [&str; 5] = [
    "/opt/homebrew/bin",
    "/usr/local/bin",
    ".local/bin",
    ".bun/bin",
    ".npm-global/bin",
];

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
    /// Whether `path_env` is the shell's. If not, it is the app's own
    /// `PATH` plus the usual folders, and a missing tool may be installed
    /// after all.
    pub from_shell: bool,
    pub npx: Option<PathBuf>,
    pub node: Option<PathBuf>,
    pub claude: Option<PathBuf>,
    pub gemini: Option<PathBuf>,
}

/// The `PATH` in the probe's output. Lines the shell's startup files print
/// are ignored, and so is a last line the deadline cut short.
pub fn parse_probe(output: &str) -> Option<String> {
    output
        .split_inclusive('\n')
        .filter_map(|line| line.strip_suffix('\n'))
        .filter_map(|line| line.trim_end_matches('\r').strip_prefix(PROBE_KEY))
        .next_back()
        .map(String::from)
}

/// Finds each tool as the first executable file named so in a folder of
/// `path_env`. Aliases and shell functions don't count: the agent is not
/// started by a shell.
pub fn resolve_tools(path_env: &str, from_shell: bool) -> ResolvedTools {
    let find = |name: &str| {
        path_env
            .split(':')
            .map(Path::new)
            .filter(|folder| folder.is_absolute())
            .map(|folder| folder.join(name))
            .find(|candidate| is_executable(candidate))
    };
    ResolvedTools {
        path_env: path_env.to_string(),
        from_shell,
        npx: find("npx"),
        node: find("node"),
        claude: find("claude"),
        gemini: find("gemini"),
    }
}

fn is_executable(path: &Path) -> bool {
    fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

/// Asks the user's interactive login shell for its `PATH` and finds the
/// tools on it. Interactive, because installers (Claude Code's in
/// `~/.local/bin`, nvm) add to `PATH` in `.zshrc`. Blocks for up to five
/// seconds, so it runs off the UI thread.
pub fn probe() -> ResolvedTools {
    let shell = std::env::var("SHELL")
        .ok()
        .filter(|shell| !shell.is_empty())
        .unwrap_or_else(|| "/bin/zsh".into());
    let mut command = Command::new(shell);
    command.args(["-ilc", PROBE_SCRIPT]);
    match read_shell_path(command, PROBE_TIMEOUT) {
        Some(path) if !path.is_empty() => resolve_tools(&path, true),
        _ => resolve_tools(&usual_path(), false),
    }
}

/// Runs `command` and returns the `PATH` it prints, as soon as it is
/// printed: a daemon the startup files leave behind may keep stdout open.
/// `None` if nothing came before `timeout`.
pub(crate) fn read_shell_path(mut command: Command, timeout: Duration) -> Option<String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let (chunks, received) = mpsc::channel();
    // Blocks until stdout closes, which a leftover daemon may delay; only
    // this thread waits for it.
    thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        while let Ok(read @ 1..) = stdout.read(&mut buffer) {
            if chunks.send(buffer[..read].to_vec()).is_err() {
                break;
            }
        }
    });
    let deadline = Instant::now() + timeout;
    let mut output = Vec::new();
    let mut path = None;
    while path.is_none() {
        let left = deadline.saturating_duration_since(Instant::now());
        let Ok(chunk) = received.recv_timeout(left) else {
            break;
        };
        output.extend(chunk);
        path = parse_probe(&String::from_utf8_lossy(&output));
    }
    let _ = child.kill();
    let _ = child.wait();
    path
}

/// The app's own `PATH` followed by the usual folders.
fn usual_path() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let own = std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into());
    let mut folders: Vec<String> = own.split(':').map(String::from).collect();
    for folder in USUAL_FOLDERS {
        let folder = if folder.starts_with('/') {
            folder.to_string()
        } else {
            format!("{home}/{folder}")
        };
        if !folders.contains(&folder) {
            folders.push(folder);
        }
    }
    folders.join(":")
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
    let mut missing = Vec::new();
    let env = vec![("PATH".to_string(), tools.path_env.clone())];
    let (program, args) = match kind {
        AgentKind::Claude => {
            let npx = require_npx(tools, &mut missing);
            require(&tools.claude, "claude", "Claude Code", &mut missing);
            (
                npx,
                vec!["-y", "@agentclientprotocol/claude-agent-acp@0.94.0"],
            )
        }
        // The adapter runs the Codex it bundles: the user's own may be too
        // old for the model their account picks.
        AgentKind::Codex => {
            let npx = require_npx(tools, &mut missing);
            (npx, vec!["-y", "@agentclientprotocol/codex-acp@2.2.2"])
        }
        AgentKind::Gemini => {
            let gemini = require(&tools.gemini, "gemini", "Gemini CLI", &mut missing);
            (gemini, vec!["--acp"])
        }
    };
    if !missing.is_empty() {
        if !tools.from_shell {
            missing.push(NO_SHELL_PATH.into());
        }
        return Err(missing.join(" "));
    }
    Ok(LaunchCommand {
        program,
        args: args.into_iter().map(String::from).collect(),
        env,
    })
}

/// `npx`, and the `node` it runs on.
fn require_npx(tools: &ResolvedTools, missing: &mut Vec<String>) -> PathBuf {
    let npx = require(&tools.npx, "npx", "Node.js", missing);
    if tools.npx.is_some() {
        require(&tools.node, "node", "Node.js", missing);
    }
    npx
}

/// The tool's path; if it is missing, says so in `missing`.
fn require(
    tool: &Option<PathBuf>,
    name: &str,
    install: &str,
    missing: &mut Vec<String>,
) -> PathBuf {
    tool.clone().unwrap_or_else(|| {
        missing.push(format!("No se encontró `{name}`: instala {install}."));
        PathBuf::new()
    })
}

/// What to tell the user about a session's failure. `raw` is the text of
/// `AgentEvent::Failed`; the agent's own message is kept, shortened.
pub fn explain_error(kind: AgentKind, raw: &str) -> String {
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
    if raw.to_lowercase().contains("npm") && network.iter().any(|code| raw.contains(code)) {
        return OFFLINE.into();
    }
    let (message, code) = split_code(raw);
    let lower = message.to_lowercase();
    // The three adapters refuse a session or a turn without a login with
    // ACP's `auth_required` (-32000), but Gemini uses that code for other
    // refusals too: the message decides.
    let generic = ["authentication required", "auth_required", "auth required"]
        .contains(&lower.trim_end_matches('.'))
        || (code == Some(-32000) && message.parse::<i32>().is_ok());
    let auth = generic
        || [
            "auth_required",
            "authentication",
            "not logged in",
            "/login",
            "log in",
            "sign in",
            "sign-in",
            "api key",
            "unauthorized",
            "oauth",
            "credentials",
        ]
        .iter()
        .any(|signal| lower.contains(signal));
    if !auth {
        return shortened(message);
    }
    let hint = format!(
        "Inicia sesión una vez en la terminal con `{}`.",
        kind.login_command()
    );
    if generic {
        hint
    } else {
        format!("{hint} (El agente dice: {})", shortened(message))
    }
}

/// The message and the JSON-RPC code the session appends as ` (code)`.
fn split_code(raw: &str) -> (&str, Option<i32>) {
    let raw = raw.trim();
    let code = raw
        .strip_suffix(')')
        .and_then(|rest| rest.rsplit_once(" ("))
        .and_then(|(message, code)| Some((message, code.parse().ok()?)));
    match code {
        Some((message, code)) => (message.trim(), Some(code)),
        None => (raw, None),
    }
}

fn shortened(message: &str) -> String {
    match message.char_indices().nth(MESSAGE_LIMIT) {
        Some((end, _)) => format!("{}…", &message[..end]),
        None => message.to_string(),
    }
}

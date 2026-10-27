use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Read;
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
/// Windows hands Levare the user's `PATH` as it was when Levare started.
const REOPEN_AFTER_INSTALL: &str = "Si ya está instalado, cierra Levare y vuelve a abrirlo.";
/// How much of an agent's own message is shown.
const MESSAGE_LIMIT: usize = 500;

/// A login shell can wait on something forever; past this, the usual
/// folders are searched instead.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
/// Prints the shell's `PATH`; valid in zsh, bash, sh and fish, where a
/// quoted `$PATH` is joined with `:` too.
const PROBE_SCRIPT: &str = r#"printf 'LEVARE_PATH=%s\n' "$PATH""#;
const PROBE_KEY: &str = "LEVARE_PATH=";
/// The extensions of the files `Command` can start on Windows: programs,
/// and batch files, which it runs through `cmd.exe`.
const RUNNABLE: [&str; 4] = [".com", ".exe", ".bat", ".cmd"];

/// The systems whose conventions the lookup follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Os {
    Mac,
    Linux,
    Windows,
}

impl Os {
    /// The system Levare runs on. Other Unix systems follow Linux.
    pub(crate) const HOST: Os = if cfg!(target_os = "macos") {
        Os::Mac
    } else if cfg!(windows) {
        Os::Windows
    } else {
        Os::Linux
    };
}

/// What the lookup reads from the system, so tests can stand in for it.
pub(crate) struct Host<'a> {
    pub os: Os,
    pub home: Option<PathBuf>,
    /// One of the app's environment variables; an empty one is `None`.
    pub var: &'a dyn Fn(&str) -> Option<OsString>,
}

impl Host<'static> {
    fn current() -> Self {
        Host {
            os: Os::HOST,
            home: dirs::home_dir(),
            var: &app_var,
        }
    }
}

fn app_var(name: &str) -> Option<OsString> {
    std::env::var_os(name).filter(|value| !value.is_empty())
}

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

/// Where [`ResolvedTools::path_env`] came from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PathSource {
    /// The login shell's, on macOS and Linux: an app opened from the Dock
    /// or a desktop menu doesn't inherit it.
    #[default]
    Shell,
    /// The app's own plus the usual folders, because the shell's could not
    /// be read: a missing tool may be installed after all.
    Guessed,
    /// The app's own plus the usual folders, on Windows, which gives every
    /// app the user's `PATH` as it was when the app started.
    App,
}

/// The user's `PATH` and the tools found on it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResolvedTools {
    /// Joined as the system joins `PATH`: `:` on Unix, `;` on Windows.
    pub path_env: String,
    pub source: PathSource,
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

/// Finds each tool as the first file it would run as in a folder of
/// `path_env`. Aliases and shell functions don't count: the agent is not
/// started by a shell.
pub fn resolve_tools(path_env: &str, source: PathSource) -> ResolvedTools {
    resolve_with(path_env, source, &Rules::of(&Host::current()))
}

pub(crate) fn resolve_with(path_env: &str, source: PathSource, rules: &Rules) -> ResolvedTools {
    let find = |name: &str| {
        std::env::split_paths(path_env)
            .filter(|folder| folder.is_absolute())
            .find_map(|folder| rules.find(&folder, name))
    };
    ResolvedTools {
        path_env: path_env.to_string(),
        source,
        npx: find("npx"),
        node: find("node"),
        claude: find("claude"),
        gemini: find("gemini"),
    }
}

/// How a bare name such as `npx` becomes a file to run.
pub(crate) struct Rules {
    /// On Windows, the extensions to try, in order: `npx` is `npx.cmd`.
    /// `None` elsewhere, where it is the file named so, with its execute
    /// bits.
    extensions: Option<Vec<String>>,
}

impl Rules {
    pub(crate) fn of(host: &Host) -> Rules {
        let extensions =
            (host.os == Os::Windows).then(|| windows_extensions((host.var)("PATHEXT")));
        Rules { extensions }
    }

    /// The file `name` runs in `folder`, if there is one.
    fn find(&self, folder: &Path, name: &str) -> Option<PathBuf> {
        match &self.extensions {
            Some(extensions) => extensions
                .iter()
                .map(|extension| folder.join(format!("{name}{extension}")))
                .find(|candidate| runs_on_windows(candidate)),
            None => Some(folder.join(name)).filter(|candidate| is_executable(candidate)),
        }
    }
}

/// The extensions in `PATHEXT` that `Command` can start, in its order,
/// lowercase. Windows' own default if it names none.
pub(crate) fn windows_extensions(pathext: Option<OsString>) -> Vec<String> {
    let pathext = pathext.as_deref().map(OsStr::to_string_lossy);
    let mut extensions: Vec<String> = Vec::new();
    for extension in pathext.as_deref().unwrap_or_default().split(';') {
        let extension = extension.trim().to_ascii_lowercase();
        if RUNNABLE.contains(&extension.as_str()) && !extensions.contains(&extension) {
            extensions.push(extension);
        }
    }
    if extensions.is_empty() {
        RUNNABLE.map(String::from).to_vec()
    } else {
        extensions
    }
}

/// Whether Windows can start the file at `path`. Files there have no
/// execute bits: the extension decides.
fn runs_on_windows(path: &Path) -> bool {
    let runnable = path
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| {
            let extension = format!(".{}", extension.to_ascii_lowercase());
            RUNNABLE.contains(&extension.as_str())
        });
    runnable && path.is_file()
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

/// Off Unix there are no execute bits: the extension decides.
#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    runs_on_windows(path)
}

/// Finds the tools on the user's `PATH`. On macOS and Linux it asks the
/// user's interactive login shell for it: interactive, because installers
/// (Claude Code's in `~/.local/bin`, nvm) add to `PATH` in `.zshrc` or
/// `.bashrc`. Blocks for up to five seconds, so it runs off the UI thread.
pub fn probe() -> ResolvedTools {
    probe_on(&Host::current(), PROBE_TIMEOUT)
}

pub(crate) fn probe_on(host: &Host, timeout: Duration) -> ResolvedTools {
    let rules = Rules::of(host);
    let Some(shell) = login_shell(host) else {
        return resolve_with(&usual_path(host), PathSource::App, &rules);
    };
    let mut command = Command::new(shell);
    command.args(["-ilc", PROBE_SCRIPT]);
    match read_shell_path(command, timeout) {
        Some(path) if !path.is_empty() => resolve_with(&path, PathSource::Shell, &rules),
        _ => resolve_with(&usual_path(host), PathSource::Guessed, &rules),
    }
}

/// The shell to ask for `PATH`. On Windows there is none: apps get the
/// user's `PATH` from the system.
pub(crate) fn login_shell(host: &Host) -> Option<OsString> {
    let fallback = match host.os {
        Os::Windows => return None,
        Os::Mac => "/bin/zsh",
        Os::Linux => "/bin/sh",
    };
    Some((host.var)("SHELL").unwrap_or_else(|| fallback.into()))
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
pub(crate) fn usual_path(host: &Host) -> String {
    let own: Vec<PathBuf> = match (host.var)("PATH") {
        Some(path) => std::env::split_paths(&path).collect(),
        None if host.os == Os::Windows => Vec::new(),
        None => vec!["/usr/bin".into(), "/bin".into()],
    };
    let mut folders: Vec<PathBuf> = Vec::new();
    for folder in own.into_iter().chain(usual_folders(host)) {
        // A folder whose name holds the separator can't be passed on.
        if !folders.contains(&folder) && std::env::join_paths([&folder]).is_ok() {
            folders.push(folder);
        }
    }
    std::env::join_paths(folders)
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// Where installers put these tools, for when they are not on the `PATH`
/// Levare has.
pub(crate) fn usual_folders(host: &Host) -> Vec<PathBuf> {
    let home = |relative: &str| host.home.clone().map(|home| under(home, relative));
    let var =
        |name: &str, relative: &str| (host.var)(name).map(|base| under(base.into(), relative));
    let folders = match host.os {
        Os::Mac => vec![
            Some("/opt/homebrew/bin".into()),
            Some("/usr/local/bin".into()),
            home(".local/bin"),
            home(".bun/bin"),
            home(".npm-global/bin"),
            home(".volta/bin"),
            nvm_node(host),
        ],
        Os::Linux => vec![
            Some("/usr/local/bin".into()),
            home(".local/bin"),
            home(".npm-global/bin"),
            home(".volta/bin"),
            home(".bun/bin"),
            Some("/home/linuxbrew/.linuxbrew/bin".into()),
            Some("/snap/bin".into()),
            nvm_node(host),
        ],
        Os::Windows => vec![
            // `npm install -g`: `gemini.cmd`, and `claude.cmd` installed so.
            var("APPDATA", "npm"),
            // The Node.js installer: `node.exe`, `npx.cmd`.
            var("ProgramFiles", "nodejs"),
            // Claude Code's own installer: `claude.exe`.
            home(".local/bin"),
            // nvm-windows' active Node.
            var("NVM_SYMLINK", ""),
            var("LOCALAPPDATA", "Volta/bin"),
            home(".bun/bin"),
            home("scoop/shims"),
            var("LOCALAPPDATA", "Microsoft/WinGet/Links"),
        ],
    };
    folders.into_iter().flatten().collect()
}

/// `relative`, written with `/`, under `base`.
fn under(base: PathBuf, relative: &str) -> PathBuf {
    relative
        .split('/')
        .filter(|part| !part.is_empty())
        .fold(base, |path, part| path.join(part))
}

/// The newest Node that nvm installed. nvm puts its Node on `PATH` from
/// the shell's startup files, which only the probe reads.
fn nvm_node(host: &Host) -> Option<PathBuf> {
    let nvm = (host.var)("NVM_DIR")
        .map(PathBuf::from)
        .or_else(|| host.home.as_ref().map(|home| home.join(".nvm")))?;
    let versions = nvm.join("versions").join("node");
    fs::read_dir(&versions)
        .ok()?
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            let version: Vec<u64> = name
                .strip_prefix('v')?
                .split('.')
                .map(|part| part.parse().ok())
                .collect::<Option<_>>()?;
            Some((version, name))
        })
        .max()
        .map(|(_, name)| versions.join(name).join("bin"))
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
        match tools.source {
            PathSource::Shell => {}
            PathSource::Guessed => missing.push(NO_SHELL_PATH.into()),
            PathSource::App => missing.push(REOPEN_AFTER_INSTALL.into()),
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

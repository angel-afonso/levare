use super::*;
use crate::agents::{
    Host, NO_LEVARE_TOOLS, Os, Rules, login_shell, probe_on, resolve_with, usual_folders,
    usual_path, windows_extensions,
};
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn all_tools() -> ResolvedTools {
    ResolvedTools {
        path_env: "/opt/homebrew/bin:/usr/bin".into(),
        source: PathSource::Shell,
        npx: Some("/opt/homebrew/bin/npx".into()),
        node: Some("/opt/homebrew/bin/node".into()),
        claude: Some("/Users/ana/.local/bin/claude".into()),
        gemini: Some("/opt/homebrew/bin/gemini".into()),
    }
}

/// A stand-in for the app's environment.
fn vars(pairs: &[(&str, &Path)]) -> HashMap<String, OsString> {
    pairs
        .iter()
        .map(|(name, value)| (name.to_string(), value.as_os_str().to_owned()))
        .collect()
}

fn touch(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, "").unwrap();
}

/// `folders` joined as the system joins `PATH`.
fn path_of(folders: &[&Path]) -> String {
    std::env::join_paths(folders)
        .unwrap()
        .into_string()
        .unwrap()
}

fn folders_of(path_env: &str) -> Vec<PathBuf> {
    std::env::split_paths(path_env).collect()
}

#[test]
fn windows_extensions_come_from_pathext() {
    let extensions = |pathext: Option<&str>| windows_extensions(pathext.map(OsString::from));

    assert_eq!(extensions(None), [".com", ".exe", ".bat", ".cmd"]);
    assert_eq!(
        extensions(Some(
            ".COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC;.PS1"
        )),
        [".com", ".exe", ".bat", ".cmd"]
    );
    // Only what `Command` can start, in `PATHEXT`'s order.
    assert_eq!(extensions(Some(".CMD; .exe ;;.JS;.cmd")), [".cmd", ".exe"]);
    assert_eq!(extensions(Some(".PS1")), [".com", ".exe", ".bat", ".cmd"]);
}

#[test]
fn windows_finds_launchers_by_extension() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    // Node.js ships an extensionless `npx` shell script next to `npx.cmd`;
    // Windows doesn't run it, and neither does `Command` run `.ps1`.
    touch(&first.path().join("npx"));
    touch(&first.path().join("npx.ps1"));
    touch(&second.path().join("npx.cmd"));
    // A folder named like a launcher is not one.
    fs::create_dir(first.path().join("node.com")).unwrap();
    touch(&first.path().join("node.exe"));
    // In one folder, `PATHEXT`'s order decides.
    touch(&first.path().join("claude.cmd"));
    touch(&first.path().join("claude.exe"));
    // An earlier folder wins over a preferred extension.
    touch(&first.path().join("gemini.cmd"));
    touch(&second.path().join("gemini.exe"));
    let pathext = OsString::from(".COM;.EXE;.BAT;.CMD;.VBS;.JS;.PS1");
    let var = |name: &str| (name == "PATHEXT").then(|| pathext.clone());
    let rules = Rules::of(&Host {
        os: Os::Windows,
        home: None,
        var: &var,
    });
    let path_env = path_of(&[first.path(), second.path()]);

    let tools = resolve_with(&path_env, PathSource::App, &rules);

    assert_eq!(tools.npx, Some(second.path().join("npx.cmd")));
    assert_eq!(tools.node, Some(first.path().join("node.exe")));
    assert_eq!(tools.claude, Some(first.path().join("claude.exe")));
    assert_eq!(tools.gemini, Some(first.path().join("gemini.cmd")));
}

#[test]
fn windows_searches_the_app_path_then_the_installers_folders() {
    let root = tempfile::tempdir().unwrap();
    let own = root.path().join("own");
    let app_data = root.path().join("AppData").join("Roaming");
    let local = root.path().join("AppData").join("Local");
    let program_files = root.path().join("Program Files");
    let home = root.path().join("ana");
    fs::create_dir_all(&own).unwrap();
    touch(&program_files.join("nodejs").join("node.exe"));
    touch(&program_files.join("nodejs").join("npx.cmd"));
    touch(&home.join(".local").join("bin").join("claude.exe"));
    touch(&app_data.join("npm").join("gemini.cmd"));
    let env = vars(&[
        ("PATH", &own),
        ("APPDATA", &app_data),
        ("LOCALAPPDATA", &local),
        ("ProgramFiles", &program_files),
        // Never asked: Windows has no login shell to probe.
        ("SHELL", Path::new("/bin/false")),
    ]);
    let var = |name: &str| env.get(name).cloned();
    let host = Host {
        os: Os::Windows,
        home: Some(home.clone()),
        var: &var,
    };

    let tools = probe_on(&host, Duration::from_secs(5));

    assert_eq!(tools.source, PathSource::App);
    let folders = folders_of(&tools.path_env);
    assert_eq!(folders[0], own);
    for folder in [
        app_data.join("npm"),
        program_files.join("nodejs"),
        home.join(".local").join("bin"),
        local.join("Volta").join("bin"),
        local.join("Microsoft").join("WinGet").join("Links"),
    ] {
        assert!(folders.contains(&folder), "{folder:?} in {folders:?}");
    }
    assert_eq!(
        tools.npx,
        Some(program_files.join("nodejs").join("npx.cmd"))
    );
    assert_eq!(
        tools.node,
        Some(program_files.join("nodejs").join("node.exe"))
    );
    assert_eq!(
        tools.claude,
        Some(home.join(".local").join("bin").join("claude.exe"))
    );
    assert_eq!(tools.gemini, Some(app_data.join("npm").join("gemini.cmd")));
    let claude = launch_command(AgentKind::Claude, &tools).unwrap();
    assert_eq!(claude.program, program_files.join("nodejs").join("npx.cmd"));
    assert_eq!(claude.env, [("PATH".to_string(), tools.path_env.clone())]);
}

#[test]
fn windows_says_to_reopen_after_installing() {
    let tools = ResolvedTools {
        source: PathSource::App,
        npx: None,
        ..all_tools()
    };

    let error = launch_command(AgentKind::Codex, &tools).unwrap_err();

    assert_eq!(
        error,
        "No se encontró `npx`: instala Node.js. \
         Si ya está instalado, cierra Levare y vuelve a abrirlo."
    );
}

#[test]
fn linux_and_mac_search_the_installers_folders() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    let versions = home.join(".nvm").join("versions").join("node");
    // The newest by number, not by name.
    for version in ["v9.11.2", "v18.20.4", "v20.3.0", "system", "v21.x"] {
        fs::create_dir_all(versions.join(version).join("bin")).unwrap();
    }
    let none = |_: &str| None;
    let on = |os| Host {
        os,
        home: Some(home.to_path_buf()),
        var: &none,
    };

    let linux = usual_folders(&on(Os::Linux));
    let mac = usual_folders(&on(Os::Mac));

    let newest = versions.join("v20.3.0").join("bin");
    for folder in [
        PathBuf::from("/usr/local/bin"),
        home.join(".local").join("bin"),
        home.join(".npm-global").join("bin"),
        home.join(".volta").join("bin"),
        home.join(".bun").join("bin"),
        newest.clone(),
    ] {
        assert!(linux.contains(&folder), "{folder:?} in {linux:?}");
        assert!(mac.contains(&folder), "{folder:?} in {mac:?}");
    }
    assert!(!linux.contains(&PathBuf::from("/opt/homebrew/bin")));
    assert_eq!(mac[0], PathBuf::from("/opt/homebrew/bin"));

    // `NVM_DIR` moves nvm's folder.
    let elsewhere = tempfile::tempdir().unwrap();
    let node = elsewhere.path().join("versions/node/v22.1.0/bin");
    fs::create_dir_all(&node).unwrap();
    let nvm_dir = |name: &str| (name == "NVM_DIR").then(|| elsewhere.path().into());
    let host = Host {
        os: Os::Linux,
        home: Some(home.to_path_buf()),
        var: &nvm_dir,
    };
    assert!(usual_folders(&host).contains(&node));
}

#[test]
fn usual_path_keeps_the_app_path_first_once() {
    let none = |_: &str| None;
    let linux = Host {
        os: Os::Linux,
        home: None,
        var: &none,
    };
    let folders = folders_of(&usual_path(&linux));
    assert_eq!(
        folders[..3],
        [
            PathBuf::from("/usr/bin"),
            PathBuf::from("/bin"),
            PathBuf::from("/usr/local/bin"),
        ]
    );

    let own = path_of(&[Path::new("/usr/local/bin"), Path::new("/opt/tools")]);
    let env = vars(&[("PATH", Path::new(&own))]);
    let var = |name: &str| env.get(name).cloned();
    let mac = Host {
        os: Os::Mac,
        home: None,
        var: &var,
    };
    let folders = folders_of(&usual_path(&mac));
    assert_eq!(
        folders[..3],
        [
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/opt/tools"),
            PathBuf::from("/opt/homebrew/bin"),
        ]
    );
    let local = PathBuf::from("/usr/local/bin");
    assert_eq!(folders.iter().filter(|folder| **folder == local).count(), 1);
}

#[test]
fn login_shell_per_system() {
    let none = |_: &str| None;
    let fish = |name: &str| (name == "SHELL").then(|| "/usr/bin/fish".into());
    let shell = |os, var: &dyn Fn(&str) -> Option<OsString>| {
        login_shell(&Host {
            os,
            home: None,
            var,
        })
    };

    assert_eq!(shell(Os::Mac, &none), Some("/bin/zsh".into()));
    assert_eq!(shell(Os::Linux, &none), Some("/bin/sh".into()));
    assert_eq!(shell(Os::Linux, &fish), Some("/usr/bin/fish".into()));
    assert_eq!(shell(Os::Windows, &fish), None);
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

/// Tests that run login shells and build executables with Unix
/// permissions, as the lookup does there.
#[cfg(unix)]
mod unix {
    use super::*;
    use crate::agents::read_shell_path;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    use std::time::Instant;

    fn file(path: &Path, mode: u32, script: &str) {
        fs::write(path, script).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn resolve_finds_executables_on_path() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let script = "#!/bin/sh\n";
        file(&first.path().join("npx"), 0o755, script);
        file(&second.path().join("npx"), 0o755, script);
        file(&first.path().join("node"), 0o755, script);
        // Not executable, so the one further on is taken.
        file(&first.path().join("claude"), 0o644, script);
        file(&second.path().join("claude"), 0o755, script);
        // A folder named like a tool is not the tool.
        fs::create_dir(first.path().join("gemini")).unwrap();
        file(&second.path().join("gemini-real"), 0o755, script);
        std::os::unix::fs::symlink(
            second.path().join("gemini-real"),
            second.path().join("gemini"),
        )
        .unwrap();
        // A Windows launcher is not what a bare name runs here.
        file(&first.path().join("gemini.cmd"), 0o755, script);
        let path_env = format!(
            "relative/bin::{}:{}",
            first.path().display(),
            second.path().display()
        );

        let tools = resolve_tools(&path_env, PathSource::Shell);

        assert_eq!(tools.path_env, path_env);
        assert_eq!(tools.source, PathSource::Shell);
        assert_eq!(tools.npx, Some(first.path().join("npx")));
        assert_eq!(tools.node, Some(first.path().join("node")));
        assert_eq!(tools.claude, Some(second.path().join("claude")));
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
    fn probe_asks_the_login_shell_then_falls_back_to_the_usual_folders() {
        let folder = tempfile::tempdir().unwrap();
        let shell = folder.path().join("shell");
        // Run as `shell -ilc <script>`, it answers as a login shell would.
        file(
            &shell,
            0o755,
            "#!/bin/sh\nprintf 'LEVARE_PATH=/from/the/shell:/usr/bin\\n'\n",
        );
        let env = vars(&[("SHELL", &shell), ("PATH", Path::new("/own/bin"))]);
        let var = |name: &str| env.get(name).cloned();
        for os in [Os::Mac, Os::Linux] {
            let host = Host {
                os,
                home: None,
                var: &var,
            };

            let tools = probe_on(&host, Duration::from_secs(5));

            assert_eq!(tools.source, PathSource::Shell);
            assert_eq!(tools.path_env, "/from/the/shell:/usr/bin");
        }

        let broken = vars(&[
            ("SHELL", Path::new("/nonexistent/shell")),
            ("PATH", Path::new("/own/bin")),
        ]);
        let var = |name: &str| broken.get(name).cloned();
        let host = Host {
            os: Os::Linux,
            home: None,
            var: &var,
        };

        let tools = probe_on(&host, Duration::from_secs(5));

        assert_eq!(tools.source, PathSource::Guessed);
        let folders = folders_of(&tools.path_env);
        assert_eq!(folders[0], PathBuf::from("/own/bin"));
        assert!(folders.contains(&PathBuf::from("/usr/local/bin")));
    }
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
        source: PathSource::Guessed,
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

    // The adapter runs the Codex it bundles, so the user's own `codex` is
    // neither needed nor passed.
    let codex = launch_command(AgentKind::Codex, &all_tools()).unwrap();
    assert_eq!(codex.program, PathBuf::from("/opt/homebrew/bin/npx"));
    assert_eq!(codex.args, ["-y", "@agentclientprotocol/codex-acp@2.2.2"]);
    assert_eq!(
        codex.env,
        [("PATH".to_string(), "/opt/homebrew/bin:/usr/bin".to_string())]
    );

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

# Portability audit: Linux and Windows

State of Levare on Linux and Windows as of 2026-10-27, from reading the code and from
CI run [40570605166](https://github.com/angel-afonso/levare/actions/runs/40570605166)
on branch `ci-linux-windows`. Nobody has run Levare on either system by hand.

The Status column says where each finding stands on 2026-10-28, from the code on
branch `audit-fixes`:

- **Fixed**: the code no longer does what the row describes. The commit that fixed it
  follows, and a `file:line` where it helps to find the change.
- **Partial**: part of the fix is in; the rest is described.
- **Open**: nothing has changed yet.

Line numbers in the Where column are from 2026-10-27 and may have moved since; Status
gives the current ones.

Severity:

- **Blocker**: a core task is impossible.
- **Broken**: a feature doesn't work.
- **Degraded**: it works, but badly.
- **Cosmetic**: wrong look or text.

Platforms are written W (Windows), L (Linux) or W+L (both).

## What CI showed

| | Linux | Windows | Status |
| --- | --- | --- | --- |
| `cargo check`, clippy | pass | pass | — |
| Tests | 4 fail (updater) | 25 fail (see below) | Fixed: the updater, recovery, agent and keymap fixes below |
| App opens and stays up 30 s | yes | yes | — |
| Screenshot | black (Xvfb + Mesa lavapipe; inconclusive) | score renders correctly | Open: the Linux screenshot is still inconclusive |

The Windows screenshot confirms these findings:

- the window is larger than the 1024×768 screen and opens partly off it (fixed,
  `996f819`);
- the window has no min/max/close buttons (fixed, `125cc92`);
- the status bar reads «Autoguardado no disponible» (fixed, `7666e37`);
- the ribbon labels show Mac glyphs (`⌥A`, `⇧2`) (fixed, `27e92fe`);
- with no audio device, it reports that and keeps running.

The score itself (Bravura, staff lines, Times New Roman title) renders correctly.

### Failing tests

| Platform | Tests | Cause | Status |
| --- | --- | --- | --- |
| W+L | `update`: 3 install tests, `this_build_has_a_platform_key` | The updater is macOS-only (`ditto`, `codesign`, no platform key). | Fixed (`e774614`) |
| W | `score-io`: 7 recovery tests | `lock_file` always fails off unix. | Fixed (`7666e37`) |
| W | `agent-host`: 13 tests | `random_token` reads `/dev/urandom` (`agent-host/src/mcp.rs:115`), so the MCP server never starts. | Fixed (`75ca92f`) |
| W | `keymap`: `docs_atajos_md_matches_the_registry` | Probably CRLF line endings from the Windows checkout; there is no `.gitattributes`. | Fixed (`a481639`, `.gitattributes`) |

## Findings

### Input: shortcuts and modifiers

| Sev. | Plat. | Where | What happens | Fix | Status |
| --- | --- | --- | --- | --- | --- |
| Blocker | W+L | `keymap/src/profile.rs:66-198` | All 45 `cmd-` bindings parse as the Win/Super key (GPUI `keystroke.rs:152`), and the OS takes most of those combos. Save, undo, zoom, octave and tab switching by key are lost. | Use `secondary-` (Ctrl off macOS); per-OS defaults only where conventions differ. Avoid `ctrl-alt` (AltGr on Windows). | Fixed (`27e92fe`); user shortcuts with Ctrl+Alt refused (`6fb9de0`) |
| Broken | W+L | `app-gpui/src/ui/text_input.rs:704-707` | `cmd-a/c/v/x` in text fields: Ctrl+C/V do nothing in comments, staff text, search or the agent prompt. | `secondary-`. | Fixed (`27e92fe`, `text_input.rs:705-708`) |
| Broken | W+L | `selection.rs:606,909`, `mark_drag.rs:419` | ⌘-click and ⌘-drag (hairpins, free mark move) and ⌘-arrow (nudge ×10) become Win/Super gestures that snap or move the window. | `modifiers.secondary()`. | Fixed (`32ba011`) |
| Broken | W+L | `app-gpui/src/keyboard.rs:107-111` | Shift+1…9 (add interval) only works on a US layout. On Spanish and other layouts Shift+2 reports `"`. | Map shifted digits per layout (`ToUnicodeEx` on Windows, xkbcommon on Linux). | Partial (`284b51e`): Windows asks `ToUnicodeEx`; Linux uses a table of common layouts and falls back to US (`keyboard.rs:149-159`) |
| Degraded | L | `selection.rs:597`, `profile.rs:189` | Alt+drag (slur) moves the window on Xfce, Cinnamon and MATE; Alt+Space opens the window menu on GNOME and KDE. | Choose other modifiers on Linux. | Fixed: Replay is Alt+Mayús+Space (`profile.rs:244-259`); slurs are Ctrl+drag (`ba531e0`) |
| Cosmetic | W+L | `keymap/src/keys.rs:226-236` | Labels use ⌃⌥⇧⌘. | `Ctrl+Mayús+Z` style off macOS. | Fixed (`27e92fe`, `keys.rs:313-345`) |
| Cosmetic | W+L | `selection.rs:433,460,672,801,845,872,921,926`; `main.rs:522,546,587,920`; `agent.rs:30-34`; `ui/agent_panel.rs:12`; `keymap/src/docs.rs:11` | Status hints hard-code ⌘Z, ⇧⌘P, ⌥←, «Levare › Preferencias…». | Build them from `shortcuts.label(command)`, as `practice.rs:34` does. | Fixed (`6246fba`, `221726c`) |
| Cosmetic | W, X11 | `selection.rs:75-79,268-271` | Ctrl+wheel zoom alternates between one and two steps per notch. | One step per `ScrollDelta::Lines` notch. | Fixed (`1860958`) |
| Broken | W+L | `keymap/src/profile.rs` | Ctrl+[ was the only shortcut for moving the playback line to the start. Spanish, German and French keyboards type [ with AltGr, which reaches Levare as Ctrl+Alt, so it can't be pressed. Found after the audit. | A shortcut every layout can type. | Fixed (`22232b7`): Ctrl+Inicio |
| Broken | W+L | `keymap/src/keymap.rs` | A ⌥⌘ shortcut the musician saved on macOS becomes Ctrl+Alt elsewhere: AltGr on Windows, often a desktop shortcut on Linux. Found after the audit. | Refuse it when recording, skip it when loading, and say so in Preferencias. | Fixed (`6fb9de0`) |

### Window, menus, desktop integration

| Sev. | Plat. | Where | What happens | Fix | Status |
| --- | --- | --- | --- | --- | --- |
| Blocker | W | `main.rs:3569-3573`, `ui.rs:470-475` | `appears_transparent` removes the caption. No `WindowControlArea` is set, so the window can't be dragged, minimized, maximized or closed with the mouse, and title double-click does nothing. Seen in CI. | Draw min/max/close with `WindowControlArea::{Min,Max,Close}`; mark the bar `Drag`. | Fixed (`125cc92`, `ui/window_frame.rs`) |
| Broken | L | `main.rs:3566-3574` | Server decorations by default. GNOME Wayland gives no frame, resize edges or buttons; KDE and X11 add a second titlebar above Levare's. | `WindowDecorations::Client`, then handle `window_decorations()`, `start_window_resize`, `show_window_menu`. | Fixed (`69d7577`) |
| Degraded | W | `main.rs:3564` | 1440×1000 window, centered but not clamped; on small screens it opens partly off-screen. Seen in CI. | Clamp to the display bounds. | Fixed (`996f819`, `window_frame.rs:74`) |
| Degraded | W+L | `menu_typography.rs:8` | Native menus are not drawn. Menu-only commands lose their only route: select all, full screen, next/previous instrument, right/left hand, divide/join part, clear form/articulation/technique, remove ending/tie/slur/hairpin, the three update commands. Command search still reaches all of them. | Put every command in the command palette and the in-window File menu; check `toolbar/src/spec.rs:1795` (`NATIVE_MENUS`). | Partial (`6c95baa`): updates, Preferencias and Salir are in the Archivo menu; the other menu-only commands are reached only through the command search |
| Degraded | W+L | `main.rs:3487` | The window title is set in every `render` (16 ms during playback). That is a synchronous X11 round trip and a `SetWindowTextW` each frame. | Set it only when it changes. | Fixed (`a482229`) |
| Cosmetic | W+L | `app_icon.rs:48` | No app icon. Windows loads icon resource 1 and there is no `build.rs`; Linux has no `app_id`. | Embed an `.rc` icon on Windows; set `app_id` and a window icon on Linux. | Fixed (`cac50a7`) |
| Cosmetic | W | `main.rs:1` | No `windows_subsystem = "windows"`, so a console window opens with the app. | Add it for release builds. | Fixed (`0bcf6a4`) |
| Cosmetic | L | `main.rs:2392` | The unsaved-changes prompt uses GPUI's plain fallback. | A custom `PromptBuilder`. | Open (now `main.rs:2505`) |
| Broken | W+L | `app-gpui/src/main.rs` | Double-clicking a score or using «Abrir con» started Levare with an empty score: it never read its arguments, and no system knew which files Levare opens. Found after the audit. | Open the files passed at start; declare the file types to each system. | Partial (`c052b19`, `app-gpui/src/system_open.rs`): macOS declares `.levare`, `.partitura` and MusicXML in `Info.plist`; Linux's `install.sh` installs `levare-mime.xml` and the desktop entry takes `%F`; on Windows «Abrir con» works through the arguments, but associating the file types needs an installer |
| Broken | W | `app-gpui/src/main.rs:1` | With no console (`windows_subsystem`), a panic or a window that can't open leaves no trace; on macOS a Levare opened from Finder only reaches the unified log. Found after the audit. | A log and crash reports in the data folder. | Fixed (`dac1963`, `8e64e9a`, `app-gpui/src/diagnostics.rs`): `registro.log`, `fallo-*.txt`, and a message box on Windows when the window can't open |

### Files, autosave, export

| Sev. | Plat. | Where | What happens | Fix | Status |
| --- | --- | --- | --- | --- | --- |
| Blocker | W | `score-io/src/recovery.rs:184-187` | `lock_file` always errors off unix, so there is no autosave and no crash recovery. Every launch reports a false «puede haber otra sesión activa». Restart-to-update refuses unsaved work. | `std::fs::File::try_lock`. | Fixed (`7666e37`, `recovery.rs:177-183`) |
| Broken | W+L | `score-pdf/src/lib.rs:33-37`, `score-export/src/lib.rs:17,98-103` | PDF is Quartz-only. The export picker has PDF ticked by default; after the user picks a folder, the whole export, MusicXML included, fails with «La exportación a PDF requiere macOS». | A portable PDF backend; until then, PDF off and disabled off macOS. | Fixed (`8e80939`): Windows and Linux use the portable krilla writer |
| Broken | L | GPUI `linux/platform.rs:291-415` | Open, Save As and Export need an xdg-desktop-portal FileChooser. Without one (minimal window managers), a new score can't be saved, and the error is in English. | Document the dependency and show a Spanish hint, or fall back to another picker. | Fixed: Spanish hint (`44112db`); the dependency is listed in `packaging/linux/README.txt:21` |
| Broken | W | `score-export/src/lib.rs:51-72`, `main.rs:2700` | Only `/ \ :` are replaced in file names. «¿Quién…?» breaks the suggested name and the export `create_dir`. | Also replace `<>"\|?*`, trailing dots and spaces, and reserved names (CON, NUL…). | Fixed (`ec4cc1f`, `score-io/src/names.rs`) |
| Broken | W | `score-io/src/lib.rs:346`, `musicxml.rs:503`, `recent.rs:57`, `recovery.rs:199` | `NamedTempFile::persist` fails with «Acceso denegado» while OneDrive, Defender or the indexer holds the target. | Retry on access-denied or sharing violations, or use `ReplaceFileW`. | Fixed (`346f1e2`, then `7396657`: every writer goes through `atomic-write`) |
| Degraded | W | `main.rs:2633-2637` | Save As starts in `current_dir()`, which is the install folder or System32 when launched from the Start menu. | `dirs::document_dir()`. | Fixed (`98d6bef`) |
| Degraded | W | tempfile `windows.rs:18-20` | `persist` fails on paths over 260 characters. | Shorter components, or a clear error. | Partial (`ec4cc1f`): names Levare builds are cut so its paths stay under 260 characters below a reasonably deep folder (`score-io/src/names.rs`); a deeper folder still fails with the system's error, which `atomic-write` doesn't explain (`atomic-write/src/lib.rs:198-219`) |
| Cosmetic | W | `score-io/src/recent.rs:63`, `shortcuts.rs:447-454` | Recent files show `\\?\C:\…` paths and mixed separators. | `dunce::canonicalize`; `Path::join` for labels. | Fixed (`0687876`) |

### Fonts and drawing

| Sev. | Plat. | Where | What happens | Fix | Status |
| --- | --- | --- | --- | --- | --- |
| Degraded | L | `paint.rs:259-620`, `ui.rs:1438,1514,1721`, `ui/toolbar.rs:58`, `ui/selection_panel.rs:533` | «Times New Roman», «Menlo» and `.SystemUIFont` (IBM Plex Sans) are usually missing on Linux. Score text loses its serif, bold and italic, and runs wider than layout estimates, so it overlaps. GPUI panics if no fallback font exists. No `add_fonts` call anywhere. | Bundle a Times-metric OFL serif (Liberation Serif or Tinos) and a mono, register with `add_fonts`, use them on every OS. | Fixed (`920a10c`, `2e5bb43`, `89cffc6`): Liberation Serif, Liberation Mono and Cantarell are registered (`app-gpui/src/fonts.rs`) |
| Degraded | W+L | `paint.rs:544-552` | At scale factor 1.0, 1.3 px × zoom lines fall below one device pixel and look faint and uneven; Retina hides it. Guess, not seen. | Snap horizontal lines to device pixels; minimum width of 1 device pixel. | Open: `paint.rs:544-552` still strokes `line.thickness * scale` unsnapped |
| Cosmetic | W+L | `toolbar/src/project.rs:73-90` | The ribbon measures its tiles with fixed per-character widths taken from the macOS interface font. Windows and Linux draw other fonts, and their key labels (`Ctrl+Mayús+Z`) are text rather than glyphs, so a tile can clip its label or a group can fold earlier than it needs to. Guess, not seen. | Measure with the platform's font, or keep one `Metrics` per platform. | Open |

### Sound

| Sev. | Plat. | Where | What happens | Fix | Status |
| --- | --- | --- | --- | --- | --- |
| Broken | W+L | `sound-vsco/src/manifest.rs:226-229,277-293`, `app-gpui/src/store.rs:107-111` | sfizz is only looked for as `libsfizz.dylib`, and is only built for macOS (`scripts/build_sfizz.sh` uses `sed -i ''`, `sysctl`, `lipo`, `shasum`). SFZ packs don't load and the store refuses to install. GeneralUser GS (embedded SF2) works. | Per-OS library name next to the exe; build `libsfizz.so` and `sfizz.dll` in the release (static CRT, or `LOAD_WITH_ALTERED_SEARCH_PATH`). | Fixed (`034a310`); sfizz is built per OS in the release |
| Broken | W | `sound-vsco/src/lib.rs:100` | `canonicalize()` gives `\\?\` paths. Windows doesn't resolve `..` inside them, so Virtual Playing Orchestra (`sample=..\libs\…`) won't load. | `dunce::canonicalize`. | Fixed (`034a310`, `sound-vsco/src/lib.rs:130-132`) |
| Degraded | W+L | `platform/src/lib.rs:122` | Any stream error counts as a disconnect, including recoverable ALSA errors after suspend. Possible worker spin (guess). | Only `DeviceNotAvailable` is fatal; reopen otherwise. | Fixed (`d2854b2`) |
| Degraded | W+L | workspace `Cargo.toml:35` | cpal without `audio_thread_priority`: the audio thread is not real-time off macOS. Expect dropouts on large scores. | Enable the feature (Linux needs `libdbus-1-dev`). | Fixed (`7028138`) |
| Degraded | L | `platform/src/renderer.rs:681`, `playback.rs:535-537` | The playhead follows rendered frames, not played frames. It runs about 100 ms or more ahead of the sound with ALSA's default buffer. | Subtract the output latency from `OutputCallbackInfo`. | Fixed (`86aabbe`) |
| Degraded | W | `platform/src/lib.rs:61-64` | Audio stays on the device that was default at start; plugging in headphones needs «Reintentar audio». Guess. | Reopen on default-device change. | Fixed (`537942d`) |
| Degraded | L | — | The binary links `libasound.so.2`, and PulseAudio/PipeWire are reached through ALSA "default". | List `libasound2` and the PipeWire ALSA plugin as requirements. | Fixed (`12d6239`, `packaging/linux/README.txt:18`) |
| Degraded | W | `sound-store/src/install.rs:151-154` | Installing or updating a pack can fail on transient locks (Defender, the indexer, sfizz streaming). Guess. | Release the pack first; retry rename and remove. | Fixed (`12c045e`) |
| Cosmetic | W | `sound-store/src/install.rs:358-375` | No free-space pre-check, and «No queda espacio en el disco» never shows (compares `libc::ENOSPC`). | `ErrorKind::StorageFull`, `GetDiskFreeSpaceExW`. | Fixed (`12c045e`, `install.rs:456-509`) |
| Cosmetic | W+L | `platform/Cargo.toml:11` | `midir` is unused but links ALSA seq and WinMM. | Remove it. | Fixed (`86ff01e`) |

### Agent panel

| Sev. | Plat. | Where | What happens | Fix | Status |
| --- | --- | --- | --- | --- | --- |
| Blocker | W | `agent-host/src/mcp.rs:115` | `random_token` reads `/dev/urandom`, so the MCP server fails with «path not found» and no agent session can start. Found by the CI tests. | `getrandom` (or the OS RNG through an existing dependency). | Fixed (`75ca92f`) |
| Blocker | W | `agent-host/src/agents.rs:91-138,180-208` | The agent lookup:<ul><li>probes `$SHELL -ilc`, falling back to `/bin/zsh`;</li><li>reads `HOME`;</li><li>splits `PATH` on `:`;</li><li>looks for bare `npx`, `claude`, `gemini`;</li><li>passes a `:`-joined `PATH` to the child.</li></ul>Claude, Codex and Gemini are never found. | Skip the shell probe on Windows; `split_paths`/`join_paths`; `PATHEXT` (`.exe`, `.cmd`). | Fixed (`ec4af8c`) |
| Degraded | W | `agent-client-protocol-3.5.0/src/acp_agent.rs:262-326` | Stopping an agent kills only `cmd.exe`; `node` and `claude` are left running. | A Job Object with `KILL_ON_JOB_CLOSE`. | Fixed (`f764ac7`, `agent-host/src/job.rs`) |
| Broken | L (Wayland) | `app-gpui/src/clipboard.rs:31-46` | Cut writes, then reads back to verify. Wayland answers with the old offer, so Cut reports «No se pudo verificar la copia». | Skip the read-back on Linux. | Fixed (`760dd51`) |

### Updates and release

| Sev. | Plat. | Where | What happens | Fix | Status |
| --- | --- | --- | --- | --- | --- |
| Broken | W+L | `update/src/manifest.rs:39-47`, `update/src/install.rs` | No platform key, so updates are silently off; «Buscar actualizaciones…» says none are published. Install is `.app`-only (`ditto`, `codesign`). | `linux-x86_64` and `windows-x86_64` keys; a binary install that renames the running exe aside and rolls back on failure. | Fixed (`e774614`) |
| Cosmetic | W+L | `updates.rs:359-376` | Turning automatic checks on says Levare will check. | Hide the update commands until supported. | Fixed: updates work on all three (`e774614`) |
| — | W+L | `.github/workflows/release.yml`, `scripts/build_sfizz.sh`, `.github/scripts/update-manifest.sh` | The release only builds macOS. | Linux and Windows jobs, sfizz per OS, packaging (Linux `.tar.gz`, Windows `.zip`), manifest keys. | Fixed (`d531583`) |
| Blocker | W | `.cargo/config.toml` | `levare.exe` linked the C runtime dynamically and needed `VCRUNTIME140.dll`, which the download doesn't ship: on a PC without the Visual C++ Redistributable Levare didn't start. CI runners have it, so nothing noticed. Found after the audit. | Link the CRT statically, as `sfizz.dll` already does. | Fixed (`587a15b`): `+crt-static`, and the release fails if `levare.exe` imports a runtime DLL |
| — | W+L | `.github/workflows/release.yml` | CI opens a debug build without sfizz, and the sfizz tests are ignored there. A missing file, a library that doesn't load or an executable that quits at once would reach musicians unnoticed. Found after the audit. | Launch the packaged download and load its SFZ engine before publishing. | Fixed (`71640f1`, `b5de7e2`) |
| Degraded | W | `.github/workflows/release.yml` | Windows gets a `.zip` with no installer and no code signature: SmartScreen may warn on first launch, there is no Start menu entry, and the file types aren't associated. | An installer (MSIX or MSI) and an Authenticode signature. | Open |

## Checked and fine

- Data folder: `dirs::data_local_dir()` gives `~/.local/share/Levare` on Linux and `%LOCALAPPDATA%\Levare` on Windows.
- `.levare`, `.partitura` and MusicXML: BOM, CRLF and UTF-8 handling. `.levare` and MXL unpack in memory. Export publishing via `create_dir` and `rename` works on Windows.
- Bravura is drawn from its own outlines, so it needs no installed font. Path rendering is the same on Metal, Vulkan and DirectX.
- Audio formats, sample rates, channel counts and buffer sizes come from the device; no fixed 44.1/48 kHz. With no output device the app carries on (seen in both CI launches).
- The 3,967 sample references in the built packs match the case of the files on disk exactly.
- Zip extraction uses `enclosed_name`; symlinks are skipped.
- Agent spawn uses `CREATE_NO_WINDOW` and piped stdio. The MCP server binds to 127.0.0.1.
- Quit and last-window-closed, IME in text fields, right-click menus, double-click, key repeat on X11, and Alt shortcuts all behave.

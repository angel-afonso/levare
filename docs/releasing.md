# Releasing

Levare is released on GitHub Releases for macOS, Windows and Linux at the same time,
from the same commit and with the same version. The version and the changelog come from
the commit messages (see `AGENTS.md`).

## The three workflows

| Workflow | When | What it does |
| --- | --- | --- |
| **CI** (`ci.yml`) | Every push to `main` and every pull request | Formatting; clippy and tests on macOS, Linux and Windows; on Linux and Windows it also opens the app for 30 seconds and keeps a screenshot. |
| **Release PR** (`release-pr.yml`) | By hand: Actions → Release PR → Run workflow, from `main` | release-plz opens (or updates) the "chore: release X.Y.Z" pull request, which raises `[workspace.package] version` and adds the entries to `CHANGELOG.md`. |
| **Release** (`release.yml`) | Every push to `main`; a `v*` tag; or by hand | If the `Cargo.toml` version has no tag yet, tags it, builds every platform, signs and publishes. If it has one, does nothing. |

To release: run Release PR, review the pull request (the changelog above all) and merge
it. The push to `main` that makes does the rest.

Merge pull requests with a merge commit or by rebasing, never by squashing; squash
merging is turned off in the repository. A squash leaves one commit titled after the pull
request, so release-plz sees none of its conventional commits.

### A new crate

release-plz finds each crate in the workspace as it was at the last `v*` tag and stops
with `cannot find package "<name>" in workspace` when a crate was added since. Until the
first release that includes it is tagged, give the crate a `git_tag_name` of its own in
`release-plz.toml` that no tag matches, such as `"<name>-v{{ version }}"`, so it counts
as a first release. Then remove that line, or the next releases list all of the crate's
commits again.

### A test build

Running **Release** by hand (Actions → Release → Run workflow, or
`gh workflow run release.yml --ref <branch>`) never tags or publishes, from any branch.
It builds every archive and keeps each as a workflow artifact, so a branch can check the
packaging before it is merged. With `UPDATE_SIGNING_KEY` set, it also signs the archives
and writes a `latest.json` for the commit (artifact `levare-update`), which tests the
signing; nothing reads it.

## What a release publishes

| File | For |
| --- | --- |
| `levare-vX.Y.Z-universal-apple-darwin.dmg` | macOS installer: drag Levare to Applications. |
| `levare-vX.Y.Z-universal-apple-darwin.zip` | The same `Levare.app`; what the macOS updater downloads. |
| `levare-vX.Y.Z-x86_64-pc-windows-msvc.zip` | Windows: a `Levare\` folder with `levare.exe`, `sfizz.dll` and `licenses\`. Unzip it anywhere you can write and run `levare.exe`; the updater downloads the same file. |
| `levare-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz` | Linux: a `levare/` folder with `levare`, `libsfizz.so`, `dev.partitura.editor.desktop`, `levare-mime.xml`, `levare.png`, `install.sh`, `README.txt` and `licenses/`. `./install.sh` adds Levare to the user's applications menu and registers the `.levare` and `.partitura` file types (`--uninstall` removes both); the updater downloads the same file. |
| `latest.json` and one `….minisig` per archive | The manifest the app reads and each archive's signature. |
| `SHA256SUMS` | Checksums of all of the above. |

- **macOS**: one universal app for Apple Silicon and Intel (both architectures built
  separately, then merged with `lipo`), macOS 11 or newer.
- **Windows**: x86_64, built with MSVC. `levare.exe` (through `.cargo/config.toml`) and
  `sfizz.dll` link the C runtime statically, so the folder needs no Visual C++
  redistributable; the workflow fails if `levare.exe` imports the runtime's DLLs.
- **Linux**: x86_64, built on Ubuntu 22.04, so it runs on distributions with glibc 2.35
  or newer (the build logs the newest glibc symbol it uses). It needs ALSA (PulseAudio and
  PipeWire are reached through ALSA), `libdbus-1-3` (real-time priority for the audio
  thread), a Vulkan driver, and xdg-desktop-portal for the open and save dialogs. The desktop entry is named after Levare's app id (`dev.partitura.editor`, also its
  `StartupWMClass`), so the desktop matches the window to its entry and icon.

### The sfizz engine

Each release ships sfizz 1.2.3, the SFZ engine for the sound packs, built in the
workflow by `scripts/build_sfizz.sh` from the official source archive (checked against
its SHA-256; see `docs/vsco.md`):

| System | Library | Where it goes |
| --- | --- | --- |
| macOS | `libsfizz.dylib`, universal (arm64 + x86_64), macOS 11+ | `Levare.app/Contents/Frameworks/` |
| Windows | `sfizz.dll`, x64, static C runtime (`/MT`) | Next to `levare.exe` |
| Linux | `libsfizz.so`, x86_64, libstdc++ linked in (needs only glibc) | Next to `levare` |

The script needs cmake and a C++ compiler (Xcode's command line tools, GCC, or Visual
Studio from Git Bash on Windows). Its licenses go into `licenses/sfizz`. The workflow
caches each system's build on the script's hash, so sfizz is only rebuilt when the
script changes.

### Licenses

Every download carries a `licenses/` folder: `Levare.app/Contents/Resources/licenses`
on macOS, next to the executable on Windows and Linux. The Licenses job writes it with
`.github/scripts/collect-licenses.sh`, and sfizz's notices are added to it:

| Path | What |
| --- | --- |
| `Levare-LICENSE.txt` | Levare's MIT license (`LICENSE`). |
| `THIRD_PARTY_NOTICES.html` | Every Rust crate built into Levare with its license, from [cargo-about](https://github.com/EmbarkStudios/cargo-about) with `about.toml` and `about.hbs`. |
| `fonts/` | Bravura, Cantarell and Liberation (SIL OFL 1.1) and `assets/fonts/README.md`. |
| `GeneralUser-GS/` | The bundled SoundFont's license and origin. |
| `sfizz/` | sfizz's license and its dependencies' notices. |

cargo-about fails the job when a crate's license is not in `about.toml`'s `accepted`
list. `deny.toml` allows the same list, so CI's Dependencies job (cargo-deny) catches a
new license on the pull request that brings it; add one to both only after checking it
allows shipping Levare as it is.

### Checks on the Windows and Linux downloads

Before an archive is kept, its job unpacks it into a fresh folder and:

- opens that Levare for 30 seconds, like CI opens its debug build (on Linux with
  `.github/scripts/launch-linux.sh`, on X11 and Wayland); the job fails if it quits.
  The log and screenshots are kept as the `launch-<target>` artifact;
- runs the ignored sfizz test
  `sfizz::tests::the_bundled_sfizz_engine_loads_a_patch_and_renders_it` with
  `LEVARE_SFIZZ` naming the unpacked library, which loads a patch and renders a note.

## Repository secrets

Settings → Secrets and variables → Actions.

| Secret | Required | What it is |
| --- | --- | --- |
| `UPDATE_SIGNING_KEY` | Yes, to release | The whole minisign secret key file (no password) whose public key is `crates/app-gpui/assets/update.pub`. Without it, a push to `main` does not tag, build or publish anything (Release only warns). |
| `MACOS_CERTIFICATE` | No | The "Developer ID Application" certificate exported as `.p12`, in base64 (`base64 -i cert.p12`). |
| `MACOS_CERTIFICATE_PASSWORD` | With the above | The `.p12`'s password. |
| `APPLE_API_KEY` | No | The App Store Connect API key (`AuthKey_….p8`) in base64, for notarization. |
| `APPLE_API_KEY_ID`, `APPLE_API_ISSUER_ID` | With the above | Its Key ID and Issuer ID. |

Without the Apple secrets the macOS app gets an ad-hoc signature and is not notarized:
the first time it has to be opened with right-click → Open. With them, Gatekeeper opens
it without a warning.

The Windows and Linux builds are not code-signed. Windows SmartScreen may warn the first
time `levare.exe` runs ("More info → Run anyway"). Updates are still verified by their
minisign signature, as on macOS.

Release PR also needs Actions to be allowed to open pull requests: Settings → Actions →
General → "Allow GitHub Actions to create and approve pull requests".

### The update signing key

The app only installs an archive signed with the secret key whose public half it carries
(`crates/app-gpui/assets/update.pub`), and only if the signature's trusted comment names
that version and that file (`levare X.Y.Z <file>`). **Keep a copy of the secret key off
the computer**: if it is lost, installed copies will not accept any update signed with
another key and have to be reinstalled by hand.

To change it: generate a new pair without a password (`minisign -G -W -p update.pub -s
update.key`), replace `crates/app-gpui/assets/update.pub`, update the secret and release
a version; installed copies need that version installed by hand.

## The in-app updater

`latest.json` lists one archive per platform key: `darwin-aarch64` and `darwin-x86_64`
(both the universal zip), `linux-x86_64` and `windows-x86_64`. Each copy of Levare reads
only its own key, so adding platforms does not affect copies already installed (0.1.0 on
macOS reads the new manifests as before).

In a release build:

- Ten seconds after it opens, and every six hours, it looks for a newer version in
  `https://github.com/angel-afonso/levare/releases/latest/download/latest.json`,
  downloads it into the data folder (`actualizaciones/`, emptied when Levare opens) and
  checks its SHA-256 and its signature. The status bar shows the progress and, at the
  end, «Reiniciar para actualizar a X.Y.Z».
- **Reiniciar para actualizar** (the status bar item, or the command) unpacks the archive
  next to the installation and swaps it in, then restarts. It does not restart while the
  agent is working, something is being saved or exported or sounds are being installed,
  nor with unsaved changes if the autosave is unavailable: the status bar says so.
  Unsaved work comes back through the autosave; a saved score is opened again.
- **Buscar actualizaciones…** checks right away and says what it found.
- **Activar o desactivar la búsqueda automática de actualizaciones** is saved in
  `actualizaciones.json`, in the data folder. The first automatic check says in the
  status bar that Levare looks on GitHub every 6 hours and names this command; the same
  file remembers that it was said (`avisado`).

The commands are in the Levare menu on macOS and in the in-window File menu on Windows and
Linux; command search finds them everywhere.

### How each system installs an update

- **macOS**: `ditto` unpacks `Levare.app`. Its code signature must verify, and come from
  the same Apple team as the installed app if that one has a team. The installed bundle
  is renamed to `.Levare.app.old`, the new one moved into its place (the old one is put
  back if that fails), and GPUI reopens the bundle.
- **Windows and Linux**: the archive (zip or tar.gz, unpacked in Rust; links and paths
  leaving the folder are skipped) must contain the executable. Every file of its folder
  replaces the one of the same name in the folder of the running executable:
  `levare.exe` and `sfizz.dll`, or `levare` and `libsfizz.so`, plus the licenses and the
  desktop files. Each replaced file is first renamed to `.<name>.old`; Windows can rename
  a running executable or a loaded DLL but not overwrite or delete it. If any step fails,
  every file moved so far is taken out and the old ones are put back. The `.old` files
  are deleted at once where the system allows it, and otherwise the next time Levare
  starts. Levare then starts the new executable once the old process has exited, and
  quits.

Levare does not update itself when:

- it is a development build (`cargo run`), or runs from cargo's `target/` folder (for
  example `cargo run --release`);
- on macOS, it is not running from `Levare.app`;
- on Windows and Linux, its folder is not writable (for example an install under `/usr`,
  `/opt` or `C:\Program Files`). The status bar says to install the new version from the
  releases page or move Levare to a folder of the user's.

«Buscar actualizaciones…» says why in each case.

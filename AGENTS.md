# Instructions for agents

## Language

Everything at the software level is written in English: code, comments, commit
messages, the changelog, docs, pull requests and CI messages. The app's interface stays
in Spanish: labels, menus, status messages and errors shown to the musician. Older docs
in Spanish move to English when they are next changed.

## Platforms

Levare is built, tested and released for macOS, Windows and Linux alike. None of them is
the main one: a change is finished when it works on all three.

- Shortcuts use `secondary-` (⌘ on macOS, Ctrl elsewhere), never `cmd-`. A shortcut gets
  per-platform defaults only where the platforms' conventions differ.
- Labels shown to the musician come from the keymap (`⇧⌘Z` on macOS, `Ctrl+Mayús+Z`
  elsewhere), never hard-coded glyphs.
- Platform code sits behind `cfg(target_os = …)` with a working branch for every
  platform, not just a stub that fails.
- Paths go through `Path::join`, `std::env::split_paths` and `dirs`; never assume `/`,
  `:` or a Unix tool.

Without a Linux or Windows machine, `cargo check` and `cargo clippy` can still target
them from macOS (`--target x86_64-pc-windows-msvc`, `--target x86_64-unknown-linux-gnu`),
and CI runs the tests and opens the app on all three.

## Commit messages are the changelog

Levare's version and its `CHANGELOG.md` come from the commit messages, through
[release-plz](https://release-plz.dev) (configured in `release-plz.toml`). There is no
changes file to add: each commit is its own entry. When the Release PR workflow runs,
release-plz reads the commits since the last `vX.Y.Z` tag, picks the next version from
their types and copies their titles into `CHANGELOG.md` in a release pull request.
`docs/releasing.md` describes the whole process.

So every commit that changes behavior is a
[Conventional Commit](https://www.conventionalcommits.org/):

```
<type>(<scope>): <title>
```

### Type: decides the version

| Type | For | Version |
| --- | --- | --- |
| `feat` | Something new the musician can do or see | minor (0.4.2 → 0.5.0) |
| `fix` | A bug fixed: a wrong result, a crash, lost data | patch (0.4.2 → 0.4.3) |
| `perf` | The same, faster or lighter | patch |
| `refactor` | Code reorganized, no change in behavior | patch |
| `docs` | Documentation only | patch |
| `chore` | Maintenance: dependencies, configuration | patch |
| `ci`, `test`, `style` | CI, tests or formatting only | patch, left out of the changelog |

A change that breaks something users rely on (the `.levare` format, saved shortcuts, a
sound pack) takes `!` after the scope, `feat(file)!: …`, and explains what breaks in a
`BREAKING CHANGE:` footer. Before 1.0 it raises the minor version.

### Scope: where it changed

The part of the app the musician notices, in lowercase: `score` (writing and editing),
`ribbon`, `panels`, `agent`, `playback`, `sounds`, `file` (open, save, export, import),
`shortcuts`, `review`, `layout`, `updates`, `window`. For an internal change with no
visible part, the crate's directory name (`score-core`, `keymap`, `update`…). No scope if
it spans several.

Only commits that touch a crate (`crates/…`, including its `Cargo.toml`) count toward a
release and appear in the changelog. Changes only to `docs/`, the README, `scripts/` or
CI publish nothing; still use `docs:`, `ci:` or `chore:` for them.

### Title: it is the changelog line

The title goes into `CHANGELOG.md` as is, so write it for the people who use Levare:

- Say what changes for the musician, in the present tense, lowercase, with no final
  period: `fix(panels): remember the chosen tab when Levare reopens`, not
  `fix: fix in panels.rs`.
- One change per commit. Two unrelated fixes are two commits, each with its own line and
  type.
- The why and the details go in the body; only the title reaches the changelog.

### Never by hand

- Don't change `[workspace.package] version` in `Cargo.toml` or edit `CHANGELOG.md`: the
  release pull request does both.
- Don't create `v*` tags: when the release pull request is merged, the Release workflow
  tags and publishes the version.

## Before calling something done

What CI checks (README, «Verifying»), besides the cargo-deny dependency check:

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

`docs/atajos.md` is generated from `crates/keymap`; if you change commands or shortcuts,
regenerate it with `LEVARE_ACTUALIZAR_ATAJOS=1 cargo test -p score-keymap`.

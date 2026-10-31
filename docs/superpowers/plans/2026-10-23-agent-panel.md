# Agent panel: implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A conversation side panel in Levare with an agent (Claude Code, Codex or Gemini CLI over ACP) that reads and edits the open score with the editor's own actions.

**Architecture:** Three new crates without GPUI: `score-text` (compact text notation and musical positions), `agent-tools` (tools on top of the `Editor`, independent of the transport) and `agent-host` (HTTP MCP server on 127.0.0.1 with `rmcp` and an ACP client with `agent-client-protocol`, on its own tokio runtime). `app-gpui` provides the panel, the locking of the score during the turn and the bridge that runs on the interface thread each tool call that arrives over a channel.

**Tech Stack:** Rust 2024, GPUI `=0.2.2`, `agent-client-protocol =3.5.0` (feature `process`), `rmcp =3.7.0` (features `server`, `macros`, `transport-streamable-http-server`), `tokio =1.53.3` (already in `Cargo.lock`), `schemars` in the version that `rmcp 3.7.0` requires, `futures`.

**Spec:** [docs/superpowers/specs/2026-10-23-agent-panel-design.md](../specs/2026-10-23-agent-panel-design.md)

## Global Constraints

- Dependencies with an exact version (`=x.y.z`) declared in `[workspace.dependencies]` of the root `Cargo.toml` and used with `x.workspace = true`.
- GPUI only in `app-gpui`; `score-text`, `agent-tools` and `agent-host` do not depend on GPUI (`docs/development.md:32`).
- `cargo fmt --all --check`, `cargo test --workspace --locked` and `cargo clippy --workspace --all-targets --locked -- -D warnings` must pass.
- Interface texts and messages to the agent in Spanish; Rust identifiers in English like the rest of the code; vocabulary from `CONTEXT.md` (part, written pitch, sounding pitch).
- The agent never runs commands from the Archivo (File) or Vista (View) categories, nor undo/redo, nor commands that open dialogs or text fields.
- Everything the agent does in a turn is undone with a single ⌘Z; while the turn lasts the user cannot edit, undo or redo, but can play back, scroll and zoom.
- When the turn ends or is stopped, cursor and selection return to where the user left them.
- With 32 parts the interface stays smooth: the interface thread only applies edits; network, processes and I/O stay off it.
- New things with command IDs follow the rule in `docs/development.md`: a constant in `id::`, an entry in the registry, a branch in the `match` of `AppView::run` and `docs/shortcuts.md` regenerated.

## Review Focus

1. **Ambiguous part name** («violines» with Violín I and Violín II): the tool returns an error that lists the candidates and never picks one. Test in Task 3.
2. **A turn that ends badly** (the agent dies, the user presses Detener (Stop) halfway through `escribir`, ACP error): a single undo entry or none is left, the selection is restored and the score is unlocked. Test in Task 7.
3. **Agent not logged in, or no network for `npx`**: the panel shows an error in Spanish with the command to log in, and nothing is left locked. Test in Task 6.
4. **App opened from Finder** (without the terminal's `PATH`): `npx`, `node`, `claude`, `codex` and `gemini` are looked up with the login shell, and an agent that is missing something shows up as unavailable with the reason. Test in Task 6.
5. **Toggle on an existing mark** (asking for «trino» where there already is one): `ejecutar` returns the before and after of the bar so that the agent sees that it removed it. Test in Task 4.

---

### Task 1: Grouped history in `score-edit`

**Files:**
- Modify: `crates/score-edit/src/lib.rs` (next to `execute_joined`, ~1341)
- Create: `crates/score-edit/src/group_tests.rs` (declared with `#[cfg(test)] mod group_tests;`)

**Interfaces:**
- Produces:
  - `pub struct HistoryMark` (opaque: holds `undo.len()` and the `Snapshot` of that moment).
  - `Editor::mark(&self) -> HistoryMark`
  - `Editor::rollback(&mut self, mark: &HistoryMark)`: goes back to the mark's state, truncates `undo` to its length and empties `redo`.
  - `Editor::squash(&mut self, mark: &HistoryMark) -> bool`: leaves at most one undo entry after the mark (the mark's own snapshot) and returns whether there were changes. If `undo.len() <= mark` it does nothing and returns `false`.
  - `Editor::restore_selection(&mut self, mark: &HistoryMark)`: copies cursor, selection, `range_anchor`, `divisi_anchor` and `tone` from the mark to the current state without creating an undo entry.

- [ ] **Step 1: Write the failing tests** in `group_tests.rs`:
  - `squash_turns_many_edits_into_one_undo`: three `execute(Command::Write(..))` after `mark()`; `squash` → `true`; a single `undo()` returns the score equal to the mark's and `can_undo()` goes back to its value before the mark.
  - `squash_without_changes_adds_nothing`: `mark()` + `squash` → `false`; `can_undo()` unchanged.
  - `rollback_discards_partial_edits`: two edits, `rollback` → `score()` equal to the mark's and `can_redo() == false`.
  - `restore_selection_keeps_score_and_history`: change the selection and the score, `squash`, `restore_selection` → the selection is the mark's, the score keeps the changes and an `undo()` still leads to the previous state.
- [ ] **Step 2: Run** `cargo test -p score-edit group_tests`. Expected: FAIL (methods do not exist).
- [ ] **Step 3: Implement** the four methods and `HistoryMark` in `lib.rs`.
- [ ] **Step 4: Run** `cargo test -p score-edit`. Expected: PASS.
- [ ] **Step 5: Commit** `git commit -m "History: group and revert edits from a mark"`

### Task 2: Crate `score-text`, compact notation

**Files:**
- Create: `crates/score-text/Cargo.toml` (depends on `score-core`), `crates/score-text/src/lib.rs`, `crates/score-text/src/notation.rs`, `crates/score-text/src/notation_tests.rs`
- Modify: root `Cargo.toml` (`score-text = { path = "crates/score-text" }` in `[workspace.dependencies]`)

**Interfaces:**
- Produces:
  - `pub struct WrittenEvent { pub event: score_core::Event, pub tie_to_next: bool }`
  - `pub struct TextError { pub offset: usize, pub message: String }` (message in Spanish; `impl Display`)
  - `pub fn parse_voice(text: &str) -> Result<Vec<WrittenEvent>, TextError>`
  - `pub fn format_voice(score: &Score, part: PartId, staff: StaffId, measure: usize, voice: Voice) -> String`

Grammar (written pitches, scientific octave: `C4` = middle C, like `score_playback::midi_key`):

```
voz      := evento (" " evento)*
evento   := (altura | acorde | "R") ":" duracion ["~"] [marcas]
altura   := [A-G] ["#" | "b" | "n"] [0-8]
acorde   := "[" altura (" " altura)+ "]"
duracion := ("1"|"2"|"4"|"8"|"16"|"32") ["."] ["t"]      // "t" = triplet
marcas   := "{" texto "}"                                 // informational only
```

`format_voice` writes the event's marks between braces with the Spanish labels that already exist in `score-core` (articulation, dynamic, technique, ornament, start and end of slurs and hairpins…), separated by commas. `parse_voice` accepts and discards the `{…}` blocks, so the agent can send back what it read.

- [ ] **Step 1: Write the failing tests** in `notation_tests.rs`:
  - `parses_notes_rests_chords`: `"C4:4 R:8 Eb5:8 [C4 E4 G4]:2"` → 4 events; the third has `Accidental::Flat` and octave 5; the fourth, 3 pitches and a half-note duration.
  - `parses_dots_triplets_ties`: `"D4:4.~ D4:8 E4:8t"` → dot on the 1st, `tie_to_next` on the 1st, triplet on the 3rd.
  - `ignores_mark_blocks`: `"C5:4{Trino, f} D5:4"` → 2 events.
  - `rejects_bad_tokens`: `"H4:4"`, `"C4:3"`, `"C9:4"` and `"C4"` → `Err` with `offset` at the token and a message that names it.
  - `format_round_trips`: write with `Editor` a 4/4 bar with a dotted note, a chord, a rest and a dynamic; `parse_voice(format_voice(..))` gives the same `Event`s.
  - `formats_written_pitch_for_transposing_parts`: B♭ clarinet with a sounding C written in → `D` comes out (written pitch).
- [ ] **Step 2: Run** `cargo test -p score-text`. Expected: FAIL.
- [ ] **Step 3: Implement** `notation.rs` (hand-written parser, no new dependencies).
- [ ] **Step 4: Run** `cargo test -p score-text`. Expected: PASS.
- [ ] **Step 5: Commit** `git commit -m "Compact text notation to read and write bars"`

### Task 3: Musical positions in `score-text`

**Files:**
- Create: `crates/score-text/src/position.rs`, `crates/score-text/src/position_tests.rs`

**Interfaces:**
- Consumes: `Score::part_name(id)` (`score-core/src/instrument.rs:196`), `Score::time_at`, `Measure::timed_iter`.
- Produces:
  - `pub fn resolve_part(score: &Score, query: &str) -> Result<PartId, String>`: 1-based number or name; ignoring case and accents; accepts a prefix and a simple plural («violas» → «Viola»). If several parts match, `Err` with all the candidates numbered.
  - `pub fn bar_index(score: &Score, bar: u32) -> Result<usize, String>`: bar number as Levare shows it (look at how `score-layout` numbers and respect the pickup bar) → 0-based index.
  - `pub fn beat_tick(score: &Score, measure: usize, beat: f32) -> Result<u16, String>`: 1-based beat in units of the denominator (in 6/8 it counts eighth notes); `tick = (beat − 1) · 192 / den`. Error if it is not an integer or if it falls outside the bar's duration.
  - `pub fn event_at(score: &Score, part: PartId, staff: StaffId, voice: Voice, measure: usize, tick: u16) -> Result<ElementId, String>`: if no event starts at that tick, the error lists the beats where events do start («empiezan en 1, 2, 3, 4»).

- [ ] **Step 1: Write the failing tests**:
  - `resolves_part_by_number_and_name`: «2», «viola», «VIOLAS», «violín» → the right part.
  - `ambiguous_part_lists_candidates`: with Violín I and Violín II, `"violín"` → `Err` containing both names and their numbers.
  - `beat_ticks_in_simple_and_compound`: 4/4 beat 3 → 96; 6/8 beat 4 → 72; 4/4 beat 2.5 → 72.
  - `beat_outside_measure_fails`: 3/4 beat 4 → `Err`.
  - `event_at_reports_onsets`: a bar of four quarter notes, tick 24 → `Err` containing «1, 2, 3, 4».
  - `pickup_bar_numbering`: a score whose first bar is shorter (like the one MusicXML import creates) → `bar_index` matches the numbering that Levare shows.
- [ ] **Step 2: Run** `cargo test -p score-text position`. Expected: FAIL.
- [ ] **Step 3: Implement** `position.rs`.
- [ ] **Step 4: Run** `cargo test -p score-text`. Expected: PASS.
- [ ] **Step 5: Commit** `git commit -m "Musical positions: part, bar and beat"`

### Task 4: Crate `agent-tools`

**Files:**
- Create: `crates/agent-tools/Cargo.toml` (depends on `score-core`, `score-edit`, `score-text`, `score-keymap`, `serde`, `serde_json` and `schemars`; `score-toolbar` and `score-io` only in dev-dependencies), `src/lib.rs`, `src/catalog.rs`, `src/tools.rs`, `src/instructions.rs`, `src/tools_tests.rs`, `src/catalog_tests.rs`
- Modify: root `Cargo.toml`

**Interfaces:**
- Consumes: Tasks 1 to 3.
- Produces:
  - `pub trait Host { fn editor(&self) -> &Editor; fn editor_mut(&mut self) -> &mut Editor; fn run(&mut self, id: CommandId) -> Result<(), String>; }`
  - `pub fn allowed_commands(registry: &Registry) -> Vec<&CommandSpec>`: categories Edición (Editing), Entrada de notas (Note input), Duración (Duration), Alteraciones (Accidentals), Expresión (Expression), Notación (Notation), Compases (Bars), Instrumentos (Instruments), Navegación (Navigation), Revisión (Review) and Reproducción (Playback); state `Ready`; minus `EXCLUDED`.
  - `pub const EXCLUDED: &[CommandId]`: undo, redo and every command that opens an interface (command search, score information, go to bar, full screen, text, lyrics and chord-symbol editors, instrument pickers and panel toggles; see `main.rs` 1366, 1428, 1573, 1583, 1682 and 1722).
  - `#[derive(Deserialize, JsonSchema)] pub struct Position { parte: String, compas: u32, #[serde(default = "first_beat")] tiempo: f32, pentagrama: Option<u8>, voz: Option<u8> }` (`fn first_beat() -> f32 { 1.0 }`)
  - `pub enum ToolCall`, with one arguments struct per variant (`LeerArgs`, `SeleccionarArgs`, …; `AjustarArgs` is an enum with `#[serde(tag = "tipo", rename_all = "snake_case")]`). Each struct is `Deserialize + JsonSchema`, and its schema is the one the MCP announces for that tool. The variants are:

    | Variant | Arguments | What it does |
    |---|---|---|
    | `Resumen` | — | Title, numbered parts with their staves, number of bars, initial time signature, key signature and tempo and their changes, and the current selection. |
    | `Leer` | `partes: Option<Vec<String>>, desde: u32, hasta: u32` | At most 32 bars per call. One line per part, staff, voice and bar with `format_voice`, and before them, the changes of time signature, key signature, tempo and rehearsal numbers. |
    | `Seleccionar` | `desde: Position, hasta: Option<Position>` | Selects the event or the range and returns the formatted bar with `»` in front of the selected event. |
    | `Comandos` | `categoria: Option<String>` | Lists `id — label` from `allowed_commands`, grouped by category. |
    | `Ejecutar` | `comando: String` | Exact id or label; runs `Host::run` and returns «sin cambios», or the selection's bar as «antes:» and «después:». |
    | `Escribir` | `parte, pentagrama?, voz?, desde_compas: u32, compases: Vec<String>` | Each string is a complete bar in the notation from Task 2. Validates that each bar exactly fills its duration. If `desde_compas` is the one after the last, it adds bars (`AppendMeasure`); beyond that, error. Writes with `WriteAt` and joins the `~` ties with `AddConnection(Tie)`, also across bars. All inside `mark`/`rollback`/`squash`: either it is applied whole or not at all, and the error says which bar failed and why. |
    | `Ajustar` | `#[serde(tag = "tipo")]` with the subvariants below | Adjustments with parameters. |

    Subvariants of `Ajustar`:
    - `compas_tiempo{compas, numerador, denominador}`
    - `tonalidad{compas, quintas: i8, menor: bool}` (with `KeySignature::new`)
    - `tempo{compas, bpm, texto?}`
    - `clave{parte, pentagrama, compas, clave}`
    - `anadir_partes{instrumentos: Vec<String>}` (looks up by `Instrument::name()` ignoring accents; if a name is not recognized, an error with the similar ones)
    - `quitar_parte{parte}`
    - `conexion{tipo: ligadura|union|crescendo|diminuendo|octava_alta|octava_baja|pedal, desde: Position, hasta: Position}`
    - `texto{posicion, clase: expresion|tecnica, texto}`
    - `titulo{titulo}`
  - `pub fn dispatch(host: &mut dyn Host, registry: &Registry, call: ToolCall) -> Result<String, String>`
  - `pub fn tool_descriptions() -> Vec<(&'static str, &'static str)>`: name and description in Spanish of each tool, including the grammar and the written-pitch convention, so that the MCP announces them.
  - `pub const INSTRUCTIONS: &str`, which is prepended to the session's first message and says:
    - use only Levare's tools and start with `resumen`;
    - read before using a toggle;
    - ask if the part is ambiguous;
    - pitches are written pitches;
    - do not touch files;
    - answer briefly and in the user's language.

- [ ] **Step 1: Write the failing tests** with a `FakeHost` (real Editor; `run` translates the `MarkKind`s with `score_toolbar::marks::kind` into `Command::ToggleMark` and returns `Err` for anything else):
  - `catalog_excludes_file_view_undo_and_dialogs`: no id in `allowed_commands` is from Archivo or Vista, nor undo, nor in `EXCLUDED`; every id in `EXCLUDED` exists in `Registry::builtin()`.
  - `select_then_toggle_reports_before_after`: `Seleccionar` beat 2 of bar 1 and `Ejecutar` the trill twice; the 1st reply has the trill under «después:» and the 2nd has it under «antes:» but not under «después:».
  - `write_measures_appends_and_ties`: `Escribir` two bars from the last + 1 with `~` between bars → there are 2 more bars and a `Connection` of type `Tie` that crosses them.
  - `write_rejects_overfull_without_applying`: the 2nd of three bars adds up to 4½ beats → `Err` that mentions the bar and «4½» (or the excess duration), and the score is left as it was before.
  - `adjust_connection_slur_beats_1_to_3`: `Ajustar conexion ligadura` from beat 1 to 3 of bar 4 → `Connection{kind: Slur}` between those events.
  - `summary_lists_parts_and_selection`: with `examples/cuarteto-en-sol.partitura` loaded with `score_io::decode`, `Resumen` contains the five part names, numbered, and the number of bars.
  - `read_caps_range`: `Leer` from 1 to 40 → `Err` asking for a range of 32 bars or fewer.
- [ ] **Step 2: Run** `cargo test -p agent-tools`. Expected: FAIL.
- [ ] **Step 3: Implement** `catalog.rs`, `tools.rs` and `instructions.rs`.
- [ ] **Step 4: Run** `cargo test -p agent-tools`. Expected: PASS.
- [ ] **Step 5: Commit** `git commit -m "Agent tools over the editor"`

### Task 5: `agent-host`, internal MCP server

**Files:**
- Create: `crates/agent-host/Cargo.toml` (`agent-tools`, `rmcp`, `tokio` with `rt-multi-thread` and `net`, `futures`, `axum` or `hyper-util` with a pinned version to serve `StreamableHttpService`), `src/lib.rs`, `src/runtime.rs`, `src/mcp.rs`, `tests/mcp_server.rs`
- Modify: root `Cargo.toml`

**Interfaces:**
- Consumes: `agent_tools::{ToolCall, tool_descriptions}`.
- Produces:
  - `pub struct Runtime` with `Runtime::new() -> io::Result<Self>` (multi-threaded tokio, 2 threads, named `levare-agent`), `handle()`, and `shutdown_background` when dropped.
  - `pub struct ToolInvocation { pub call: ToolCall, pub reply: futures::channel::oneshot::Sender<Result<String, String>> }`
  - `pub struct McpServer { pub url: String, pub token: String, /* task */ }` with `pub async fn start(tools: futures::channel::mpsc::UnboundedSender<ToolInvocation>) -> io::Result<McpServer>`: listens on `127.0.0.1:0`, requires `Authorization: Bearer <token>` (32 random bytes in hex) and keeps `rmcp`'s `Host` check. Each MCP tool deserializes its arguments into the matching `ToolCall`, sends it and waits for the reply; an `Err` is returned as a tool result with `isError: true`.

- [ ] **Step 1: Write the failing test** `tests/mcp_server.rs` (with `#[tokio::test]` and `rmcp`'s HTTP client in dev-dependencies):
  - `lists_levare_tools`: `tools/list` returns exactly the names from `tool_descriptions()`.
  - `call_round_trips_through_channel`: a fake thread answers `Ok("hecho")` to the invocation; `tools/call resumen` → text «hecho».
  - `tool_error_is_flagged`: if the fake thread answers `Err("x")` → `isError` true and text «x».
  - `rejects_missing_token`: a request without the header → 401.
- [ ] **Step 2: Run** `cargo test -p agent-host --test mcp_server`. Expected: FAIL.
- [ ] **Step 3: Implement** `runtime.rs` and `mcp.rs`.
- [ ] **Step 4: Run** `cargo test -p agent-host`. Expected: PASS.
- [ ] **Step 5: Commit** `git commit -m "Internal MCP server with Levare's tools"`

### Task 6: `agent-host`, agents and ACP session

**Files:**
- Create: `crates/agent-host/src/agents.rs`, `src/session.rs`, `src/agents_tests.rs`, `src/session_tests.rs`
- Modify: `crates/agent-host/Cargo.toml` (`agent-client-protocol` with `process`)

**Interfaces:**
- Consumes: `McpServer` (Task 5).
- Produces:
  - `pub enum AgentKind { Claude, Codex, Gemini }` with `ALL` and `label()` → «Claude Code», «Codex», «Gemini CLI».
  - `pub struct ResolvedTools { pub path_env: String, pub npx: Option<PathBuf>, pub claude: Option<PathBuf>, pub codex: Option<PathBuf>, pub gemini: Option<PathBuf> }`
  - `pub fn parse_probe(output: &str) -> ResolvedTools`: a pure function over the probe's output.
  - `pub fn probe() -> ResolvedTools`: runs `$SHELL` (or `/bin/zsh`) with `-lc` and a probe that prints `PATH=` and one `name=path` line for each `command -v`.
  - `pub struct LaunchCommand { pub program: PathBuf, pub args: Vec<String>, pub env: Vec<(String, String)> }`
  - `pub fn launch_command(kind: AgentKind, tools: &ResolvedTools) -> Result<LaunchCommand, String>`, which always passes `PATH = path_env`:
    - **Claude:** `npx -y @agentclientprotocol/claude-agent-acp@0.94.0`; requires `npx` and `claude`.
    - **Codex:** `npx -y @agentclientprotocol/codex-acp@2.2.2`; requires `npx` and `codex`, and passes `CODEX_PATH`.
    - **Gemini:** `gemini --acp`; requires `gemini`.
    - If something is missing, `Err` with what is missing («No se encontró `npx`: instala Node.js»).
  - `pub enum AgentEvent { Message(String), Tool { id: String, title: String, done: bool }, Permission { request: u64, title: String, options: Vec<(String, String)> }, TurnEnded, Failed(String) }`
  - `pub struct SessionHandle` with `prompt(&self, text: String)`, `cancel(&self)` and `answer_permission(&self, request: u64, option_id: Option<String>)`.
  - `pub fn start_session(rt: &Runtime, launch: LaunchCommand, cwd: PathBuf, mcp: &McpServer) -> (SessionHandle, futures::channel::mpsc::UnboundedReceiver<AgentEvent>)`. On the runtime:
    - launches `AcpAgent`, does `initialize` and checks `mcpCapabilities.http`;
    - opens `session/new` with `McpServer::Http(url)` and the authorization header;
    - converts each `SessionUpdate` (`AgentMessageChunk` → `Message`, `ToolCall`/`ToolCallUpdate` → `Tool`; thoughts are discarded) and each `request_permission` (→ `Permission`) into events;
    - a `prompt` in progress ends with `TurnEnded`; any error or the process closing produces `Failed`.
  - `pub fn explain_error(kind: AgentKind, raw: &str) -> String`: a pure function. For authentication errors (`auth_required` or an equivalent code or message) it returns the login text: «Inicia sesión una vez en la terminal con `claude`», «`codex login`» or «`gemini`». If the agent does not announce HTTP: «Este agente no admite las herramientas de Levare». If the `npx` download fails: «Sin conexión para descargar el adaptador».

- [ ] **Step 1: Write the failing tests**:
  - `parse_probe_reads_paths`: output with `PATH=/opt/homebrew/bin:/usr/bin` and `npx=/opt/homebrew/bin/npx` → the right fields; absent tools, `None`.
  - `launch_requires_tools`: without `npx`, Claude → `Err` that mentions `npx`; with everything, Gemini → program `gemini`, args `["--acp"]` and `PATH` in `env`.
  - `explain_auth_error_per_agent`: an authentication error → for each agent, the text with its login command.
- [ ] **Step 2: Run** `cargo test -p agent-host`. Expected: FAIL.
- [ ] **Step 3: Implement** `agents.rs` and `session.rs` (as a reference, the ACP crate's `yolo_one_shot_client.rs` example; do not use the `v2_*` or `unstable_*` APIs).
- [ ] **Step 4: Run** `cargo test -p agent-host`. Expected: PASS.
- [ ] **Step 5: Commit** `git commit -m "ACP sessions with Claude Code, Codex and Gemini CLI"`

### Task 7: Agent turn in `app-gpui`: lock, bridge and undo

**Files:**
- Create: `crates/app-gpui/src/agent.rs`, `crates/app-gpui/src/agent_tests.rs`
- Modify:
  - `crates/app-gpui/src/main.rs`: `AppView` fields; checks in `run` (~1304), `try_edit` (~776), `history` (~2625), paste, `execute_joined` (~595) and window close (~195).
  - `crates/app-gpui/src/selection.rs` and `staff_text.rs`: mouse and selection inputs.
  - `crates/app-gpui/Cargo.toml`: `agent-tools`, `agent-host`.

**Interfaces:**
- Consumes: Tasks 1 and 4 to 6.
- Produces:
  - `pub fn user_may_run(category: Category, turn_active: bool) -> bool`: during the turn only Reproducción and Vista, and always the panel's own commands.
  - `pub struct Turn { mark: HistoryMark }` with `Turn::start(editor: &mut Editor) -> Turn` and `Turn::finish(self, editor: &mut Editor)` (`squash` and then `restore_selection`).
  - `pub struct AgentState`: chosen agent, `Runtime`, `McpServer`, `SessionHandle`, the tools receiver, `Option<Turn>`, the conversation log and the availabilities from `probe()`. It is created lazily with the first message.
  - `AppView::agent_locked(&self) -> bool` and the flag `agent_acting: bool`, active only while `Host::run` calls `AppView::run`.
  - `impl agent_tools::Host` for a temporary adapter over `&mut AppView` + `&mut Window` + `&mut Context<AppView>`. `run` rejects the ids that `allowed_commands` does not contain.

Flow:
- **When sending a message:**
  - takes focus away from any text field;
  - `Turn::start`;
  - sends the `prompt`, with `INSTRUCTIONS` in front in the first message;
  - opens a `cx.spawn_in(window, …)` loop that serves `ToolInvocation` (calls `agent_tools::dispatch`, then `cx.notify()`, and replies through the `oneshot`) and `AgentEvent`.
- **On receiving `TurnEnded` or `Failed`, or when pressing Detener (Stop) (`cancel`):** `Turn::finish` and unlock the score.
- **When opening or creating another document:** drop the session.
- **Session working directory:** `app_paths::data_path("agente")`, created if it does not exist.

- [ ] **Step 1: Write the failing tests** in `agent_tests.rs`:
  - `lock_allows_playback_and_view_only`: with `turn_active`, `user_may_run(Category::Playback)` and `(Category::View)` → `true`; Edición, Entrada de notas and Expresión → `false`; without a turn, everything → `true`.
  - `turn_finish_after_failure_leaves_single_undo`: `Turn::start`, two edits and a `dispatch(Escribir)` that fails and rolls back, `finish` → a single `undo()` returns the initial score and the selection is the one from before the turn.
  - `turn_without_edits_adds_no_undo`: `start` + `finish` → `can_undo()` unchanged.
- [ ] **Step 2: Run** `cargo test -p levare agent_tests`. Expected: FAIL.
- [ ] **Step 3: Implement** `agent.rs` and the `agent_locked()` checks next to each user-input `self.busy` (except when `agent_acting` is active).
- [ ] **Step 4: Run** `cargo test --workspace --locked`. Expected: PASS.
- [ ] **Step 5: Commit** `git commit -m "Agent turn: score lock and a single undo"`

### Task 8: Agent panel in the interface

**Files:**
- Create: `crates/app-gpui/src/ui/agent_panel.rs`
- Modify:
  - `crates/keymap/src/commands.rs`: id `TOGGLE_AGENT_PANEL`, category Vista, label «Panel de agente», no shortcut.
  - `crates/app-gpui/src/ui.rs`: `UiState.show_agent`, `AGENT_WIDTH = 320.`, a column to the right of the inspector in `workspace()` (~300) and a button in `titlebar` (~411).
  - `crates/app-gpui/src/main.rs`: branch in `run`, score width in `render` (~3184) and the field's Enter/Escape in `intercept` (~901).
  - `crates/app-gpui/src/ui/command_state.rs`: checked state of the toggle.
  - `crates/app-gpui/src/ui/icons.rs`: panel icon.
  - `README.md`: section «Panel de agente».
  - `docs/shortcuts.md`: regenerated.
- Test: `crates/app-gpui/src/agent_tests.rs` (conversation log)

**Interfaces:**
- Consumes: `AgentState` and `AgentEvent` (Tasks 6 and 7).
- Produces:
  - `pub enum Entry { User(String), Agent(String), Tool { id: String, title: String, done: bool }, Permission { request: u64, title: String, options: Vec<(String, String)> }, Error(String) }`
  - `pub struct Log(Vec<Entry>)` with `apply(&mut self, kind: AgentKind, event: AgentEvent)`, which does this:
    - consecutive `Message`s are merged into the last `Agent` entry;
    - a `Tool` with an id already seen updates that entry;
    - a `Failed` adds an `Error` with `explain_error`.

Panel contents, from top to bottom:
- **Agent selector:** buttons for the three; unavailable ones are disabled, with the reason in the tooltip.
- **Conversation:** scrollable, in plain text.
- **Permissions:** a card with one button per option, plus «Rechazar» (Reject).
- **Status during the turn:** «Trabajando… La partitura está bloqueada. Pulsa Detener para editar.»
- **At the bottom:** `TextInput` (`with_max_chars(2000)`) and the Enviar (Send) button, which becomes Detener (Stop) during the turn.
- **Keyboard:** Enter sends; Escape takes focus away.

- [ ] **Step 1: Write the failing tests**:
  - `log_merges_message_chunks`: `Message("Hola ")` + `Message("mundo")` → one entry `Agent("Hola mundo")`.
  - `log_updates_tool_by_id`: `Tool{id:"1", done:false}` and then `Tool{id:"1", done:true}` → a single `done` entry.
  - `log_explains_failure`: a `Failed` with an authentication error → `Error` with the login command.
  - The test that already exists for `docs/shortcuts.md` fails until it is regenerated.
- [ ] **Step 2: Run** `cargo test -p levare agent_tests`. Expected: FAIL.
- [ ] **Step 3: Implement** `Log`, the panel, the toggle and the README section; regenerate `docs/shortcuts.md` with the registry's generator (`crates/keymap/src/docs.rs`).
- [ ] **Step 4: Run** `cargo fmt --all --check && cargo test --workspace --locked && cargo clippy --workspace --all-targets --locked -- -D warnings`. Expected: all PASS.
- [ ] **Step 5: Commit** `git commit -m "Agent panel in the interface"`

### Task 9: End-to-end verification with the three agents

**Files:** none, except the fixes that come out of it.

- [ ] **Step 1:** `cargo run`, open `examples/cuarteto-en-sol.partitura` and open the panel. With **Claude Code** ask: «Escribe una ligadura en el 1.er y 3.er tiempo del compás 4 del violín, pon un trino en el siguiente tiempo y agrega un número de ensayo» (Write a slur on the 1st and 3rd beat of bar 4 of the violin, put a trill on the next beat and add a rehearsal number). Expected: a slur from beat 1 to 3, a trill on 4 and a rehearsal number, all visible live.
- [ ] **Step 2:** While it works, try writing a note and pressing ⌘Z (they do nothing) and playing back (it works). When it finishes, a single ⌘Z undoes everything and the selection is the one from before.
- [ ] **Step 3:** Ask for «violines» on a score with Violín I and II. Expected: the agent asks which one.
- [ ] **Step 4:** Press Detener (Stop) halfway through a long request («escribe una escala de do en negras en los compases 1 a 8 de la viola», i.e. write a C scale in quarter notes in bars 1 to 8 of the viola). Expected: what was done is kept, it is undone in one go and the score is left unlocked.
- [ ] **Step 5:** Repeat steps 1 and 2 with **Codex** and with **Gemini CLI**.
- [ ] **Step 6:** `python3 scripts/bundle_macos.py` and open `target/Levare.app` from Finder. Expected: the three agents show up as available (or with a clear reason) and step 1 works.
- [ ] **Step 7:** Note in the README any known problems of an agent and commit the fixes with `git commit -m "Agent panel: fixes after trying the three agents"`.

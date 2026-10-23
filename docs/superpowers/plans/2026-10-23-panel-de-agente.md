# Panel de agente: plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Un panel lateral de conversación en Levare con un agente (Claude Code, Codex o Gemini CLI vía ACP) que lee y edita la partitura abierta con las mismas acciones del editor.

**Architecture:** Tres crates nuevos sin GPUI: `score-text` (notación de texto compacta y posiciones musicales), `agent-tools` (herramientas sobre el `Editor`, independientes del transporte) y `agent-host` (servidor MCP HTTP en 127.0.0.1 con `rmcp` y cliente ACP con `agent-client-protocol`, sobre un runtime tokio propio). `app-gpui` aporta el panel, el bloqueo de la partitura durante el turno y el puente que ejecuta en el hilo de la interfaz cada llamada de herramienta que llega por canal.

**Tech Stack:** Rust 2024, GPUI `=0.2.2`, `agent-client-protocol =3.5.0` (feature `process`), `rmcp =3.7.0` (features `server`, `macros`, `transport-streamable-http-server`), `tokio =1.53.3` (ya en `Cargo.lock`), `schemars` en la versión que exija `rmcp 3.7.0`, `futures`.

**Spec:** [docs/superpowers/specs/2026-10-23-panel-de-agente-design.md](../specs/2026-10-23-panel-de-agente-design.md)

## Global Constraints

- Dependencias con versión exacta (`=x.y.z`) declaradas en `[workspace.dependencies]` del `Cargo.toml` raíz y usadas con `x.workspace = true`.
- GPUI solo en `app-gpui`; `score-text`, `agent-tools` y `agent-host` no dependen de GPUI (`docs/desarrollo.md:32`).
- Deben pasar `cargo fmt --all --check`, `cargo test --workspace --locked` y `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Textos de interfaz y mensajes al agente en español; identificadores Rust en inglés como el resto del código; vocabulario de `CONTEXT.md` (parte, altura escrita, altura real).
- El agente nunca ejecuta comandos de las categorías Archivo ni Vista, ni deshacer/rehacer, ni comandos que abren diálogos o campos de texto.
- Todo lo que el agente hace en un turno se deshace con un solo ⌘Z; mientras dura el turno el usuario no puede editar, deshacer ni rehacer, pero sí reproducir, desplazarse y hacer zoom.
- Al terminar o detener el turno, cursor y selección vuelven a donde los dejó el usuario.
- Con 32 partes la interfaz sigue fluida: el hilo de la interfaz solo aplica ediciones; red, procesos y E/S van fuera de él.
- Lo nuevo con IDs de comando sigue la regla de `docs/desarrollo.md`: constante en `id::`, entrada en el registro, rama en el `match` de `AppView::run` y `docs/atajos.md` regenerado.

## Review Focus

1. **Nombre de parte ambiguo** («violines» con Violín I y Violín II): la herramienta devuelve un error que enumera las candidatas y nunca elige una. Test en la Tarea 3.
2. **Turno que acaba mal** (el agente muere, el usuario pulsa Detener a mitad de `escribir`, error ACP): queda una sola entrada de deshacer o ninguna, la selección se restaura y la partitura se desbloquea. Test en la Tarea 7.
3. **Agente sin sesión iniciada o sin red para `npx`**: el panel muestra un error en español con la orden para iniciar sesión, y no queda nada bloqueado. Test en la Tarea 6.
4. **App abierta desde Finder** (sin el `PATH` de la terminal): `npx`, `node`, `claude`, `codex` y `gemini` se buscan con la shell de inicio de sesión, y un agente al que le falta algo aparece como no disponible con el motivo. Test en la Tarea 6.
5. **Interruptor sobre una marca existente** (pedir «trino» donde ya lo hay): `ejecutar` devuelve el antes y el después del compás para que el agente vea que lo quitó. Test en la Tarea 4.

---

### Task 1: Historial agrupado en `score-edit`

**Files:**
- Modify: `crates/score-edit/src/lib.rs` (junto a `execute_joined`, ~1341)
- Create: `crates/score-edit/src/group_tests.rs` (declarado con `#[cfg(test)] mod group_tests;`)

**Interfaces:**
- Produces:
  - `pub struct HistoryMark` (opaca: guarda `undo.len()` y el `Snapshot` de ese momento).
  - `Editor::mark(&self) -> HistoryMark`
  - `Editor::rollback(&mut self, mark: &HistoryMark)`: vuelve al estado del mark, trunca `undo` a su longitud y vacía `redo`.
  - `Editor::squash(&mut self, mark: &HistoryMark) -> bool`: deja como mucho una entrada de deshacer después del mark (la instantánea del propio mark) y devuelve si hubo cambios. Si `undo.len() <= mark` no hace nada y devuelve `false`.
  - `Editor::restore_selection(&mut self, mark: &HistoryMark)`: copia cursor, selección, `range_anchor`, `divisi_anchor` y `tone` del mark al estado actual sin crear entrada de deshacer.

- [ ] **Step 1: Write the failing tests** en `group_tests.rs`:
  - `squash_turns_many_edits_into_one_undo`: tres `execute(Command::Write(..))` tras `mark()`; `squash` → `true`; un solo `undo()` devuelve la partitura igual a la del mark y `can_undo()` vuelve al valor anterior al mark.
  - `squash_without_changes_adds_nothing`: `mark()` + `squash` → `false`; `can_undo()` sin cambios.
  - `rollback_discards_partial_edits`: dos ediciones, `rollback` → `score()` igual al del mark y `can_redo() == false`.
  - `restore_selection_keeps_score_and_history`: cambiar selección y partitura, `squash`, `restore_selection` → la selección es la del mark, la partitura conserva los cambios y un `undo()` sigue llevando al estado anterior.
- [ ] **Step 2: Run** `cargo test -p score-edit group_tests`. Expected: FAIL (métodos inexistentes).
- [ ] **Step 3: Implement** los cuatro métodos y `HistoryMark` en `lib.rs`.
- [ ] **Step 4: Run** `cargo test -p score-edit`. Expected: PASS.
- [ ] **Step 5: Commit** `git commit -m "Historial: agrupar y revertir ediciones desde una marca"`

### Task 2: Crate `score-text`, notación compacta

**Files:**
- Create: `crates/score-text/Cargo.toml` (depende de `score-core`), `crates/score-text/src/lib.rs`, `crates/score-text/src/notation.rs`, `crates/score-text/src/notation_tests.rs`
- Modify: `Cargo.toml` raíz (`score-text = { path = "crates/score-text" }` en `[workspace.dependencies]`)

**Interfaces:**
- Produces:
  - `pub struct WrittenEvent { pub event: score_core::Event, pub tie_to_next: bool }`
  - `pub struct TextError { pub offset: usize, pub message: String }` (mensaje en español; `impl Display`)
  - `pub fn parse_voice(text: &str) -> Result<Vec<WrittenEvent>, TextError>`
  - `pub fn format_voice(score: &Score, part: PartId, staff: StaffId, measure: usize, voice: Voice) -> String`

Gramática (alturas escritas, octava científica: `C4` = do central, como `score_playback::midi_key`):

```
voz      := evento (" " evento)*
evento   := (altura | acorde | "R") ":" duracion ["~"] [marcas]
altura   := [A-G] ["#" | "b" | "n"] [0-8]
acorde   := "[" altura (" " altura)+ "]"
duracion := ("1"|"2"|"4"|"8"|"16"|"32") ["."] ["t"]      // "t" = tresillo
marcas   := "{" texto "}"                                 // solo informativo
```

`format_voice` escribe las marcas del evento entre llaves con las etiquetas en español que ya existen en `score-core` (articulación, dinámica, técnica, ornamento, inicio y fin de ligaduras y reguladores…), separadas por comas. `parse_voice` acepta y descarta los bloques `{…}`, así el agente puede reenviar lo que leyó.

- [ ] **Step 1: Write the failing tests** en `notation_tests.rs`:
  - `parses_notes_rests_chords`: `"C4:4 R:8 Eb5:8 [C4 E4 G4]:2"` → 4 eventos; el tercero tiene `Accidental::Flat` y octava 5; el cuarto, 3 alturas y duración blanca.
  - `parses_dots_triplets_ties`: `"D4:4.~ D4:8 E4:8t"` → puntillo en el 1.º, `tie_to_next` en el 1.º, tresillo en el 3.º.
  - `ignores_mark_blocks`: `"C5:4{Trino, f} D5:4"` → 2 eventos.
  - `rejects_bad_tokens`: `"H4:4"`, `"C4:3"`, `"C9:4"` y `"C4"` → `Err` con `offset` en el token y un mensaje que lo nombra.
  - `format_round_trips`: escribir con `Editor` un compás 4/4 con nota con puntillo, acorde, silencio y una dinámica; `parse_voice(format_voice(..))` da los mismos `Event`.
  - `formats_written_pitch_for_transposing_parts`: clarinete en Si♭ con do real escrito → sale `D` (altura escrita).
- [ ] **Step 2: Run** `cargo test -p score-text`. Expected: FAIL.
- [ ] **Step 3: Implement** `notation.rs` (analizador manual, sin dependencias nuevas).
- [ ] **Step 4: Run** `cargo test -p score-text`. Expected: PASS.
- [ ] **Step 5: Commit** `git commit -m "Notación de texto compacta para leer y escribir compases"`

### Task 3: Posiciones musicales en `score-text`

**Files:**
- Create: `crates/score-text/src/position.rs`, `crates/score-text/src/position_tests.rs`

**Interfaces:**
- Consumes: `Score::part_name(id)` (`score-core/src/instrument.rs:196`), `Score::time_at`, `Measure::timed_iter`.
- Produces:
  - `pub fn resolve_part(score: &Score, query: &str) -> Result<PartId, String>`: número 1-based o nombre; sin distinguir mayúsculas ni acentos; acepta prefijo y plural simple («violas» → «Viola»). Si coinciden varias partes, `Err` con todas las candidatas numeradas.
  - `pub fn bar_index(score: &Score, bar: u32) -> Result<usize, String>`: número de compás tal como lo muestra Levare (mira cómo numera `score-layout` y respeta la anacrusa) → índice 0-based.
  - `pub fn beat_tick(score: &Score, measure: usize, beat: f32) -> Result<u16, String>`: tiempo 1-based en unidades del denominador (en 6/8 cuenta corcheas); `tick = (beat − 1) · 192 / den`. Error si no es entero o si cae fuera de la duración del compás.
  - `pub fn event_at(score: &Score, part: PartId, staff: StaffId, voice: Voice, measure: usize, tick: u16) -> Result<ElementId, String>`: si no empieza ningún evento en ese tick, el error enumera los tiempos donde sí empiezan («empiezan en 1, 2, 3, 4»).

- [ ] **Step 1: Write the failing tests**:
  - `resolves_part_by_number_and_name`: «2», «viola», «VIOLAS», «violín» → la parte correcta.
  - `ambiguous_part_lists_candidates`: con Violín I y Violín II, `"violín"` → `Err` que contiene los dos nombres y sus números.
  - `beat_ticks_in_simple_and_compound`: 4/4 tiempo 3 → 96; 6/8 tiempo 4 → 72; 4/4 tiempo 2.5 → 72.
  - `beat_outside_measure_fails`: 3/4 tiempo 4 → `Err`.
  - `event_at_reports_onsets`: compás de cuatro negras, tick 24 → `Err` que contiene «1, 2, 3, 4».
  - `pickup_bar_numbering`: partitura con primer compás más corto (como la que crea la importación MusicXML) → `bar_index` coincide con la numeración que muestra Levare.
- [ ] **Step 2: Run** `cargo test -p score-text position`. Expected: FAIL.
- [ ] **Step 3: Implement** `position.rs`.
- [ ] **Step 4: Run** `cargo test -p score-text`. Expected: PASS.
- [ ] **Step 5: Commit** `git commit -m "Posiciones musicales: parte, compás y tiempo"`

### Task 4: Crate `agent-tools`

**Files:**
- Create: `crates/agent-tools/Cargo.toml` (depende de `score-core`, `score-edit`, `score-text`, `score-keymap`, `serde`, `serde_json` y `schemars`; `score-toolbar` y `score-io` solo en dev-dependencies), `src/lib.rs`, `src/catalog.rs`, `src/tools.rs`, `src/instructions.rs`, `src/tools_tests.rs`, `src/catalog_tests.rs`
- Modify: `Cargo.toml` raíz

**Interfaces:**
- Consumes: Tareas 1 a 3.
- Produces:
  - `pub trait Host { fn editor(&self) -> &Editor; fn editor_mut(&mut self) -> &mut Editor; fn run(&mut self, id: CommandId) -> Result<(), String>; }`
  - `pub fn allowed_commands(registry: &Registry) -> Vec<&CommandSpec>`: categorías Edición, Entrada de notas, Duración, Alteraciones, Expresión, Notación, Compases, Instrumentos, Navegación, Revisión y Reproducción; estado `Ready`; menos `EXCLUDED`.
  - `pub const EXCLUDED: &[CommandId]`: deshacer, rehacer y todo comando que abre interfaz (búsqueda de comandos, información de la partitura, ir al compás, pantalla completa, editores de texto, letra y cifrado, selectores de instrumento y conmutadores de paneles; ver `main.rs` 1366, 1428, 1573, 1583, 1682 y 1722).
  - `#[derive(Deserialize, JsonSchema)] pub struct Position { parte: String, compas: u32, #[serde(default = "first_beat")] tiempo: f32, pentagrama: Option<u8>, voz: Option<u8> }` (`fn first_beat() -> f32 { 1.0 }`)
  - `pub enum ToolCall`, con una struct de argumentos por variante (`LeerArgs`, `SeleccionarArgs`, …; `AjustarArgs` es un enum con `#[serde(tag = "tipo", rename_all = "snake_case")]`). Cada struct es `Deserialize + JsonSchema`, y su esquema es el que anuncia el MCP para esa herramienta. Las variantes son:

    | Variante | Argumentos | Qué hace |
    |---|---|---|
    | `Resumen` | — | Título, partes numeradas con pentagramas, número de compases, compás de tiempo, tonalidad y tempo iniciales y sus cambios, y selección actual. |
    | `Leer` | `partes: Option<Vec<String>>, desde: u32, hasta: u32` | Máximo 32 compases por llamada. Una línea por parte, pentagrama, voz y compás con `format_voice`, y antes, los cambios de compás de tiempo, tonalidad, tempo y números de ensayo. |
    | `Seleccionar` | `desde: Position, hasta: Option<Position>` | Selecciona el evento o el rango y devuelve el compás formateado con `»` delante del evento seleccionado. |
    | `Comandos` | `categoria: Option<String>` | Lista `id — etiqueta` de `allowed_commands`, agrupada por categoría. |
    | `Ejecutar` | `comando: String` | Id o etiqueta exacta; ejecuta `Host::run` y devuelve «sin cambios», o el compás de la selección «antes:» y «después:». |
    | `Escribir` | `parte, pentagrama?, voz?, desde_compas: u32, compases: Vec<String>` | Cada cadena es un compás completo en la notación de la Tarea 2. Valida que cada compás llene exactamente su duración. Si `desde_compas` es el siguiente al último, añade compases (`AppendMeasure`); más allá, error. Escribe con `WriteAt` y une las ligaduras `~` con `AddConnection(Tie)`, también entre compases. Todo dentro de `mark`/`rollback`/`squash`: o se aplica entero o nada, y el error dice qué compás falló y por qué. |
    | `Ajustar` | `#[serde(tag = "tipo")]` con las subvariantes de abajo | Ajustes con parámetros. |

    Subvariantes de `Ajustar`:
    - `compas_tiempo{compas, numerador, denominador}`
    - `tonalidad{compas, quintas: i8, menor: bool}` (con `KeySignature::new`)
    - `tempo{compas, bpm, texto?}`
    - `clave{parte, pentagrama, compas, clave}`
    - `anadir_partes{instrumentos: Vec<String>}` (busca por `Instrument::name()` sin acentos; si no reconoce un nombre, error con los parecidos)
    - `quitar_parte{parte}`
    - `conexion{tipo: ligadura|union|crescendo|diminuendo|octava_alta|octava_baja|pedal, desde: Position, hasta: Position}`
    - `texto{posicion, clase: expresion|tecnica, texto}`
    - `titulo{titulo}`
  - `pub fn dispatch(host: &mut dyn Host, registry: &Registry, call: ToolCall) -> Result<String, String>`
  - `pub fn tool_descriptions() -> Vec<(&'static str, &'static str)>`: nombre y descripción en español de cada herramienta, incluida la gramática y la convención de alturas escritas, para que el MCP las anuncie.
  - `pub const INSTRUCTIONS: &str`, que se antepone al primer mensaje de la sesión y dice:
    - usar solo las herramientas de Levare y empezar por `resumen`;
    - leer antes de usar un interruptor;
    - preguntar si la parte es ambigua;
    - las alturas son escritas;
    - no tocar archivos;
    - responder breve y en el idioma del usuario.

- [ ] **Step 1: Write the failing tests** con un `FakeHost` (Editor real; `run` traduce los `MarkKind` con `score_toolbar::marks::kind` a `Command::ToggleMark` y devuelve `Err` para lo demás):
  - `catalog_excludes_file_view_undo_and_dialogs`: ningún id de `allowed_commands` es de Archivo o Vista, ni deshacer, ni de `EXCLUDED`; cada id de `EXCLUDED` existe en `Registry::builtin()`.
  - `select_then_toggle_reports_before_after`: `Seleccionar` el tiempo 2 del compás 1 y `Ejecutar` el trino dos veces; la 1.ª respuesta tiene el trino en «después:» y la 2.ª en «antes:» pero no en «después:».
  - `write_measures_appends_and_ties`: `Escribir` dos compases desde el último + 1 con `~` entre compases → hay 2 compases más y una `Connection` de tipo `Tie` que los cruza.
  - `write_rejects_overfull_without_applying`: el 2.º de tres compases suma 4½ tiempos → `Err` que menciona el compás y «4½» (o la duración sobrante), y la partitura queda igual que antes.
  - `adjust_connection_slur_beats_1_to_3`: `Ajustar conexion ligadura` del tiempo 1 al 3 del compás 4 → `Connection{kind: Slur}` entre esos eventos.
  - `summary_lists_parts_and_selection`: con `examples/cuarteto-en-sol.partitura` cargado con `score_io::decode`, `Resumen` contiene los cinco nombres de parte numerados y el número de compases.
  - `read_caps_range`: `Leer` de 1 a 40 → `Err` que pide un rango de 32 compases o menos.
- [ ] **Step 2: Run** `cargo test -p agent-tools`. Expected: FAIL.
- [ ] **Step 3: Implement** `catalog.rs`, `tools.rs` e `instructions.rs`.
- [ ] **Step 4: Run** `cargo test -p agent-tools`. Expected: PASS.
- [ ] **Step 5: Commit** `git commit -m "Herramientas del agente sobre el editor"`

### Task 5: `agent-host`, servidor MCP interno

**Files:**
- Create: `crates/agent-host/Cargo.toml` (`agent-tools`, `rmcp`, `tokio` con `rt-multi-thread` y `net`, `futures`, `axum` o `hyper-util` con versión fijada para servir `StreamableHttpService`), `src/lib.rs`, `src/runtime.rs`, `src/mcp.rs`, `tests/mcp_server.rs`
- Modify: `Cargo.toml` raíz

**Interfaces:**
- Consumes: `agent_tools::{ToolCall, tool_descriptions}`.
- Produces:
  - `pub struct Runtime` con `Runtime::new() -> io::Result<Self>` (tokio multi-hilo, 2 hilos, nombre `levare-agent`), `handle()`, y al soltarse `shutdown_background`.
  - `pub struct ToolInvocation { pub call: ToolCall, pub reply: futures::channel::oneshot::Sender<Result<String, String>> }`
  - `pub struct McpServer { pub url: String, pub token: String, /* tarea */ }` con `pub async fn start(tools: futures::channel::mpsc::UnboundedSender<ToolInvocation>) -> io::Result<McpServer>`: escucha en `127.0.0.1:0`, exige `Authorization: Bearer <token>` (32 bytes aleatorios en hex) y conserva la comprobación de `Host` de `rmcp`. Cada herramienta del MCP deserializa sus argumentos en el `ToolCall` correspondiente, lo envía y espera la respuesta; un `Err` se devuelve como resultado de herramienta con `isError: true`.

- [ ] **Step 1: Write the failing test** `tests/mcp_server.rs` (con `#[tokio::test]` y el cliente HTTP de `rmcp` en dev-dependencies):
  - `lists_levare_tools`: `tools/list` devuelve exactamente los nombres de `tool_descriptions()`.
  - `call_round_trips_through_channel`: un hilo falso responde `Ok("hecho")` a la invocación; `tools/call resumen` → texto «hecho».
  - `tool_error_is_flagged`: si el hilo falso responde `Err("x")` → `isError` verdadero y texto «x».
  - `rejects_missing_token`: una petición sin cabecera → 401.
- [ ] **Step 2: Run** `cargo test -p agent-host --test mcp_server`. Expected: FAIL.
- [ ] **Step 3: Implement** `runtime.rs` y `mcp.rs`.
- [ ] **Step 4: Run** `cargo test -p agent-host`. Expected: PASS.
- [ ] **Step 5: Commit** `git commit -m "Servidor MCP interno con las herramientas de Levare"`

### Task 6: `agent-host`, agentes y sesión ACP

**Files:**
- Create: `crates/agent-host/src/agents.rs`, `src/session.rs`, `src/agents_tests.rs`, `src/session_tests.rs`
- Modify: `crates/agent-host/Cargo.toml` (`agent-client-protocol` con `process`)

**Interfaces:**
- Consumes: `McpServer` (Tarea 5).
- Produces:
  - `pub enum AgentKind { Claude, Codex, Gemini }` con `ALL` y `label()` → «Claude Code», «Codex», «Gemini CLI».
  - `pub struct ResolvedTools { pub path_env: String, pub npx: Option<PathBuf>, pub claude: Option<PathBuf>, pub codex: Option<PathBuf>, pub gemini: Option<PathBuf> }`
  - `pub fn parse_probe(output: &str) -> ResolvedTools`: función pura sobre la salida de la sonda.
  - `pub fn probe() -> ResolvedTools`: ejecuta `$SHELL` (o `/bin/zsh`) con `-lc` y una sonda que imprime `PATH=` y una línea `nombre=ruta` por cada `command -v`.
  - `pub struct LaunchCommand { pub program: PathBuf, pub args: Vec<String>, pub env: Vec<(String, String)> }`
  - `pub fn launch_command(kind: AgentKind, tools: &ResolvedTools) -> Result<LaunchCommand, String>`, que siempre pasa `PATH = path_env`:
    - **Claude:** `npx -y @agentclientprotocol/claude-agent-acp@0.94.0`; requiere `npx` y `claude`.
    - **Codex:** `npx -y @agentclientprotocol/codex-acp@2.2.2`; requiere `npx` y `codex`, y pasa `CODEX_PATH`.
    - **Gemini:** `gemini --acp`; requiere `gemini`.
    - Si falta algo, `Err` con lo que falta («No se encontró `npx`: instala Node.js»).
  - `pub enum AgentEvent { Message(String), Tool { id: String, title: String, done: bool }, Permission { request: u64, title: String, options: Vec<(String, String)> }, TurnEnded, Failed(String) }`
  - `pub struct SessionHandle` con `prompt(&self, text: String)`, `cancel(&self)` y `answer_permission(&self, request: u64, option_id: Option<String>)`.
  - `pub fn start_session(rt: &Runtime, launch: LaunchCommand, cwd: PathBuf, mcp: &McpServer) -> (SessionHandle, futures::channel::mpsc::UnboundedReceiver<AgentEvent>)`. En el runtime:
    - lanza `AcpAgent`, hace `initialize` y comprueba `mcpCapabilities.http`;
    - abre `session/new` con `McpServer::Http(url)` y la cabecera de autorización;
    - convierte cada `SessionUpdate` (`AgentMessageChunk` → `Message`, `ToolCall`/`ToolCallUpdate` → `Tool`; los pensamientos se descartan) y cada `request_permission` (→ `Permission`) en eventos;
    - un `prompt` en curso termina con `TurnEnded`; cualquier error o el cierre del proceso producen `Failed`.
  - `pub fn explain_error(kind: AgentKind, raw: &str) -> String`: función pura. Para errores de autenticación (`auth_required` o un código o mensaje equivalente) devuelve el texto de inicio de sesión: «Inicia sesión una vez en la terminal con `claude`», «`codex login`» o «`gemini`». Si el agente no anuncia HTTP: «Este agente no admite las herramientas de Levare». Si la descarga de `npx` falla: «Sin conexión para descargar el adaptador».

- [ ] **Step 1: Write the failing tests**:
  - `parse_probe_reads_paths`: salida con `PATH=/opt/homebrew/bin:/usr/bin` y `npx=/opt/homebrew/bin/npx` → los campos correctos; las herramientas ausentes, `None`.
  - `launch_requires_tools`: sin `npx`, Claude → `Err` que menciona `npx`; con todo, Gemini → programa `gemini`, args `["--acp"]` y `PATH` en `env`.
  - `explain_auth_error_per_agent`: un error de autenticación → para cada agente, el texto con su orden de inicio de sesión.
- [ ] **Step 2: Run** `cargo test -p agent-host`. Expected: FAIL.
- [ ] **Step 3: Implement** `agents.rs` y `session.rs` (como referencia, el ejemplo `yolo_one_shot_client.rs` del crate ACP; no uses las APIs `v2_*` ni las `unstable_*`).
- [ ] **Step 4: Run** `cargo test -p agent-host`. Expected: PASS.
- [ ] **Step 5: Commit** `git commit -m "Sesiones ACP con Claude Code, Codex y Gemini CLI"`

### Task 7: Turno del agente en `app-gpui`: bloqueo, puente y deshacer

**Files:**
- Create: `crates/app-gpui/src/agent.rs`, `crates/app-gpui/src/agent_tests.rs`
- Modify:
  - `crates/app-gpui/src/main.rs`: campos de `AppView`; comprobaciones en `run` (~1304), `try_edit` (~776), `history` (~2625), pegar, `execute_joined` (~595) y el cierre de ventana (~195).
  - `crates/app-gpui/src/selection.rs` y `staff_text.rs`: entradas de ratón y selección.
  - `crates/app-gpui/Cargo.toml`: `agent-tools`, `agent-host`.

**Interfaces:**
- Consumes: Tareas 1 y 4 a 6.
- Produces:
  - `pub fn user_may_run(category: Category, turn_active: bool) -> bool`: durante el turno solo Reproducción y Vista, y siempre los comandos del propio panel.
  - `pub struct Turn { mark: HistoryMark }` con `Turn::start(editor: &mut Editor) -> Turn` y `Turn::finish(self, editor: &mut Editor)` (`squash` y luego `restore_selection`).
  - `pub struct AgentState`: agente elegido, `Runtime`, `McpServer`, `SessionHandle`, el receptor de herramientas, `Option<Turn>`, el registro de conversación y las disponibilidades de `probe()`. Se crea perezosamente con el primer mensaje.
  - `AppView::agent_locked(&self) -> bool` y la bandera `agent_acting: bool`, activa solo mientras `Host::run` llama a `AppView::run`.
  - `impl agent_tools::Host` para un adaptador temporal sobre `&mut AppView` + `&mut Window` + `&mut Context<AppView>`. `run` rechaza los ids que `allowed_commands` no contiene.

Flujo:
- **Al enviar un mensaje:**
  - quita el foco de cualquier campo de texto;
  - `Turn::start`;
  - envía el `prompt`, con `INSTRUCTIONS` delante en el primer mensaje;
  - abre un bucle `cx.spawn_in(window, …)` que atiende `ToolInvocation` (llama a `agent_tools::dispatch`, luego `cx.notify()`, y responde por el `oneshot`) y `AgentEvent`.
- **Al recibir `TurnEnded` o `Failed`, o al pulsar Detener (`cancel`):** `Turn::finish` y desbloquea la partitura.
- **Al abrir o crear otro documento:** suelta la sesión.
- **Directorio de trabajo de la sesión:** `app_paths::data_path("agente")`, creado si no existe.

- [ ] **Step 1: Write the failing tests** en `agent_tests.rs`:
  - `lock_allows_playback_and_view_only`: con `turn_active`, `user_may_run(Category::Playback)` y `(Category::View)` → `true`; Edición, Entrada de notas y Expresión → `false`; sin turno, todo → `true`.
  - `turn_finish_after_failure_leaves_single_undo`: `Turn::start`, dos ediciones y un `dispatch(Escribir)` que falla y hace rollback, `finish` → un solo `undo()` devuelve la partitura inicial y la selección es la de antes del turno.
  - `turn_without_edits_adds_no_undo`: `start` + `finish` → `can_undo()` sin cambios.
- [ ] **Step 2: Run** `cargo test -p levare agent_tests`. Expected: FAIL.
- [ ] **Step 3: Implement** `agent.rs` y las comprobaciones `agent_locked()` junto a cada `self.busy` de entrada de usuario (salvo cuando `agent_acting` está activo).
- [ ] **Step 4: Run** `cargo test --workspace --locked`. Expected: PASS.
- [ ] **Step 5: Commit** `git commit -m "Turno del agente: bloqueo de la partitura y un solo deshacer"`

### Task 8: Panel de agente en la interfaz

**Files:**
- Create: `crates/app-gpui/src/ui/agent_panel.rs`
- Modify:
  - `crates/keymap/src/commands.rs`: id `TOGGLE_AGENT_PANEL`, categoría Vista, etiqueta «Panel de agente», sin atajo.
  - `crates/app-gpui/src/ui.rs`: `UiState.show_agent`, `AGENT_WIDTH = 320.`, columna a la derecha del inspector en `workspace()` (~300) y botón en `titlebar` (~411).
  - `crates/app-gpui/src/main.rs`: rama en `run`, ancho de la partitura en `render` (~3184) y Enter/Escape del campo en `intercept` (~901).
  - `crates/app-gpui/src/ui/command_state.rs`: estado marcado del conmutador.
  - `crates/app-gpui/src/ui/icons.rs`: icono del panel.
  - `README.md`: sección «Panel de agente».
  - `docs/atajos.md`: regenerado.
- Test: `crates/app-gpui/src/agent_tests.rs` (registro de conversación)

**Interfaces:**
- Consumes: `AgentState` y `AgentEvent` (Tareas 6 y 7).
- Produces:
  - `pub enum Entry { User(String), Agent(String), Tool { id: String, title: String, done: bool }, Permission { request: u64, title: String, options: Vec<(String, String)> }, Error(String) }`
  - `pub struct Log(Vec<Entry>)` con `apply(&mut self, kind: AgentKind, event: AgentEvent)`, que hace esto:
    - los `Message` seguidos se juntan en la última entrada `Agent`;
    - un `Tool` con un id ya visto actualiza esa entrada;
    - un `Failed` añade un `Error` con `explain_error`.

Contenido del panel, de arriba abajo:
- **Selector de agente:** botones con los tres; los no disponibles van desactivados, con el motivo en el tooltip.
- **Conversación:** desplazable, en texto plano.
- **Permisos:** tarjeta con un botón por opción, más «Rechazar».
- **Estado durante el turno:** «Trabajando… La partitura está bloqueada. Pulsa Detener para editar.»
- **Abajo:** `TextInput` (`with_max_chars(2000)`) y el botón Enviar, que es Detener durante el turno.
- **Teclado:** Enter envía; Escape quita el foco.

- [ ] **Step 1: Write the failing tests**:
  - `log_merges_message_chunks`: `Message("Hola ")` + `Message("mundo")` → una entrada `Agent("Hola mundo")`.
  - `log_updates_tool_by_id`: `Tool{id:"1", done:false}` y luego `Tool{id:"1", done:true}` → una sola entrada `done`.
  - `log_explains_failure`: un `Failed` con error de autenticación → `Error` con la orden de inicio de sesión.
  - El test que ya existe para `docs/atajos.md` falla hasta regenerarlo.
- [ ] **Step 2: Run** `cargo test -p levare agent_tests`. Expected: FAIL.
- [ ] **Step 3: Implement** `Log`, el panel, el conmutador y la sección del README; regenera `docs/atajos.md` con el generador del registro (`crates/keymap/src/docs.rs`).
- [ ] **Step 4: Run** `cargo fmt --all --check && cargo test --workspace --locked && cargo clippy --workspace --all-targets --locked -- -D warnings`. Expected: todo PASS.
- [ ] **Step 5: Commit** `git commit -m "Panel de agente en la interfaz"`

### Task 9: Verificación de punta a punta con los tres agentes

**Files:** ninguno, salvo los arreglos que salgan.

- [ ] **Step 1:** `cargo run`, abre `examples/cuarteto-en-sol.partitura` y abre el panel. Con **Claude Code** pide: «Escribe una ligadura en el 1.er y 3.er tiempo del compás 4 del violín, pon un trino en el siguiente tiempo y agrega un número de ensayo». Expected: ligadura del tiempo 1 al 3, trino en el 4 y número de ensayo, todo visible en directo.
- [ ] **Step 2:** Mientras trabaja, prueba a escribir una nota y a pulsar ⌘Z (no hacen nada) y a reproducir (funciona). Al terminar, un solo ⌘Z deshace todo y la selección es la de antes.
- [ ] **Step 3:** Pide «violines» en una partitura con Violín I y II. Expected: el agente pregunta cuál.
- [ ] **Step 4:** Pulsa Detener a mitad de una petición larga («escribe una escala de do en negras en los compases 1 a 8 de la viola»). Expected: lo hecho se conserva, se deshace de un golpe y la partitura queda desbloqueada.
- [ ] **Step 5:** Repite los pasos 1 y 2 con **Codex** y con **Gemini CLI**.
- [ ] **Step 6:** `python3 scripts/bundle_macos.py` y abre `target/Levare.app` desde Finder. Expected: los tres agentes aparecen disponibles (o con un motivo claro) y el paso 1 funciona.
- [ ] **Step 7:** Anota en el README los problemas conocidos de algún agente y haz commit de los arreglos con `git commit -m "Panel de agente: ajustes tras probar los tres agentes"`.

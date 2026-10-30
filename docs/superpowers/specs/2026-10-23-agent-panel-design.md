# Agent panel

Design of Levare's first AI feature: a conversation panel with a full agent that reads and
edits the open score. PDF transcription is left for a second phase that will build on this
panel.

## Intent

Writing in Levare with the keyboard is fast for notes, but many operations are heavy:
finding the bar, selecting the exact beat, finding the mark in the ribbon. The musician
wants to say it the way they would say it to a copyist:

> «Escribe una ligadura en el 1.er y 3.er tiempo del compás 4, pon un trino en el
> siguiente tiempo, agrega número de ensayo.»
> (Write a slur on the 1st and 3rd beat of bar 4, put a trill on the next beat, add a
> rehearsal number.)

The agent panel is exactly that: a conversation next to the score where you ask for anything
the editor knows how to do, and the agent does it on the real score.

The underlying goal (phase 2) is to hand over a PDF, for example from IMSLP, a single part or
a full score, and have the agent transcribe it into Levare so it can be played back and
practiced. That is why the panel is designed as a general agent and not as a «Transcribir PDF»
(Transcribe PDF) button: transcription will be just another request in the same conversation,
with the same tools, and so will the corrections afterwards.

## Goals

- A conversation side panel with an AI agent, in the style of Zed's agent panel.
- The agent is one the user already has installed, connected through **ACP**
  (Agent Client Protocol) and using the user's subscription: **Claude Code, Codex and
  Gemini CLI**, to be chosen among those that are installed. Levare does not manage API keys.
- The agent can read the open score and run **any editor operation**: writing notes and rests,
  slurs, articulations, ornaments, dynamics, rehearsal numbers, bars, key signature, time
  signature, parts, texts, etc.
- The agent understands musical positions: part or instrument, bar, beat, staff and voice.
- If the instrument is not stated and is clear from context (the selection, a single part, the
  last thing discussed), the agent uses it; if it is ambiguous, it asks instead of guessing.
- Changes show live on the score as the agent makes them.
- Everything the agent does in response to one message is undone with **a single ⌘Z**.

## Out of this phase

- **PDF transcription** (phase 2). Notes on what was discussed are at the end.
- An external agent (Claude Desktop, Claude Code in a terminal) controlling Levare from
  outside. The tools will be ready for it, but nothing is exposed outside the sessions that
  Levare itself opens.
- Own API keys or a built-in AI provider.
- Windows and Linux (Levare is only validated on macOS).

## Anti-goals

The user did not name any. These are my own and the builder must respect them:

- The agent changing things it was not asked to change, above all **removing** marks by
  accident (see «Toggles» below).
- A small request («put a trill here») taking long or consuming a lot of subscription because
  the agent has to read the whole score.
- The agent touching the `.partitura` file directly instead of using the editor: it would break
  undo, recovery and validations.

## How it works (decided)

### ACP: Levare is the client

Levare launches the chosen agent as a subprocess and talks to it over ACP
(<https://agentclientprotocol.com>). Gemini CLI speaks ACP natively; Claude Code and
Codex do so with the adapters that Zed publishes. Levare detects which ones are
available and lets the user choose.

### Levare's tools arrive over MCP, internal

ACP does not let the client declare its own tools: the client only offers a file
system, a terminal and permission requests. Custom tools reach the agent as MCP servers that
the client passes when opening the session (`session/new`, `mcpServers`). Therefore Levare
hosts **its own MCP server**, invisible to the user, and passes it to every ACP session it
opens. It is not a separate product: it is the way to declare the tools.

### The tools are the editor, not a parallel editor

The editor already works with explicit actions:

- `score_edit::Command` (`crates/score-edit/src/lib.rs`): about 80 actions with
  parameters (`WriteAt`, `SetDynamic`, `AddConnection`, `SetTimeSignatureAt`,
  `AddParts`, …). Many act **on the selection or the cursor** of the `Editor`.
- The registry of named commands (`crates/keymap/src/commands.rs`, about 250 stable
  `CommandId`s with a Spanish label and a category), which `AppView::run` executes and which is
  what shortcuts, the ribbon and the menus trigger.

The agent must operate **through those same actions**, as the user would: select a
musical position and run the action. The aim is that a new editor feature becomes
available to the agent without writing a tool by hand for it. How they are exposed (one generic
«run command» tool with the catalog described, or tools generated from the registry) is the
builder's decision.

### Toggles

Many actions are toggles: `ToggleMark`, `ToggleRehearsalMark` and the like remove
the mark if it is already there. The agent has to be able to **read what is at a position**
before acting. The tools must make this easy, and ideally the result of each
action says what changed («trill added», «trill removed»), so that the agent detects the
mistake and corrects it.

### Compact, chunked reading

Typical scores are orchestral, with about 32 parts (see the project's performance note). The
agent must not receive the whole score serialized. What is needed is a read by range
(parts × bars) in a compact text notation that a model can read, plus a cheap general
summary (parts, number of bars, key signatures, time signatures, tempo and the current
selection).

### Coarse-grained writing

For phase 2 to be viable, there must be a way to write one or several complete bars of a part
in a single call, in that compact notation, and not only note by note. Levare validates each bar
as it receives it (total duration, range, voices) and returns a clear error to the agent if
something does not add up, without applying that bar.

### Undo

The `Editor` keeps full snapshots (`Snapshot`) in `undo`. Everything the agent runs
while responding to **one message** is grouped into a single undo entry.
The user can keep undoing afterwards with the normal history.

### Live, with the score locked

The agent's edits are applied to the `Editor` of the open document on the interface
thread and the score is redrawn as they arrive. The user can stop the agent from
the panel at any time; what is already done stays, and is undone in one go.

**Decided by the user:** while the agent works, the score is locked
for the user (editing, undo and redo). If they want to touch something, they first stop
the agent. Playing, scrolling and zooming stay available. Since nobody else edits
during the turn, grouping into a single ⌘Z is one snapshot taken at the start
of the turn.

My decision: the agent uses the `Editor`'s cursor and selection, so the user
sees them move wherever it is working. When the turn ends (or is stopped), cursor and
selection return to where the user left them, because that selection is the context of the
next request.

## My decisions (revisable)

- The panel goes on a side, like Instruments or Properties, and is hidden from the
  header like the other panels.
- The agent applies changes directly, without asking for confirmation for each edit: the
  one-step undo acts as a safety net.
- One conversation per open document; closing the document closes the session.
  Saving conversation history is out of this phase.
- Interface texts in Spanish, like the rest of the app.
- **Which commands the agent can run.** «Any editor operation» means
  the categories that change or traverse the music: Edición (Editing), Entrada de notas (Note
  input), Duración (Duration), Alteraciones (Accidentals), Expresión (Expression), Notación
  (Notation), Compases (Bars), Instrumentos (Instruments), Navegación (Navigation) and Revisión
  (comments) (Review). Also Reproducción (Playback), so it can play a passage if asked.
  **Out** are Archivo (File: save, open, new, export) and Vista (View): the agent never
  writes to disk or closes the document, because that would end its own session. A new
  command in an allowed category becomes available to the agent with no extra work.

## What is left for the builder

- ACP and MCP crates in Rust, and the transport of the internal MCP server (stdio with a
  small bridge binary, HTTP on `localhost`, or whatever the three agents support). It must
  work with all three.
- How the Claude Code and Codex adapters are detected and installed, and what the user sees
  if they have neither.
- The compact text notation for reading and writing (it can take inspiration from ABC, LilyPond or
  similar, or be its own) and how positions are expressed («beat 3 of bar 4») taking
  into account compound meters, pickup bars, voices, piano staves and divisi.
- What to do with the agent's own capabilities (terminal, files): which permissions are
  granted to it in the session and how they are requested from the user through ACP's
  permission mechanism.
- Visual design of the panel within the existing dark theme (`crates/app-gpui/src/ui`).
- Crate structure (probably a new one for ACP/MCP without a GPUI dependency, like
  the rest of the model).

## Constraints

- Rust 2024, GPUI `=0.2.2`, dependencies with pinned versions like the rest of the workspace.
- `cargo fmt`, `cargo test --workspace --locked` and `cargo clippy … -D warnings` must
  pass (see README, «Verificar»).
- Do not degrade smoothness with 32-part scores: the agent's work cannot
  block the interface thread beyond applying each edit.
- Domain vocabulary according to `CONTEXT.md` (part, written / sounding pitch, etc.).

## Phase 2: PDF transcription (notes for later)

What was discussed, so it is not lost; it is not part of this phase:

- A PDF is dragged into the panel and the user asks «transcribe this». The agent looks at the
  pages and writes directly with the tools, **without going through MusicXML**, bar by bar,
  correcting whatever Levare rejects.
- IMSLP PDFs: old, noisy scans. Perfection is not expected; the errors that
  remain are corrected with the editor or by asking the agent.
- With dense orchestral pages, the agent should work with enlarged crops per
  system or group of staves.
- Levare already imports MusicXML (`crates/score-io`), but with strict rules that reject
  the whole work on an error; that is why it was discarded as the main path.

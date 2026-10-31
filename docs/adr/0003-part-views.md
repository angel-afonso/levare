# Part views on the canvas

The canvas will show an extracted part (`extract_part` + `layout_part`) while every edit keeps going to the full score's editor, because the part's `ElementId`s are the same as the full score's and there is no data to copy or synchronize. The first phase shows the part's engraving and edits go to the full score with the same `ElementId`s, but the part does not yet have its own layout (no system or page breaks of its own, and no multi-bar rests); compared with an independent part document, this avoids divergence, at the cost of re-extracting and re-engraving the part after every edit.

## Evidence

The tests are in `crates/score-layout/src/part_view_tests.rs` and the measurement is run with `cargo test -p score-layout --release part_view_cost -- --ignored --nocapture`.

| Question | Answer |
|---|---|
| 1. Identifiers | In a score with violin (with divisi, a tie and a comment), B♭ clarinet (with a crescendo and a comment) and piano (two staves, with a comment), every `ElementId` that `layout_part` exposes (events, strokes and glyphs of notes, techniques, slurs and hairpins) exists in the full score, belongs to the same part and points to the same `Event`. The set of displayed events matches that of the full score for that part, in `Page` and `Continuous`. The connections and comment anchors of the extracted part also resolve in the full score. The three tests pass with no production code changes. |
| 2. Clicks | All lookups read only the `Layout`, never the score: `hit_test` (`score-layout/src/lib.rs:366`), `hit_pitch` (369), `hit_pitch_in_voice` (372) and `hit_pitch_with_position` (380) return the `EventPosition.id` stored in `measures[].content.events`; `Layout::hit` (`expression.rs:96`) returns `Hit::Note(ElementId, …)` or `Hit::Mark(MarkRef)` taken from `MarkBox`; `measure_at` (`lib.rs:510`) returns `(PartId, StaffId, bar)` of the placed bars; `hit_bar_mark` (269) and `hit_tempo` (351) return bar indices. Since the extracted part keeps all the bars and their indices, the results are valid in the full score. The app calls them from `selection.rs`, `playhead.rs:99-102` and `ghost.rs:29`, always on `self.layout`, the canvas engraving. |
| 4. Cost (release) | Synthetic score of 34 parts and 32 bars, 4 quarter notes per bar. `extract_part`: 0.58 ms. `layout_part` (new engine, no cache): 4.45 ms. Together, first part: 5.23 ms; part with the most staves: 5.33 ms. Reference: `layout_view` of the full score, 18.3 ms cold and 3.7 ms with a warm bar cache. Budget: redraw < 8 ms and key→note < 16 ms (`docs/tools-and-integrations.md`, section 8). |
| 5. Differences | See the next section. |

## The `layout_view` calls (question 3)

Only two are production code; the other nine are inside `#[cfg(test)]` modules.

| File:line | What it does | In a part view |
|---|---|---|
| `app-gpui/src/main.rs:2754` | Canvas engraving, via `Document::layout_view` (cache of the last view). | Change: request `layout_part` of the extracted part and cache the extracted part alongside the view. |
| `app-gpui/src/main.rs:1276` | `ZOOM_FIT`: page engraving at 1000 wide to compute the zoom. | Change: a part's page is 1280 wide, so it must use the same engraving as the canvas. |
| `app-gpui/src/follow.rs:371` | Test: page navigation. | Nothing. |
| `app-gpui/src/follow.rs:433` | Test: framing of an element in three modes and scales. | Nothing; can be extended with a part variant. |
| `app-gpui/src/follow.rs:923` | Test: framing of piano arpeggios. | Nothing. |
| `app-gpui/src/playhead.rs:267` | Test: the cursor stays on its staff. | Nothing. |
| `app-gpui/src/playhead.rs:339` | Test: voice 2 and divisi cursors. | Nothing. |
| `app-gpui/src/playhead.rs:424` | Test: clicks with exact onsets. | Nothing. |
| `app-gpui/src/paint.rs:1231` | Test: slur ink. | Nothing. |
| `app-gpui/src/selection.rs:1290` | Test: selection with divisi (passage). | Nothing. |
| `app-gpui/src/selection.rs:1361` | Test: selection with divisi (mid-bar). | Nothing. |

There are other production engraving paths that are not `layout_view` and that will also have to look at the active view: the initial layout `document.layout(960.)` (`main.rs:171`) and the note drag preview (`layout_pitch_preview` on the preview score, `main.rs`, `note_drag` branch), which today engraves the whole full score.

## Differences between the part and the full score (question 5)

| Aspect | Today | Missing |
|---|---|---|
| Written and sounding pitches | Notes are stored at written pitch and the part carries the written→sounding interval (ADR 0001). The extracted part is a clone with a single part, so it keeps the same written pitch and the same key. The drawing of the key signature between both views has not been compared; only the fact that the events match was tested. | Decide whether the full score will ever be shown in sounding pitch; then the part would stay written and the difference would be visible. |
| Divisi | `layout_part` keeps the divisi labels; the voice 2 identifiers match (test 1). | Nothing for the first phase. |
| Multi-bar rests | There are no grouped multi-bar rests in the engraving (there is no support at all in `score-layout`). | A new feature, useful mostly in parts. |
| System and page breaks | `layout_part` ignores the score's breaks since plan 024 (`views.rs`: `page_after` empty for parts) even though `extract_part` clones them. | A break model of each part's own, which would have to be stored and versioned. |
| Names and header | Header with the part's name instead of repeated names; 1280-wide sheet and at most 6 bars per system. | Nothing. |
| Comments | `extract_part` keeps only those of that part. | Nothing. |

## The Parts tab

Initial proposal: a `TabId::Parts` tab («Partes», Parts) with two groups. «Ver parte» (View part) with a «Ver parte ▾» dropdown (a «Partitura general» (full score) entry and one per instrument) and «Exportar» (Export) with «Exportar partes…» (Export parts…) (`id::EXPORT`). Command uniqueness is only required within the ribbon (`TABS`), not against `FILE_MENU`, so `id::EXPORT` is added to the group and also stays in the «Archivo» (File) menu, like `id::SCORE_INFO`.

Rules of `crates/toolbar/src/spec.rs` that it must satisfy: each tab has groups, each group has at least one ready command, each command appears once in the whole ribbon and each tab has its own `TabId::command` command. That requires `id::TAB_PARTS` and a free shortcut (`alt-cmd-9`). In addition, the current control model (`Role`: Choice, Toggle, Action) has no dropdown with dynamic entries, so a new control is needed, or a context menu that opens from an action.

## Risks and open questions

- The `LayoutEngine` cache is indexed by `(PartId, StaffId, bar)`; it has not been checked whether mixing full-score and part engraving in a single engine contaminates it. The measurement used a new engine each time. Recommendation: a separate engine for the part view.
- `layout_part` was not measured with a warm cache or with a real score (only the synthetic one described); the 5.2 ms is a cold upper bound for this load.
- The measurement is the median time of 20 repetitions on a single machine, not broken down by phase.
- The editor's selection and cursor registry use `PartId`, `StaffId` and bar, which do not change when showing a part; the behavior with multi-part selections (for example a copy between instruments) in a part view has not been investigated.
- Findings in production code for the build plan: `Document` must store the active part and cache the extracted score, `ZOOM_FIT` must use the same engraving as the canvas and note dragging must engrave the part.

## Decision

Build the "live" part view on the canvas: the part's engraving is shown and edits go to the full score with the same `ElementId`s, with no layout of its own yet (no breaks of its own and no multi-bar rests). The build plan stays as draft 029.

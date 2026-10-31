# Follow-up to audit 001 · 2026-10-22

All findings (1 to 17) were approved. Below is what was done for each one and
how it was checked. The ratios were calculated with `contrast-check.py`.

## Verification

- `cargo fmt --all`: clean.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: no warnings.
- `cargo test --workspace --locked`: all green (including the 129 tests of `levare`).
- The packaged app (`scripts/bundle_macos.py`) was opened with an isolated `LEVARE_DATA_DIR`:
  - «Pendiente» shows on the transport's loop button and on MIDI.
  - The input pill reads «Escribiendo · Negra · Voz 1», with no dot.
  - The status bar reads «Guardado», with no ✓.
  - The placeholder «Compositor, arreglista, año…» is legible on the sheet.
  - When the search box opens, its field shows the blue focus border.
- The window was on another desktop, so the typed text did not reach it. The
  «Pendiente» marks in the results, the menus and the ribbon rows were only
  checked by reading the code.

## Result per finding

1. **Done.** The fields' placeholder uses `PLACEHOLDER` (= `MUTED`): 5.63:1 on `FIELD`.
2. **Done.** The credits placeholder uses `PAPER_PLACEHOLDER 0x6f6a62`: 5.10:1.
3. **Done.** `CAPTION` becomes `0x8a8f9c`: 5.02:1 on `CARD` and 5.53:1 on `SUNKEN`.
4. **Done.** The page number becomes `0x67635c` (5.67:1), with a minimum of 10 px.
5. **Partial.**
   - Done: `TextInput` shows a blue border when focused and the text cursor uses
     the theme blue instead of `gpui::blue()`.
   - Shortcuts already existed for what previously seemed to work only with the mouse: the sound
     chip (Mostrar los sonidos), the header (Información de la partitura) and
     play from a selection (`P`). None of them is written in the interface.
   - Not done: Tab moving through the ribbon and panel buttons, with visible
     focus. In GPUI each button needs its own `FocusHandle`, so it is a
     separate task.
6. **Done.** The panel buttons show a tooltip with the name and the shortcut
   (`TOGGLE_INSTRUMENTS_PANEL`, `TOGGLE_INSPECTOR`).
7. **Done, with a limit.**
   - «TODO» no longer appears in any user-facing text. The badges say «Pendiente»
     (`pending_badge`), as do the velocity and duration sliders and the loop
     message.
   - The menus, the ribbon rows and the search share `pending_mark()`, and the
     tooltip of each pending control adds «· Pendiente».
   - The limit: a ribbon control that shows only a glyph (such as the thirty-second note) has
     no room for text. There the mark is the dashed border plus the tooltip.
   - The README was updated.
8. **Done.** The audio errors say what to do («Pulsa Reintentar audio en
   Sonidos»). The status bar no longer prefixes «Audio:».
   - «Sin acción para «X»» becomes ««X» no está disponible en esta versión».
   - Cancelling a shortcut conflict says «Se conserva el atajo anterior.».
   - «Listo para reproducir» was removed.
9. **Done.**
   - The comment at `ui/toolbar.rs:2` uses a colon.
   - In `tools-and-integrations.md`, the title and section 4 do too.
   - The aside with dashes at `development.md:687` becomes parentheses.
10. **Done.** The generator writes «Sin atajo». `docs/shortcuts.md` was regenerated and no
    longer has a single em dash.
11. **Done.** The window title is «Partitura • · Levare».
12. **Done.**
    - The section labels stay in uppercase, and the reason is written in `caption()`.
    - The comment states become «Resuelto» / «Pendiente».
13. **Done.** The new `ui/theme.rs` module gathers all the interface colours,
    with names:
    - A single `WARNING`, `ERROR` and `SUCCESS`.
    - A single accent text tone (`ACCENT_TEXT`), replacing `a9c4ff`, `a9bfe7`
      and `b8ceff`.
    - `0x3a3d46` and `0x3b3d46` are unified into `POPOVER_BORDER`.
    - `ribbon_action()` is gone: the ribbon uses `action_with_hover()` from `ui.rs`.
    - The score's ink colours stay in `paint` and `playhead`.
14. **Done.** ● and ✓ were removed from «Cambios sin guardar», «Guardado», «Escribiendo…» and
    «Audición al escribir». The tab dot and the title's «•» stay.
15. **Done.**
    - `Role::Toggle` and `Role::Action` describe how they are drawn.
    - The comment on `without_keys` was removed.
    - `part_view_tests.rs` points to ADR 0003 instead of «plan 025».
16. **Done.** `tools-and-integrations.md`:
    - The initial table is marked as a snapshot.
    - `partitura` becomes `levare`.
    - The sentence about git was removed.
    - The open questions become «Preguntas resueltas» (Resolved questions).
17. **Done.** «Armónicos de cuerda» now goes right after «Pizzicato y arco».

## Pending

- Tab navigation throughout the interface (part of finding 5).

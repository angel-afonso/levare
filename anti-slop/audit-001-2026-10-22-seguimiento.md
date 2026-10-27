# Seguimiento de la auditoría 001 · 2026-10-22

Se aprobaron todos los hallazgos (1 a 17). Abajo está lo que se hizo con cada uno y
cómo se comprobó. Los ratios se calcularon con `contrast-check.py`.

## Verificación

- `cargo fmt --all`: limpio.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: sin avisos.
- `cargo test --workspace --locked`: todo en verde (los 129 tests de `levare` incluidos).
- La app empaquetada (`scripts/bundle_macos.py`) se abrió con `LEVARE_DATA_DIR` aislado:
  - «Pendiente» se ve en el botón de bucle del transporte y en MIDI.
  - La píldora de entrada dice «Escribiendo · Negra · Voz 1», sin punto.
  - La barra de estado dice «Guardado», sin ✓.
  - El texto de ayuda «Compositor, arreglista, año…» se lee en la hoja.
  - Al abrir el buscador, su campo muestra el borde azul de foco.
- La ventana estaba en otro escritorio, así que el texto escrito no le llegó. Las
  marcas «Pendiente» de los resultados, de los menús y de las filas de la cinta solo
  se comprobaron leyendo el código.

## Resultado por hallazgo

1. **Hecho.** El texto de ayuda de los campos usa `PLACEHOLDER` (= `MUTED`): 5,63:1 sobre `FIELD`.
2. **Hecho.** El texto de ayuda de los créditos usa `PAPER_PLACEHOLDER 0x6f6a62`: 5,10:1.
3. **Hecho.** `CAPTION` pasa a `0x8a8f9c`: 5,02:1 sobre `CARD` y 5,53:1 sobre `SUNKEN`.
4. **Hecho.** El número de página pasa a `0x67635c` (5,67:1), con un mínimo de 10 px.
5. **Parcial.**
   - Hecho: `TextInput` muestra un borde azul al enfocarse y el cursor de texto usa
     el azul del tema en vez de `gpui::blue()`.
   - Ya existían atajos para lo que antes solo parecía funcionar con ratón: el chip
     de sonido (Mostrar los sonidos), la cabecera (Información de la partitura) y
     reproducir desde una selección (`P`). Ninguno está escrito en la interfaz.
   - Sin hacer: que Tab recorra los botones de la cinta y de los paneles, con foco
     visible. En GPUI cada botón necesita su propio `FocusHandle`, así que es una
     tarea aparte.
6. **Hecho.** Los botones de panel muestran tooltip con el nombre y el atajo
   (`TOGGLE_INSTRUMENTS_PANEL`, `TOGGLE_INSPECTOR`).
7. **Hecho, con un límite.**
   - «TODO» ya no aparece en ningún texto de usuario. Los badges dicen «Pendiente»
     (`pending_badge`), igual que los deslizadores de velocidad y duración y el mensaje
     de bucle.
   - Los menús, las filas de la cinta y el buscador comparten `pending_mark()`, y el
     tooltip de cada control pendiente añade «· Pendiente».
   - El límite: un control de la cinta que solo muestra un glifo (como la fusa) no
     tiene sitio para texto. Ahí la marca es el borde discontinuo más el tooltip.
   - El README se actualizó.
8. **Hecho.** Los errores de audio dicen qué hacer («Pulsa Reintentar audio en
   Sonidos»). La barra de estado ya no antepone «Audio:».
   - «Sin acción para «X»» pasa a ««X» no está disponible en esta versión».
   - Cancelar un conflicto de atajos dice «Se conserva el atajo anterior.».
   - Se quitó «Listo para reproducir».
9. **Hecho.**
   - El comentario de `ui/toolbar.rs:2` usa dos puntos.
   - En `herramientas.md`, el título y la sección 4 también.
   - El inciso con rayas de `desarrollo.md:687` pasa a paréntesis.
10. **Hecho.** El generador escribe «Sin atajo». `docs/atajos.md` se regeneró y ya no
    tiene ninguna raya.
11. **Hecho.** El título de la ventana es «Partitura • · Levare».
12. **Hecho.**
    - Los rótulos de sección siguen en mayúsculas, y el motivo está escrito en `caption()`.
    - Los estados de comentario pasan a «Resuelto» / «Pendiente».
13. **Hecho.** El nuevo módulo `ui/theme.rs` reúne todos los colores de la interfaz,
    con nombre:
    - Un solo `WARNING`, `ERROR` y `SUCCESS`.
    - Un solo tono de texto de acento (`ACCENT_TEXT`), que sustituye a `a9c4ff`, `a9bfe7`
      y `b8ceff`.
    - `0x3a3d46` y `0x3b3d46` quedan unificados en `POPOVER_BORDER`.
    - `ribbon_action()` desaparece: la cinta usa `action_with_hover()` de `ui.rs`.
    - Los colores de la tinta de la partitura siguen en `paint` y `playhead`.
14. **Hecho.** Se quitaron ● y ✓ de «Cambios sin guardar», «Guardado», «Escribiendo…» y
    «Audición al escribir». El punto de la pestaña y el «•» del título se mantienen.
15. **Hecho.**
    - `Role::Toggle` y `Role::Action` describen cómo se dibujan.
    - Se quitó el comentario de `without_keys`.
    - `part_view_tests.rs` remite al ADR 0003 en lugar de al «plan 025».
16. **Hecho.** `herramientas.md`:
    - La tabla inicial queda marcada como foto fija.
    - `partitura` pasa a `levare`.
    - Se quitó la frase sobre git.
    - Las preguntas abiertas pasan a «Preguntas resueltas».
17. **Hecho.** «Armónicos de cuerda» va ahora justo después de «Pizzicato y arco».

## Pendiente

- La navegación con Tab por toda la interfaz (parte del hallazgo 5).

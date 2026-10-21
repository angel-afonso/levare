# Vistas de parte en el lienzo

El lienzo mostrará una parte extraída (`extract_part` + `layout_part`) mientras toda edición sigue yendo al editor de la partitura general, porque los `ElementId` de la parte son los mismos de la general y no hay datos que copiar ni sincronizar. La primera fase muestra el grabado de la parte y las ediciones van a la partitura general con los mismos `ElementId`, pero la parte aún no tiene disposición propia (ni saltos de sistema o de página propios, ni compases de espera); frente a un documento de parte independiente, esto evita divergencias, a cambio de volver a extraer y regrabar la parte tras cada edición.

## Evidencia

Las pruebas están en `crates/score-layout/src/part_view_tests.rs` y la medición se ejecuta con `cargo test -p score-layout --release part_view_cost -- --ignored --nocapture`.

| Pregunta | Respuesta |
|---|---|
| 1. Identificadores | En una partitura de violín (con divisi, ligadura de unión y comentario), clarinete en Si♭ (con crescendo y comentario) y piano (dos pentagramas, con comentario), cada `ElementId` que expone `layout_part` (eventos, trazos y glifos de nota, técnicas, ligaduras y reguladores) existe en la general, pertenece a la misma parte y apunta al mismo `Event`. El conjunto de eventos mostrados coincide con el de la general para esa parte, en `Page` y `Continuous`. Las conexiones y los anclajes de comentarios de la parte extraída también se resuelven en la general. Los tres tests pasan sin cambios de código de producción. |
| 2. Pulsaciones | Todas las búsquedas leen solo el `Layout`, nunca la partitura: `hit_test` (`score-layout/src/lib.rs:366`), `hit_pitch` (369), `hit_pitch_in_voice` (372) y `hit_pitch_with_position` (380) devuelven el `EventPosition.id` guardado en `measures[].content.events`; `Layout::hit` (`expression.rs:96`) devuelve `Hit::Note(ElementId, …)` o `Hit::Mark(MarkRef)` sacado de `MarkBox`; `measure_at` (`lib.rs:510`) devuelve `(PartId, StaffId, compás)` de los compases colocados; `hit_bar_mark` (269) y `hit_tempo` (351) devuelven índices de compás. Como la parte extraída conserva todos los compases y sus índices, los resultados son válidos en la general. La app los llama desde `selection.rs`, `playhead.rs:99-102` y `ghost.rs:29` siempre sobre `self.layout`, el grabado del lienzo. |
| 4. Coste (release) | Partitura sintética de 34 partes y 32 compases, 4 negras por compás. `extract_part`: 0,58 ms. `layout_part` (motor nuevo, sin caché): 4,45 ms. Juntos, primera parte: 5,23 ms; parte con más pentagramas: 5,33 ms. Referencia: `layout_view` de la general, 18,3 ms en frío y 3,7 ms con la caché de compases tibia. Presupuesto: redibujar < 8 ms y tecla→nota < 16 ms (`docs/herramientas.md`, sección 8). |
| 5. Diferencias | Ver la sección siguiente. |

## Las llamadas a `layout_view` (pregunta 3)

Solo dos son de producción; las otras nueve están dentro de módulos `#[cfg(test)]`.

| Archivo:línea | Qué hace | En una vista de parte |
|---|---|---|
| `app-gpui/src/main.rs:2754` | Grabado del lienzo, vía `Document::layout_view` (caché de la última vista). | Cambiar: pedir `layout_part` de la parte extraída y guardar en caché la parte extraída junto a la vista. |
| `app-gpui/src/main.rs:1276` | `ZOOM_FIT`: grabado de página a 1000 de ancho para calcular el zoom. | Cambiar: la página de una parte mide 1280, así que debe usar el mismo grabado que el lienzo. |
| `app-gpui/src/follow.rs:371` | Prueba: navegación de páginas. | Nada. |
| `app-gpui/src/follow.rs:433` | Prueba: encuadre de un elemento en tres modos y escalas. | Nada; se puede ampliar con una variante de parte. |
| `app-gpui/src/follow.rs:923` | Prueba: encuadre de arpegios de piano. | Nada. |
| `app-gpui/src/playhead.rs:267` | Prueba: el cursor se queda en su pentagrama. | Nada. |
| `app-gpui/src/playhead.rs:339` | Prueba: cursores de voz 2 y divisi. | Nada. |
| `app-gpui/src/playhead.rs:424` | Prueba: clics con onsets exactos. | Nada. |
| `app-gpui/src/paint.rs:1231` | Prueba: tinta de las ligaduras. | Nada. |
| `app-gpui/src/selection.rs:1290` | Prueba: selección con divisi (pasaje). | Nada. |
| `app-gpui/src/selection.rs:1361` | Prueba: selección con divisi (a mitad de compás). | Nada. |

Hay otros caminos de grabado en producción que no son `layout_view` y que también deberán mirar la vista activa: la disposición inicial `document.layout(960.)` (`main.rs:171`) y la vista previa de arrastre de notas (`layout_pitch_preview` sobre la partitura de previsualización, `main.rs`, rama `note_drag`), que hoy graba la general completa.

## Diferencias entre la parte y la general (pregunta 5)

| Aspecto | Hoy | Falta |
|---|---|---|
| Alturas escritas y sonoras | Las notas se guardan en altura escrita y la parte lleva el intervalo escrito→sonoro (ADR 0001). La parte extraída es un clon con una sola parte, así que conserva la misma altura escrita y la misma tonalidad. No se ha comparado el dibujo de armadura entre ambas vistas; solo se probó que los eventos coinciden. | Decidir si algún día la general se verá en altura real; entonces la parte seguirá escrita y la diferencia será visible. |
| Divisi | `layout_part` conserva las etiquetas de divisi; los identificadores de la voz 2 coinciden (prueba 1). | Nada para la primera fase. |
| Compases de espera | No hay compases de espera agrupados en el grabado (no existe ningún soporte en `score-layout`). | Una función nueva, útil sobre todo en partes. |
| Saltos de sistema y de página | `layout_part` ignora los saltos de la partitura desde el plan 024 (`views.rs`: `page_after` vacío para partes) aunque `extract_part` los clona. | Un modelo de saltos propio de cada parte, que habría que guardar y versionar. |
| Nombres y encabezado | Encabezado con el nombre de la parte en lugar de nombres repetidos; hoja de 1280 y 6 compases como máximo por sistema. | Nada. |
| Comentarios | `extract_part` solo conserva los de esa parte. | Nada. |

## La pestaña Partes

Propuesta inicial: una pestaña `TabId::Parts` («Partes») con dos grupos. «Ver parte» con un desplegable «Ver parte ▾» (entrada «Partitura general» y una por instrumento) y «Exportar» con «Exportar partes…» (`id::EXPORT`). La unicidad de comandos se exige solo dentro de la cinta (`TABS`), no frente a `FILE_MENU`, así que `id::EXPORT` se añade al grupo y se queda también en el menú Archivo, como `id::SCORE_INFO`.

Reglas de `crates/toolbar/src/spec.rs` que debe cumplir: cada pestaña tiene grupos, cada grupo al menos un comando listo, cada comando aparece una vez en toda la cinta y cada pestaña tiene su propio comando `TabId::command`. Eso exige `id::TAB_PARTS` y un atajo libre (`alt-cmd-9`). Además, el modelo actual de controles (`Role`: Choice, Toggle, Action) no tiene un desplegable con entradas dinámicas, así que hace falta un control nuevo o un menú contextual que se abra desde una acción.

## Riesgos y preguntas abiertas

- La caché de `LayoutEngine` se indexa por `(PartId, StaffId, compás)`; no se ha comprobado si mezclar grabado general y de parte en un mismo motor la contamina. La medición usó un motor nuevo cada vez. Recomendación: un motor aparte para la vista de parte.
- No se midió `layout_part` con la caché tibia ni con una partitura real (solo la sintética descrita); los 5,2 ms son un tope en frío para esta carga.
- La medición es de tiempo de mediana de 20 repeticiones en una sola máquina, sin repartir por fases.
- El registro de la selección y el cursor del editor usan `PartId`, `StaffId` y compás, que no cambian al mostrar una parte; el comportamiento con selecciones multiparte (por ejemplo un copiado entre instrumentos) en una vista de parte no se ha investigado.
- Hallazgos de código de producción para el plan de construcción: `Document` debe guardar la parte activa y cachear la partitura extraída, `ZOOM_FIT` debe usar el mismo grabado que el lienzo y el arrastre de notas debe grabar la parte.

## Decisión

Construir la vista de parte «en vivo» en el lienzo: se muestra el grabado de la parte y las ediciones van a la partitura general con los mismos `ElementId`, sin disposición propia todavía (ni saltos propios ni compases de espera). El plan de construcción queda como borrador 029.

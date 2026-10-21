# Rangos instrumentales

El editor marca en rojo las alturas fuera del registro de referencia de cada
instrumento. Los límites son inclusivos y se comparan en **altura real**, con
C4 como do central. La armadura escrita, la alteración explícita, la transposición
almacenada en la parte y los armónicos intervienen en la comparación. Esto también
permite abrir documentos antiguos o importados con otra transposición.

Son avisos de registro habitual profesional, no límites físicos absolutos ni
niveles de dificultad. No impiden editar, guardar, reproducir o exportar. No se
deducen del banco de sonidos y no se guardan colores en el documento. La tinta
roja se aplica únicamente en el editor; la selección y la reproducción conservan
el aviso. Las marcas, silencios y líneas del pentagrama conservan su color.

## Referencias y convenciones

Los registros orquestales se basan en las secciones **Range** de
[Vienna Academy](https://www.vsl.co.at/academy), especialmente
[cuerdas](https://www.vsl.co.at/academy/strings/violin),
[clarinetes](https://www.vsl.co.at/academy/woodwinds/clarinet),
[flauta](https://www.vsl.co.at/academy/woodwinds/concert-flute),
[corno](https://www.vsl.co.at/academy/brass/horn-f) y
[trombón](https://www.vsl.co.at/academy/brass/tenor-trombone).
Cuando el catálogo no distingue modelos, el editor adopta una referencia fija:

- Flauta con pie de si: B3–D7; piccolo ordinario: D5–C8.
- Contrabajo orquestal de cinco cuerdas: B0–G4; clarinete bajo con do grave:
  B♭1–B5; saxofón barítono con la grave: C2–A4.
- Saxofones con fa sostenido agudo escrito, sin registro altissimo.
- Trombón tenor: E2–F5, sin incluir pedales; trombón bajo: B♭0–F5.
- Tuba genérica: E1–F4. Timbales: conjunto habitual C2–A3; no se evalúan
  la distribución entre tambores ni el tiempo necesario para afinarlos.
- [Trompeta piccolo en si bemol](https://www.vsl.co.at/academy/brass/piccolo-trumpet)
  de cuatro válvulas: D4–G6 en altura real (E3–A5 escrito).
- Guitarra clásica de 19 trastes: E2–B5; piano de 88 teclas: A0–C8;
  arpa de pedales: C1–G7.
- Marimba de cinco octavas: C2–C7; vibráfono: F3–F6; celesta de cuatro
  octavas sonoras: C4–C8. Órgano: teclados C2–C7 sin transposiciones de registros.

La tabla completa está junto al catálogo del modelo, en
`crates/score-core/src/instrument_range.rs`. Cambiar de clave no cambia el
registro. Cada cabeza escrita de un acorde se evalúa por separado, también
en segunda voz y en ambos pentagramas de una parte.

## Armónicos

Las notas con armónico natural se escriben a su altura sonora. Su límite
inferior es el segundo parcial de la cuerda más grave. Los artificiales de cuarta
suenan dos octavas por encima de la nota pisada; el rombo de contacto no cuenta
como otra nota. Los máximos son D8 para violín, E7 para viola, A7 para violoncello
y G6 para contrabajo, según las referencias de cuerda de Vienna Academy.

El aviso comprueba el registro sonoro, no la existencia de cada parcial natural,
la digitación de un acorde o la dificultad de una técnica.

## Rangos personalizados y revisión

**Revisión › Rangos › Editar rango…** permite sustituir la referencia habitual
de la parte activa por límites propios en altura real. Se aceptan nombres
científicos (`C4`, `F#3`, `B♭5`) o españoles (`Do4`, `Fa♯3`, `Si♭5`), desde C0
hasta G9. Ambos extremos son inclusivos; el límite inferior no puede superar al
superior. Los límites personalizados se aplican al sonido de todos los eventos,
incluidos los armónicos. Restablecer elimina la excepción y recupera los límites
habituales y el tratamiento específico de armónicos descrito arriba.

La excepción se guarda en la parte, se conserva al extraerla y admite historial.
El formato nativo 26 mantiene la compatibilidad de lectura con los archivos
anteriores, que conservan los valores habituales. MusicXML y PDF no incluyen
estos límites de revisión. No cambian las notas, el sonido ni el grabado exportado.

El botón **Notas en rojo** cambia únicamente la visibilidad del aviso durante
la sesión. **Comprobar** sigue encontrando notas fuera de rango cuando el color
está oculto. La lista de avisos permite navegar a cada nota del acorde, filtrar
por parte y revisar también problemas de repeticiones y saltos.

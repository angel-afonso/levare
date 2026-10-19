# Partitura — Herramientas e integraciones

Inventario de las piezas necesarias para que Partitura sea una alternativa sólida a Sibelius:
capacidades completas, alta estabilidad y, sobre todo, velocidad.

> **Estado:** inventario de reconocimiento (2026-10-17). La primera entrega ya está
> implementada; ver [desarrollo.md](desarrollo.md) para el alcance, las versiones
> fijadas y las siguientes etapas. Las secciones siguientes conservan la visión completa.
>
> **★** = no existe librería que lo resuelva; hay que construirlo.

---

## 0. Punto de partida

El workspace empezó como un esqueleto. Estado tras la primera entrega:

| Crate | Rol previsto | Estado |
|---|---|---|
| `partitura` (`crates/app-gpui`) | Aplicación GPUI | Interfaz oscura con cinta, orden automático por familias, eliminación visible de instrumentos, inspector, transporte, zoom y controles TODO |
| `score-core` | Modelo musical | Partes, pentagramas, instrumentos, acordes de hasta ocho notas, sostenidos/bemoles/becuadros, armaduras mayores/menores, ligaduras de unión/expresión, dinámicas, reguladores de crescendo/diminuendo, articulaciones, pizzicato/arco, silencios, duraciones con puntillo y tresillo 3:2, compases e IDs |
| `score-edit` | Comandos y deshacer/rehacer | Selección de pasajes, copiar/cortar/pegar con ligaduras y marcas, ediciones transaccionales e instantáneas persistentes |
| `score-document` | Documento/sesión | Editor, caché de layout y estado de guardado |
| `score-layout` | Motor de grabado/posicionamiento | Barras de unión automáticas por pulso y tresillo, barras dobles/parciales, corchetes de familia, barras de compás agrupadas, llaves de piano, marcas Bravura y curvas entre sistemas, escala de presentación y espaciado compacto por límites de tinta, armaduras por sistema, alteraciones en columnas, acordes con plica común, gran pentagrama de piano, pentagramas alineados, claves sol/do/fa y caché por pentagrama/compás |
| `score-playback` | Interpretación → eventos MIDI | Puntillos y tresillos con reloj entero exacto, reguladores con expresión continua y manos de piano independientes, pizzicato/arco persistente, dinámicas persistentes, staccato/acentos, cadenas ligadas, línea de tiempo común, canales por parte y recuperación del ataque al iniciar desde selección |
| `score-io` | Formatos de archivo | JSON nativo versionado, validación y guardado atómico |
| `score-platform` (`crates/platform`) | Audio y MIDI del sistema (cpal, midir) | Audio CPAL, piano RustySynth/GeneralUser GS, cola rtrb; MIDI externo pendiente |

`assets/fonts` incluye Bravura y su licencia; hay instrucciones de desarrollo y CI
en `.github/workflows`. El proyecto aún no es un repositorio git.

### Ajustes de arquitectura detectados y resueltos en la primera entrega

- Eliminada la dependencia `score-io → score-playback`; la interfaz de eventos para
  MIDI se definirá al implementar reproducción/exportación.
- Añadida la capa de **documento/sesión** con partitura, selección, cursor, historial
  y estado de guardado. La interfaz coordina la reproducción mediante un plan
  inmutable, detiene planes obsoletos al editar y conserva separada la selección.
- Añadida la capa de **entrada**: atajos de teclado → comandos de edición.
- Añadida la capa de **dibujo**: geometría de `score-layout` → pintado de GPUI.

---

## 1. Interfaz

| Pieza | Herramienta | Para qué |
|---|---|---|
| Framework | **gpui** | Ventanas, eventos y dibujo por GPU (Metal en macOS). Base de la velocidad de la interfaz. |
| Widgets | **gpui-component** | Menús, paneles acoplables, inputs, diálogos y paletas. GPUI solo trae primitivas. |
| Lienzo de la partitura | ★ propio, sobre la API de pintado de GPUI | Pentagramas, glifos, curvas, selección y cursor. |
| Diálogos de archivo | API de GPUI (respaldo: **rfd**) | Abrir y guardar. |

## 2. Tipografía musical

| Pieza | Herramienta | Para qué |
|---|---|---|
| Fuente | **Bravura** o **Leland** (estándar SMuFL, licencia OFL) | Figuras, claves, alteraciones, dinámicas y articulaciones. |
| Métricas | Metadatos JSON de la fuente + **serde_json** | Anclajes exactos (plicas, bordes, grosores recomendados). Sin esto el resultado se ve amateur. |
| Contornos | **skrifa** o **ttf-parser** | Extraer los glifos para pintarlos y cachearlos. |

## 3. Núcleo musical (`score-core`, `score-edit`)

| Pieza | Herramienta | Para qué |
|---|---|---|
| Modelo | ★ propio | Partitura, instrumentos, pentagramas, compases, voces, notas, silencios, tuplets, ligaduras, letra, acordes, tonalidad, compás y tempo. |
| IDs estables | **slotmap** | Referencias a elementos que sobreviven a ediciones. |
| Deshacer/rehacer | **imbl** (estructuras persistentes) | Instantáneas baratas: deshacer instantáneo e ilimitado, y lecturas seguras desde otros hilos (layout, reproducción). |
| Teoría musical | ★ propio | Transposición, enarmonía, tonalidades, rangos de instrumentos. |
| Base de instrumentos | ★ datos propios | Nombres, claves, transposición, rangos, programa General MIDI. (Las listas de MuseScore son GPL; no se pueden copiar en un proyecto MIT.) |
| Entrada rápida | ★ capa de comandos y atajos | Estilo Sibelius: letra = nota, número = duración, flechas = moverse/transponer. Es el corazón de "rápido de usar". |

## 4. Motor de layout (`score-layout`) — la parte más difícil

| Pieza | Herramienta | Para qué |
|---|---|---|
| Espaciado horizontal | ★ algoritmo tipo Gourlay | Espaciar notas según su duración, con alineación vertical entre pentagramas. |
| Saltos de línea y página | ★ optimización tipo Knuth-Plass | Repartir sistemas de forma equilibrada. |
| Colisiones | ★ "skylines" | Evitar choques entre dinámicas, articulaciones, ligaduras y texto. |
| Curvas | **kurbo** | Ligaduras de expresión, ligaduras de valor y reguladores en Bézier. |
| Selección con el ratón | **rstar** (índice espacial) | Saber qué elemento hay bajo el cursor en microsegundos. |
| Paralelismo | **rayon** | Calcular sistemas en paralelo. |
| Referencia | Verovio (C++), LilyPond, MuseScore | Solo para estudiar algoritmos. Verovio no es interactivo; MuseScore y LilyPond son GPL. |

**Clave de rendimiento:** layout **incremental**. Al editar un compás solo se recalcula el sistema
afectado (y los siguientes si cambia el reparto de líneas), nunca la obra entera.

## 5. Reproducción y sonido (`score-playback`, `score-platform`)

| Pieza | Herramienta | Para qué |
|---|---|---|
| Interpretación | ★ propio | Expandir repeticiones, D.C., D.S. y Coda; dinámicas → velocidad; articulaciones → duración; swing; mapa de tempo; cursor de reproducción sincronizado. |
| Salida de audio | **cpal** | Enviar audio a la tarjeta de sonido. |
| Sintetizador | **rustysynth** (SoundFont, Rust puro) u **oxisynth** | Sonido integrado sin dependencias externas. |
| Sonidos | **GeneralUser GS** o **MuseScore_General** (licencias permisivas; confirmar antes de distribuir) | Banco General MIDI incluido con la app. |
| MIDI externo | **midir** | Enviar a otros programas o dispositivos y grabar desde un teclado MIDI. |
| Hilo de audio en tiempo real | **rtrb** (cola sin bloqueos) + **assert_no_alloc** | Que el audio nunca se corte: el hilo de audio no bloquea ni reserva memoria. |
| Sonido profesional (fase posterior) | **clack** (host CLAP) o hosting VST3 | Usar librerías de sonido de terceros. |

## 6. Archivos (`score-io`)

| Formato | Herramienta | Para qué |
|---|---|---|
| Formato propio | **serde** + **postcard**/JSON dentro de un **zip**, con versionado de esquema | Guardado rápido; los archivos siguen abriendo en versiones futuras. |
| MusicXML | **quick-xml** | Importar y exportar con Sibelius, MuseScore, Dorico y Finale. Imprescindible para ser una alternativa real. |
| MIDI | **midly** | Leer y escribir archivos `.mid`. |
| PDF | **krilla** | Impresión y exportación con calidad de imprenta. |
| SVG/PNG | **tiny-skia** + **resvg** | Exportar imágenes. |
| Audio | **hound** (WAV), **flacenc** (FLAC) | Exportar la mezcla. |

## 7. Estabilidad

| Pieza | Herramienta | Para qué |
|---|---|---|
| Autoguardado y recuperación | ★ propio | No perder trabajo ante un cierre inesperado. |
| Tests de layout | **insta** (snapshots) | Detectar cualquier cambio visual no intencionado. |
| Tests del modelo | **proptest** | Secuencias aleatorias de ediciones y deshacer que nunca deben romper invariantes. |
| Importadores robustos | **cargo-fuzz** | Un MusicXML o MIDI malformado nunca debe tumbar la app. |
| Diagnóstico | **tracing** (+ **sentry** opcional) | Logs estructurados e informes de fallos. |
| CI | GitHub Actions | Compilar, `clippy`, tests y benchmarks en cada cambio. |

## 8. Rendimiento medible

| Pieza | Herramienta | Para qué |
|---|---|---|
| Perfilado | **tracy** (vía `tracing-tracy`) | Ver el coste de cada frame y cada fase del layout. |
| Benchmarks | **divan** o **criterion** | Proteger presupuestos fijos en CI. |

Presupuestos propuestos (a confirmar):

- Pulsar una tecla de entrada → nota visible en pantalla: **< 16 ms**.
- Redibujar la partitura visible: **< 8 ms** (120 fps).
- Abrir una partitura orquestal grande: **< 1 s**.
- Inicio de la reproducción: **< 50 ms**.

## 9. Capacidades adicionales

| Pieza | Herramienta | Para qué |
|---|---|---|
| Partes individuales | ★ propio | Extraer partes del score general, con layout propio y vinculadas al original. |
| Plugins / scripting | **mlua** (Lua) o **wasmtime** (WASM) | Equivalente a ManuScript de Sibelius. |
| Idiomas | **fluent** | Interfaz traducible. |
| Preferencias | **directories** + **toml** | Configuración y rutas por sistema. |
| Distribución | **cargo-packager**, firma y notarización (macOS), auto-actualización | Instaladores y actualizaciones. |

---

## Riesgos

1. **GPUI es joven:** su API cambia entre versiones y tiene poca documentación. Fijar la versión
   (como ya hace `Cargo.toml`) y aislar su uso en `partitura` para que el resto no dependa de ella.
2. **El motor de layout** concentra la mayor parte del trabajo real: es lo que separa una
   partitura profesional de una amateur.
3. **Licencias:** MuseScore y LilyPond son GPL. Sirven para estudiar ideas, no para copiar código ni datos.
   Revisar la licencia de cada fuente y SoundFont antes de distribuirlos.
4. **Tiempo real de audio:** cualquier bloqueo o reserva de memoria en el hilo de audio provoca cortes;
   hay que diseñarlo así desde el principio.

## Preguntas abiertas

- Plataformas objetivo: ¿solo macOS, o también Windows y Linux?
- Sonido: ¿sintetizador integrado, MIDI externo o ambos desde la primera versión?
- Alcance de la primera versión: qué capacidades entran primero y cuáles después.

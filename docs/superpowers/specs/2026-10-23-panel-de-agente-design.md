# Panel de agente

Diseño de la primera función de IA de Levare: un panel de conversación con un agente
completo que lee y edita la partitura abierta. La transcripción de PDF queda para una
segunda fase que se apoyará en este panel.

## Intención

Escribir en Levare con teclado es rápido para notas, pero muchas operaciones pesan:
buscar el compás, seleccionar el tiempo justo, encontrar la marca en la cinta. El músico
quiere decirlo como lo diría a un copista:

> «Escribe una ligadura en el 1.er y 3.er tiempo del compás 4, pon un trino en el
> siguiente tiempo, agrega número de ensayo.»

El panel de agente es eso: una conversación al lado de la partitura donde se pide
cualquier cosa que el editor sabe hacer, y el agente la hace en la partitura real.

El objetivo de fondo (fase 2) es pasar un PDF, por ejemplo de IMSLP, parte suelta o
partitura completa, y que el agente lo transcriba en Levare para reproducirlo y
practicarlo. Por eso el panel se diseña como agente general y no como un botón de
«Transcribir PDF»: la transcripción será una petición más en la misma conversación,
con las mismas herramientas, y las correcciones posteriores también.

## Objetivos

- Un panel lateral de conversación con un agente de IA, al estilo del panel de agente de
  Zed.
- El agente es uno de los que el usuario ya tiene instalados, conectado por **ACP**
  (Agent Client Protocol) y usando la suscripción del usuario: **Claude Code, Codex y
  Gemini CLI**, a elegir entre los que estén instalados. Levare no gestiona claves de API.
- El agente puede leer la partitura abierta y ejecutar **cualquier operación del
  editor**: escribir notas y silencios, ligaduras, articulaciones, ornamentos, dinámicas,
  números de ensayo, compases, tonalidad, compás de tiempo, partes, textos, etc.
- El agente entiende posiciones musicales: parte o instrumento, compás, tiempo,
  pentagrama y voz.
- Si el instrumento no se dice y está claro por el contexto (la selección, una sola parte,
  lo último hablado), el agente lo usa; si es ambiguo, pregunta en vez de adivinar.
- Los cambios se ven en directo en la partitura según el agente los hace.
- Todo lo que el agente hace en respuesta a un mensaje se deshace con **un solo ⌘Z**.

## Fuera de esta fase

- **Transcripción de PDF** (fase 2). Notas de lo hablado al final.
- Que un agente externo (Claude Desktop, Claude Code en una terminal) controle Levare
  desde fuera. Las herramientas quedarán preparadas para ello, pero no se expone nada
  fuera de las sesiones que abre el propio Levare.
- Claves de API propias o un proveedor de IA integrado.
- Windows y Linux (Levare solo está validado en macOS).

## Anti-objetivos

El usuario no señaló ninguno. Estos son de mi cosecha y el builder debe respetarlos:

- Que el agente cambie cosas que no se le pidieron, sobre todo **quitar** marcas sin
  querer (ver «Interruptores» más abajo).
- Que una petición pequeña («pon un trino aquí») tarde mucho o consuma mucha suscripción
  porque el agente tiene que leer la partitura entera.
- Que el agente toque el archivo `.partitura` directamente en vez de usar el editor:
  rompería el deshacer, la recuperación y las validaciones.

## Cómo funciona (decidido)

### ACP: Levare es el cliente

Levare lanza el agente elegido como subproceso y habla con él por ACP
(<https://agentclientprotocol.com>). Gemini CLI habla ACP de forma nativa; Claude Code y
Codex lo hacen con los adaptadores que publica Zed. Levare detecta cuáles están
disponibles y deja elegir.

### Las herramientas de Levare llegan por MCP, interno

ACP no permite que el cliente declare herramientas propias: el cliente solo ofrece
sistema de archivos, terminal y peticiones de permiso. Las herramientas a medida llegan al
agente como servidores MCP que el cliente pasa al abrir la sesión (`session/new`,
`mcpServers`). Por tanto, Levare aloja **su propio servidor MCP**, invisible para el
usuario, y lo pasa a cada sesión ACP que abre. No es un producto aparte: es la forma de
declarar las herramientas.

### Las herramientas son el editor, no un editor paralelo

El editor ya funciona con acciones explícitas:

- `score_edit::Command` (`crates/score-edit/src/lib.rs`): unas 80 acciones con
  parámetros (`WriteAt`, `SetDynamic`, `AddConnection`, `SetTimeSignatureAt`,
  `AddParts`, …). Muchas actúan **sobre la selección o el cursor** del `Editor`.
- El registro de comandos con nombre (`crates/keymap/src/commands.rs`, unos 250
  `CommandId` estables con etiqueta en español y categoría), que ejecuta `AppView::run`
  y es lo que disparan atajos, cinta y menús.

El agente debe operar **a través de esas mismas acciones**, como lo haría el usuario:
seleccionar una posición musical y ejecutar la acción. La meta es que una función nueva
del editor quede disponible para el agente sin escribir una herramienta a mano para
ella. Cómo se exponen (una herramienta genérica de «ejecutar comando» con el catálogo
descrito, o herramientas generadas desde el registro) es decisión del builder.

### Interruptores

Muchas acciones son interruptores: `ToggleMark`, `ToggleRehearsalMark` y similares quitan
la marca si ya está. El agente tiene que poder **leer qué hay en una posición** antes de
actuar. Las herramientas deben hacerlo fácil, y lo ideal es que el resultado de cada
acción diga qué cambió («trino añadido», «trino quitado»), para que el agente detecte el
error y lo corrija.

### Lectura compacta y por fragmentos

Las partituras típicas son orquestales, de unas 32 partes (ver la nota de rendimiento del
proyecto). El agente no debe recibir la partitura entera serializada. Hace falta una
lectura por rango (partes × compases) en una notación de texto compacta y legible para
un modelo, más un resumen general barato (partes, número de compases, tonalidades,
compases de tiempo, tempo y selección actual).

### Escritura gruesa

Para que la fase 2 sea viable, debe existir una forma de escribir uno o varios compases
completos de una parte en una sola llamada, en esa notación compacta, y no solo nota a
nota. Levare valida cada compás al recibirlo (duración total, rango, voces) y devuelve al
agente un error claro si algo no cuadra, sin aplicar ese compás.

### Deshacer

El `Editor` guarda instantáneas completas (`Snapshot`) en `undo`. Todo lo que el agente
ejecuta durante la respuesta a **un mensaje** se agrupa en una sola entrada de deshacer.
El usuario puede seguir deshaciendo después con el historial normal.

### En vivo, con la partitura bloqueada

Las ediciones del agente se aplican al `Editor` del documento abierto en el hilo de la
interfaz y la partitura se redibuja según llegan. El usuario puede parar al agente desde
el panel en cualquier momento; lo ya hecho queda, y se deshace de un golpe.

**Decidido por el usuario:** mientras el agente trabaja, la partitura queda bloqueada
para el usuario (edición, deshacer y rehacer). Si quiere tocar algo, primero para al
agente. Reproducir, desplazarse y hacer zoom siguen disponibles. Como nadie más edita
durante el turno, el agrupado en un solo ⌘Z es una única instantánea tomada al empezar
el turno.

Mi decisión: el agente usa el cursor y la selección del `Editor`, así que el usuario los
ve moverse por donde está trabajando. Al terminar el turno (o al pararlo), cursor y
selección vuelven a donde los dejó el usuario, porque esa selección es el contexto de la
siguiente petición.

## Decisiones mías (revisables)

- El panel va en un lateral, como Instrumentos o Propiedades, y se oculta desde la
  cabecera como los demás paneles.
- El agente aplica los cambios directamente, sin pedir confirmación para cada edición: el
  deshacer en un paso hace de red de seguridad.
- Una conversación por documento abierto; al cerrar el documento se cierra la sesión.
  Guardar el historial de conversaciones queda fuera de esta fase.
- Textos de interfaz en español, como el resto de la app.
- **Qué comandos puede ejecutar el agente.** «Cualquier operación del editor» significa
  las categorías que cambian o recorren la música: Edición, Entrada de notas, Duración,
  Alteraciones, Expresión, Notación, Compases, Instrumentos, Navegación y Revisión
  (comentarios). También Reproducción, para que pueda hacer sonar un pasaje si se le pide.
  Quedan **fuera** Archivo (guardar, abrir, nuevo, exportar) y Vista: el agente nunca
  escribe en disco ni cierra el documento, porque eso acabaría con su propia sesión. Un
  comando nuevo de una categoría permitida queda disponible para el agente sin trabajo
  extra.

## Lo que queda para el builder

- Crates de ACP y MCP en Rust, y transporte del servidor MCP interno (stdio con un
  pequeño binario puente, HTTP en `localhost`, o lo que admitan los tres agentes). Debe
  funcionar con los tres.
- Cómo se detectan e instalan los adaptadores de Claude Code y Codex, y qué ve el usuario
  si no tiene ninguno.
- La notación de texto compacta para leer y escribir (puede inspirarse en ABC, LilyPond o
  similar, o ser propia) y cómo se expresan posiciones («tiempo 3 del compás 4») teniendo
  en cuenta compases compuestos, anacrusa, voces, pentagramas de piano y divisi.
- Qué hacer con las capacidades propias del agente (terminal, archivos): qué permisos se
  le conceden en la sesión y cómo se le piden al usuario por el mecanismo de permisos de
  ACP.
- Diseño visual del panel dentro del tema oscuro existente (`crates/app-gpui/src/ui`).
- Estructura de crates (probablemente uno nuevo para ACP/MCP sin dependencia de GPUI, como
  el resto del modelo).

## Restricciones

- Rust 2024, GPUI `=0.2.2`, dependencias con versión fijada como el resto del workspace.
- `cargo fmt`, `cargo test --workspace --locked` y `cargo clippy … -D warnings` deben
  pasar (ver README, «Verificar»).
- No degradar la fluidez con partituras de 32 partes: el trabajo del agente no puede
  bloquear el hilo de la interfaz más allá de aplicar cada edición.
- Vocabulario del dominio según `CONTEXT.md` (parte, altura escrita / real, etc.).

## Fase 2: transcripción de PDF (notas para después)

Lo hablado, para no perderlo; no forma parte de esta fase:

- Se arrastra un PDF al panel y se pide «transcribe esto». El agente mira las páginas y
  escribe directamente con las herramientas, **sin pasar por MusicXML**, compás a compás,
  corrigiendo lo que Levare le rechace.
- PDFs de IMSLP: escaneos antiguos, ruidosos. No se espera perfección; los errores que
  queden se corrigen con el editor o pidiéndoselo al agente.
- Con páginas orquestales densas, el agente debería trabajar con recortes ampliados por
  sistema o grupo de pentagramas.
- Levare ya importa MusicXML (`crates/score-io`), pero con reglas estrictas que rechazan
  la obra entera ante un error; por eso se descartó como camino principal.

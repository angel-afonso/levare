# Levare

Editor de notación musical nativo en Rust y GPUI. Partituras de varios instrumentos
con pentagramas alineados, entrada por teclado, historial, archivos locales y
reproducción conjunta con sonidos integrados.

## Ejecutar

En macOS, con Rust estable y las herramientas de desarrollo de Xcode instaladas:

```sh
cargo run
```

GPUI está fijado a `0.2.2`. Por defecto se usa su opción `runtime_shaders`, que
permite desarrollar sin descargar el componente Metal Toolchain de Xcode.
Para compilar los shaders durante la construcción (requiere ese componente),
desactiva las opciones por defecto y conserva el paquete VSCO:

```sh
cargo build --release --no-default-features --features vsco
```

El icono de Levare se incluye también al ejecutar `cargo run`. Para generar o
actualizar `target/Levare.app` con ese icono en Finder y el Dock:

```sh
python3 scripts/bundle_macos.py
```

Añade `--release` para empaquetar la versión optimizada. La app y su cabecera usan
`assets/branding/levare/icon-macos-v2.png`, con margen transparente y esquinas
adaptadas al Dock. El script prepara los tamaños de macOS y Retina antes de
compilar; `icon-v1.png` conserva el diseño original.

Levare abre los documentos `.partitura` existentes y conserva el portapapeles
musical compatible. Las instalaciones anteriores siguen usando sus atajos,
archivos recientes y recuperación; las nuevas guardan esos datos en `Levare`.

Windows y Linux todavía no están validados. La primera plataforma es macOS;
el modelo, la edición y el layout no dependen de GPUI.

## Interfaz del editor

La interfaz sigue la referencia visual de **Editor · Entrada de notas**: tema
oscuro, cinta de herramientas, instrumentos por familia, hoja clara, inspector y
transporte inferior. La partitura muestra sus datos reales, sin documentos de
muestra ni conexiones MIDI simuladas. Puedes abrir
[`examples/cuarteto-en-sol.partitura`](examples/cuarteto-en-sol.partitura) para
ver el conjunto de cinco instrumentos.

- **Archivo:** nueva, abrir, guardar, guardar como y exportar PDF o MusicXML. Los iconos de la cabecera
  permiten guardar, deshacer y rehacer.
- **Cinta:** ocho pestañas, una por tarea (⌥⌘1–8 las abren).
  **Escribir** reúne lo que se elige con cada nota: entrada de notas y octava,
  duración, puntillo, tresillo y silencio, alteración, unión y ligadura de
  expresión, voz y acordes con intervalos. **Articulaciones**: articulaciones y
  calderón, notas de adorno, arpegios, trémolos y técnica de cuerda. **Dinámicas**:
  de *ppp* a *fff*, reguladores, líneas (8va, pedal, trino, glissando) y divisi.
  **Texto**: título y créditos, tempo, textos sobre el pentagrama, letra y cifrado,
  marcas de ensayo y su estilo. **Partitura**: instrumentos, compases, cambios de
  clave, tonalidad y compás de tiempo, repeticiones, da capo y segno, y saltos de
  sistema y página. **Reproducción**: reproducir, bucle, media velocidad, repeticiones,
  audición, mezclador y sonidos. **Revisión**: comprobar, avisos, registro del
  instrumento y comentarios. **Vista**: modo de página, zoom y plegar la cinta.
  Todos los controles se dibujan igual: sin fondo en reposo, en azul cuando están
  activos o elegidos, y las opciones que se excluyen (duración, alteración, voz) comparten
  una pista. Bajo cada símbolo van su nombre y su tecla, y cada grupo lleva su título
  debajo. En ventanas estrechas los controles pierden el nombre (el principal de cada
  grupo lo conserva) y, si aún falta sitio, los grupos menos usados se pliegan en un
  botón desplegable. A la derecha de las pestañas, una pastilla dice qué se va a
  escribir («Escribiendo · Negra con puntillo · Voz 2»), con un punto azul cuando la
  entrada de notas está activa; sin ella dice «Seleccionando» y la tecla para empezar
  a escribir. Si no cabe, deja primero los modificadores y después se recorta.
  El portapapeles está en el menú **Edición** y en sus atajos; deshacer y rehacer, en
  la cabecera. No hay botones de «Quitar»: selecciona la nota en la partitura y vuelve
  a pulsar la marca activa para quitarla, o pulsa Backspace para borrar la selección.
- **Instrumentos:** selección por familias y manos del piano; el selector de
  tonalidad aparece en este panel. Los botones de la cabecera ocultan los paneles.
- **Propiedades:** nota y duración seleccionadas, elección de nota dentro del
  acorde, alteraciones, octava de entrada, armadura y audición.
- **Mezclador / Sonidos:** volumen general, volumen y posición estéreo por instrumento
  (con varias partes, cada una parte de su sitio habitual en el escenario),
  reverberación suave con cantidad, tamaño de sala y encendido/apagado. Los deslizadores
  responden a clic, arrastre y botones −/+, incluso durante la reproducción.
  Los ajustes son de sesión, se conservan al cambiar de banco de sonidos y no se guardan
  en la partitura. **Intérpretes** elige grabaciones de solista o de sección para cada
  cuerda, madera o metal; esa elección sí se guarda y se puede deshacer
  ([detalles](docs/sfz.md#solista-o-sección)). Sonidos permite elegir banco, añadir/quitar
  parte y reconectar audio, y **Más sonidos** instala paquetes adicionales del catálogo
  con su licencia ([detalles](docs/sfz.md#más-sonidos)).
  Mezclador y Sonidos también están en **Reproducción › Sonido**.
- **Transporte:** reproducir/pausar, detener e iniciar desde el principio sin
  borrar la selección. Reloj y regla siguen la posición musical real.
- **Vista:** pestaña con página, desplazamiento continuo o un solo sistema y zoom entre 50 y 150 %,
  con selección y seguimiento ajustados a la escala. El zoom inicial es 70 %.
- **Agente:** conversación con un agente de IA que edita la partitura abierta; se
  muestra con el icono de la cabecera (ver [Panel de agente](#panel-de-agente)).

Los controles marcados como **Pendiente** (o con borde discontinuo en la cinta)
todavía no funcionan: varias pestañas de documentos,
búsqueda, impresión directa, entrada MIDI/tiempo real, voces 3–4, plicas manuales,
velocidad/duración sonora por nota, silencio/solo y cambio de timbre.
Guardar actualiza el archivo elegido; la recuperación automática conserva además
una copia de la sesión para reabrirla después de un cierre inesperado.

## Panel de agente

El icono del bocadillo de la cabecera, o el menú **Vista › Panel de agente**, abre a la
derecha una conversación con un agente de IA que lee y edita la partitura abierta
con las mismas acciones del editor. Pídele lo que harías a mano, como se lo dirías
a un copista: «pon una ligadura en los tiempos 1 y 3 del compás 4 y un trino en el
siguiente».

- **Agentes:** Claude Code, Codex o Gemini CLI, con tu propia suscripción; Levare no
  usa claves de API. Claude Code y Codex necesitan además Node.js (`npx`); Codex usa
  la versión que trae su adaptador, no la que tengas instalada. El agente se
  elige bajo el campo de texto; en la lista, uno que no está instalado aparece
  apagado y dice qué falta.
  La primera vez, inicia sesión en la terminal (`claude`, `codex login` o `gemini`);
  si no, el panel te lo recuerda.
- **Conversación:** escribe en el campo inferior y pulsa **Enviar** o Enter; Esc
  vuelve a la partitura. Las respuestas y las herramientas que usa el agente
  aparecen en orden. Con Claude Code y Codex, las herramientas de Levare no piden
  permiso; si el agente pide permiso para otra cosa, el panel se abre: elige una de
  sus opciones o **Rechazar**.
  Al cambiar de agente o reiniciarse la sesión, el panel marca la conversación nueva.
- **Mientras trabaja:** la partitura queda bloqueada para editar, deshacer y rehacer,
  pero puedes reproducir, desplazarte y hacer zoom. **Detener** lo para en cualquier
  momento y conserva lo que ya hizo; si el agente no se detiene en unos segundos, o
  pulsas **Detener** otra vez, Levare lo cierra.
- **Deshacer:** todo lo que el agente hace en respuesta a un mensaje se deshace con
  un solo ⌘Z. Al terminar, el cursor y la selección vuelven a donde los dejaste.
- Hay una conversación por documento: abrir otro o cerrar la ventana la termina, y
  cambiar de agente empieza una nueva. Las herramientas de Levare nunca guardan,
  abren ni cierran archivos; si el agente pide usar la terminal o archivos, puedes
  rechazarlo.

### Problemas conocidos

- **Claude Code** necesita una sesión iniciada y válida: si ha caducado, el panel
  pide iniciar sesión; ejecuta `claude` en una terminal y vuelve a intentarlo.
- **Gemini CLI** con una cuenta personal responde ahora que este cliente ya no está
  admitido (Google lo traslada a Antigravity); Levare muestra ese mensaje tal cual.
  Además, Gemini no dice de qué servidor es cada herramienta, así que cada llamada a
  las herramientas de Levare muestra una tarjeta de permiso.
- Si reproduces durante un turno, suena la partitura tal como estaba al empezar la
  reproducción, sin los cambios que el agente haga después.

## Uso

Los atajos imitan el perfil **Notebook (portátil)** de Sibelius y funcionan igual
con o sin bloque numérico: la fila de números hace de teclado numérico. La lista
completa, generada desde el código, está en [docs/atajos.md](docs/atajos.md).
Puedes cambiarlos en **Levare › Preferencias de atajos…** (`⌘,`) o con el botón
**Atajos** de la cabecera.

La barra y el inspector muestran el nombre y el atajo actual de cada control al
mantener el cursor encima. Los menús y la ventana **Atajos** conservan las teclas
visibles como referencia, sin repetirlas permanentemente junto a los botones.

| Tecla / acción | Resultado |
|---|---|
| `N` | Activar / desactivar la entrada de notas |
| `⌥1` / `⌥2` | Seleccionar la voz 1 / 2 del pentagrama activo |
| `A`–`G` | Escribir La–Sol con la octava y duración actuales |
| `⇧A`–`⇧G` | Añadir esa nota encima del acorde seleccionado |
| `⇧2`–`⇧9` | Añadir una segunda … novena encima (según la armadura) |
| `1` `2` `3` `4` `5` `6` | Fusa, semicorchea, corchea, negra, blanca, redonda |
| `7` / `8` / `9` | Becuadro / sostenido / bemol; repetirlo lo quita |
| `0` | Silencio con la duración actual |
| `↩` | Ligadura de unión con la siguiente nota o acorde |
| `S` | Inicio y después final de una ligadura de expresión |
| `R` | Repetir la nota, acorde o silencio seleccionado |
| `↑` / `↓` | Subir / bajar un grado |
| `⌘↑` / `⌘↓` | Subir / bajar una octava |
| `⌥↑` / `⌥↓` | Nota superior / inferior del acorde y, al final, el pentagrama vecino |
| `←` / `→`, `⇥` / `⇧⇥` | Elemento anterior / siguiente |
| `⌫` / `⌦` | Eliminar el elemento seleccionado |
| `Espacio` / `P` | Reproducir, pausar o continuar / reproducir desde la selección |
| `Esc` | Detener; si ya está detenido, quitar la selección |
| `⌘B` | Añadir un compás al final |
| `I` / `K` / `M` | Instrumentos / tonalidad / mezclador |
| `T` | Cambiar la métrica desde el inicio o el compás seleccionado |
| `Q` | Cambiar la clave del instrumento o pentagrama activo |
| `⌥A` / `⌥⌫` | Modo acorde / quitar la nota más aguda (propios de Levare) |
| `⌘Z` / `⌘Y` (o `⇧⌘Z`) | Deshacer / rehacer |
| `⌘+` / `⌘−` / `⌘1` | Acercar / alejar / 100 % |
| Clic en una nota | Seleccionarla; la entrada continúa después de ella |
| Clic en un compás vacío | Mover el punto de entrada allí |

Las teclas de Sibelius sin función todavía (`L`
líneas, `⌥3`/`⌥4` voces adicionales, `1` fusa…)
están reservadas y muestran «Pendiente». Las articulaciones no tienen atajo por
defecto, como en Sibelius portátil; asígnalas en Preferencias si las usas a menudo.

Un compás lleno avanza al siguiente al introducir otra figura. Si no existe,
se crea. Una figura que exceda la duración restante se rechaza sin alterar el
documento. Cambiar la duración afecta a la próxima entrada. Puedes abrir
[`examples/primera-melodia.partitura`](examples/primera-melodia.partitura) como ejemplo.

## Notas fuera del rango instrumental

Las notas fuera del registro habitual del instrumento aparecen **en rojo** en
la partitura. En un acorde se marca cada nota por separado. Al seleccionarla,
**Propiedades** muestra el aviso y los límites del registro en altura real
(C4 = do central). Se consideran la armadura, las alteraciones, la transposición
de la parte y los armónicos de cuerda. El color se actualiza también al arrastrar
una nota, editarla, deshacer o rehacer.

El aviso es orientativo: permite seguir escribiendo y reproduciendo la nota.
La percusión sin altura no lleva este aviso. Los colores de aviso son propios
del editor y no aparecen en PDF ni MusicXML. Los registros son valores
profesionales de referencia para modelos habituales; todavía no se personalizan
por nivel de dificultad. Puedes ajustar los límites por parte desde Revisión.
Las convenciones y fuentes están en
[Rangos instrumentales](docs/instrument-ranges.md).

## Revisión de la partitura

**Revisión › Comprobar › Toda la partitura** comprueba toda la obra; **Selección** revisa
los eventos del pasaje seleccionado y las marcas de forma de sus compases.
Busca notas fuera de rango, inicios de repetición sin cierre y saltos que carecen
de Segno, Fine o To Coda; también comprueba que To Coda tenga un destino posterior
y un salto que lo active. Los compases vacíos y los silencios implícitos son válidos.

**Revisión › Avisos › Mostrar** abre el panel de Revisión. Cada fila explica el aviso
y su instrumento y compás; al pulsarla se selecciona la nota exacta del acorde o
se muestra el compás de la marca. **Anterior / Siguiente** recorren los resultados
del filtro, volviendo al principio o al final. Puedes filtrar por instrumento;
los avisos de repeticiones y saltos son globales. Las listas largas tienen páginas.
La revisión completa se actualiza al editar o deshacer. Una revisión de selección
pide comprobar de nuevo después de editar, para no atribuir avisos a un pasaje
que haya cambiado.

**Revisión › Registro del instrumento › Mostrar** muestra u oculta el color de aviso sin desactivar la
comprobación. **Editar…** ajusta los límites de la parte activa en altura
real; admite nombres como `C4`, `Do4`, `F♯3` o `Si♭5`. Los límites son inclusivos,
se aplican también al sonido de sus armónicos y se guardan con el documento.
**Restablecer** recupera el registro habitual. Aplicar y restablecer admiten
Deshacer/Rehacer. La percusión sin altura no requiere límites. El color y los
resultados de revisión son ajustes de sesión y no se exportan.

## Comentarios de revisión

Selecciona una nota, acorde, silencio o pasaje y pulsa **Revisión › Comentarios ›
Nuevo**. Escribe la observación y pulsa **Guardar** o Enter; Esc
cancela el borrador. El vínculo queda fijado al abrirlo, aunque después cambies
la selección. El campo admite una observación de hasta 2000 caracteres. Hay un
límite de espacio acumulado para comentarios y sus vínculos; si se alcanza, el
borrador conserva el texto para reducir el pasaje o liberar espacio en la lista.

**Revisión › Comentarios › Mostrar** abre la lista con filtros **Todos**, **Pendientes** y
**Resueltos**. Al pulsar una observación se selecciona su música y se muestra el
inicio del pasaje, también en otra voz o mano del piano. **Resolver** la marca
como terminada; **Reabrir** vuelve a dejarla pendiente. También puedes editar el
texto o eliminar el comentario. Las listas largas tienen páginas.

Los comentarios se guardan en el archivo nativo y en la recuperación automática;
crear, editar, resolver, reabrir y eliminar admiten Deshacer/Rehacer. Acompañan a
sus eventos al mover notas, cambiar el divisi o insertar y eliminar compases.
Si se borra toda su música, la observación se conserva **sin vínculo musical**:
puedes editarla y usar **Usar selección actual** para asociarla a otra música.
Si cambia la partitura mientras escribes un borrador, actualiza su vínculo antes
de guardarlo; el texto se conserva. Las observaciones son del documento y no se
duplican al copiar música ni se incluyen en PDF o MusicXML. Cambiar su texto o
estado conserva los avisos de comprobación y no modifica la reproducción.

## Cambios de clave

En **Partitura › Cambios › Clave…** (`Q`) puedes elegir sol en segunda, fa en cuarta,
do en tercera o do en cuarta para el instrumento activo. Elige el inicio de la
pieza o el compás actual: la clave se mantiene hasta el siguiente cambio.
En piano y otros instrumentos de dos pentagramas, cada pentagrama conserva
su propia clave; las ramas de una sección en divisi comparten la clave del instrumento.
Las notas conservan sus alturas y su sonido. Puedes restablecer la clave anterior,
deshacer y rehacer; los cambios se guardan y se conservan en PDF y MusicXML.
Los cambios se colocan al comienzo de un compás.

## Copiar, cortar y pegar música

- **Shift + arrastre sobre las notas, o arrastre desde el pentagrama vacío**, para seleccionar un pasaje completo,
  hacia delante o hacia atrás, incluso entre compases y renglones. La franja azul
  indica el pasaje y la barra superior muestra cuántos elementos contiene, con
  botones **Copiar** y **Cortar**. **Shift + arrastre** selecciona un pasaje en cualquier
  dirección. Un clic selecciona una nota y sitúa la reproducción en su entrada.

- **Arrastra una cabeza de nota hacia arriba o abajo** para cambiar su altura.
  El grabado muestra una vista previa mientras mueves el ratón; al soltar se aplica
  un solo cambio que puedes deshacer con **⌘Z**. **Esc** cancela. En un acorde se
  mueve únicamente la cabeza elegida. Se conservan duración, voz y posición
  rítmica. Los silencios
  conservan el gesto de selección de pasajes. Si estás reproduciendo, empezar a
  cambiar una altura detiene el audio; al soltar se escucha la nota editada si la
  audición está activada.
- **Arrastra una nota horizontalmente** para mover la figura completa (todo el
  acorde si tiene varias cabezas) a otro tiempo o compás, dentro de su voz y
  pentagrama. Una copia azul indica el destino; al soltar se aplica una sola
  edición. **Esc** cancela y **⌘Z** deshace. Se conserva la duración y quedan
  silencios en el origen, sin desplazar las demás notas. El destino se ajusta
  a semicorcheas (a fusas si la figura es una fusa), sus tresillos para figuras de tresillo, o entradas
  existentes. Si hay una nota/acorde de igual duración, las figuras se
  intercambian. Los solapamientos incompatibles o una figura que no cabe se
  rechazan con explicación, sin modificar la obra. Al atravesar una armadura
  se añaden las alteraciones necesarias para conservar las alturas sonoras.
  Las ligaduras incompatibles con los nuevos tiempos se retiran y se recuperan
  al deshacer; un arpegio entre manos separadas queda local donde hay un acorde.
- **Shift + clic** o **Shift + ←/→** amplía la selección dentro del mismo
  pentagrama; **⌘A** selecciona todos sus elementos escritos.
- **⌘C** copia, **⌘X** corta y **⌘V** pega. También están disponibles en el
  menú **Edición**, con los atajos configurables.
- El pegado se coloca **después de la selección**, o en el cursor del compás
  vacío que elijas. Puedes cambiar de mano, instrumento o documento antes de pegar.
- Se conservan acordes, silencios, duración, alteraciones, dinámicas, articulaciones
  y las ligaduras cuyos dos extremos estén en el pasaje. Las alturas sonoras se
  mantienen al pegar en otra tonalidad; la dinámica vigente acompaña al comienzo.
- Se conserva la separación temporal entre compases, completando huecos implícitos
  con silencios. Si una figura cruza una barra, se divide y se liga; se crean compases
  alineados para todos los instrumentos cuando hacen falta.

Pegar no reemplaza música existente: si el pasaje no cabe en los compases de destino,
se muestra el motivo y el documento queda intacto. Elige un compás vacío o un punto
con espacio suficiente. Cada pegado o corte se deshace en un solo paso.

El portapapeles musical es del sistema y funciona entre ventanas de Levare.
El texto corriente o las imágenes de otras aplicaciones no se interpretan como notas.
La selección de pasajes abarca un pentagrama cada vez; puedes copiar cada mano del
piano por separado. Las ligaduras con extremos fuera del pasaje no se copian.

## Puntillos y tresillos

En **Escribir › Duración** elige la figura y en **Escribir › Modificar** activa
**Puntillo** o **Tresillo**. Atajos por defecto: **`.`** y **⌘3**, configurables en Preferencias.
También están en el inspector y en el menú **Edición**. Los modos quedan activos
para las siguientes notas y silencios hasta volver a pulsarlos; cambiar la figura
base conserva los modos.

Con una nota o acorde seleccionado se cambia su duración; con un pasaje seleccionado
se aplica a todas sus figuras en una sola operación. El puntillo añade la mitad
(por ejemplo, negra con puntillo = negra + corchea). El tresillo usa proporción
**3:2**: tres corcheas de tresillo duran exactamente una negra. Puedes combinar
puntillo y tresillo. La reproducción, las ligaduras, los reguladores y las dos manos
del piano usan estas duraciones exactas.

El grabado coloca los puntos fuera de las líneas y agrupa hasta tres figuras de
tresillo consecutivas e iguales con un **3**, sobre la barra de unión cuando el
grupo está completo o con un corchete en los demás casos. Los grupos incompletos
siguen visibles mientras escribes; un cambio de figura o un nuevo compás comienza
otro grupo. Esta entrega cubre tresillos de figuras iguales, sin grupos anidados
ni otros números de división.

Los cambios se guardan, admiten deshacer/rehacer y se conservan al copiar y pegar.
Si una edición aumenta la duración por encima de lo que cabe en el compás, se
rechaza completa. Al pegar se conservan las figuras originales que caben; las que
cruzan una barra se dividen con ligaduras cuando el destino permite escribir la
división. Si no se puede representar con las figuras disponibles, elige un compás
vacío o un destino alineado con el pasaje. Ejemplo:
[`examples/ritmos.partitura`](examples/ritmos.partitura).

## Barras de unión automáticas

Las corcheas, semicorcheas y fusas se unen automáticamente por pulso, también en acordes
y en cada mano del piano por separado. Las semicorcheas usan una segunda barra y las
fusas una tercera, con la plica medio espacio más larga;
los ritmos mixtos añaden barras parciales. Los silencios, las barras de compás y
los límites de cada tresillo separan los grupos. Una nota aislada conserva su
corchete individual.

En 4/4 se agrupan por negras; en 2/2, por blancas; en 6/8, 9/8 y 12/8, por grupos
de tres corcheas. Los valores predeterminados para 5/8 y 7/8 son **3+2** y **2+2+3**.
Los tresillos completos de corcheas o semicorcheas muestran solo el **3** junto a
su barra; los grupos incompletos o con silencios conservan el corchete.

Se recalculan al escribir, editar, pegar o deshacer, también al abrir partituras
anteriores. No cambian las duraciones ni la reproducción. Ejemplo:
[`examples/barras.partitura`](examples/barras.partitura).

## Ligaduras, dinámicas y articulaciones

La cinta (**Escribir** para uniones y ligaduras de expresión, **Articulaciones** para
articulaciones, arpegios y técnica, **Dinámicas** para dinámicas y reguladores), el inspector **Expresión** y el menú nativo **Expresión**
permiten editar estas marcas en notas y acordes de cualquier pentagrama. Todos los
cambios se guardan y admiten deshacer/rehacer. Ejemplo:
[`examples/piano-expresion.partitura`](examples/piano-expresion.partitura).

- **Unión (`↩`):** selecciona la primera de dos notas o acordes iguales y consecutivos.
  Se prolonga el sonido sin un nuevo ataque, también entre compases. Los acordes se
  ligan completos; los sonidos deben coincidir en todas sus alturas. Volver a pulsar
  `↩` quita solo la unión con la siguiente. Las cadenas pueden continuar varias notas.
- **Expresión (`S`):** selecciona un pasaje y pulsa `S`: la frase va de su primera a
  su última nota; repetirlo sobre el mismo pasaje la quita. También puedes
  seleccionar el inicio, pulsar `S`, seleccionar la nota final del mismo pentagrama y
  pulsar `S` otra vez. Esc cancela la operación. Pulsar `S` con la primera
  nota de una frase seleccionada la quita. Hay más formas de dibujarla en
  [Dibujar ligaduras y reguladores](#dibujar-ligaduras-y-reguladores). La frase se dibuja con
  una curva, incluso entre sistemas. Los extremos pertenecen a la misma voz;
  pueden coincidir frases de voces diferentes, pero no solaparse dos de una misma voz. En la reproducción, las notas de la frase se unen con un breve
  solapamiento (legato); la última se separa como de costumbre y un staccato dentro
  de la frase suena portato (75 %).
- **Dinámicas:** ppp, pp, p, mp, mf, f, ff y fff. Se aplican desde la posición marcada
  hasta la siguiente indicación del mismo pentagrama. Cada mano del piano mantiene
  su propia dinámica. Sin indicación se usa mf (velocidad MIDI 88).
  Se puede colocar una dinámica en un silencio para preparar la entrada siguiente.
  Volver a pulsar la dinámica activa de la nota la quita.
- **Articulaciones:** sin indicación, cada nota suena al 92 % (con un hueco máximo de
  60 ms) para separarse de la siguiente; staccato acorta el sonido al 50 %; tenuto
  conserva la duración completa; acento y marcato refuerzan el ataque (+16/+24 de
  velocidad, hasta 127).
  Afectan a todas las notas del acorde, sin cambiar su duración escrita. En una cadena
  ligada se conserva el ataque inicial; un staccato final acorta al 50 % la duración
  sonora conjunta. Esto conserva el sonido al dividir una figura durante el pegado.

- **Arpegios:** selecciona un acorde y elige **Articulaciones › Arpegio** (también en el inspector y el menú **Expresión**).
  Hay indicación normal, ascendente y descendente. La línea de Bravura se ajusta
  a la altura del acorde y queda a la izquierda de sus alteraciones. Normal y
  ascendente suenan de grave a agudo; descendente, de agudo a grave. Las notas se
  escalonan sin alargar el compás; una cadena ligada conserva un solo ataque.
  **Ambas manos** une las notas que empiezan simultáneamente en los dos
  pentagramas del mismo piano. Selecciona una nota o acorde en cualquiera de las
  manos y activa esa opción; puede haber una sola nota en cada mano. La línea
  atraviesa ambos pentagramas y todas las notas forman un único arpegio. Cambiar
  la dirección desde cualquiera de las manos actualiza la marca compartida.
  Desactivar **Ambas manos** deja un arpegio local en el acorde seleccionado;
  si es una nota sola, retira la indicación.
  Volver a pulsar la misma opción de dirección quita el arpegio. Si el acorde queda con una
  sola nota, se retira la indicación; deshacer la recupera. Al pegar una figura
  dividida entre compases, el arpegio queda en el primer fragmento.
  Ejemplo: [`examples/piano-arpegios.partitura`](examples/piano-arpegios.partitura).

- **Calderón:** en **Articulaciones › Articulación**, el inspector o el menú
  **Expresión**. Va sobre notas o silencios, junto a cualquier articulación. Al
  reproducir, ese momento dura el doble en todas las partes y la música sigue
  después; la regla y el reloj lo tienen en cuenta.

- **Notas de adorno:** en **Articulaciones › Nota de adorno**, selecciona la nota y elige
  **Acciaccatura** (pequeña, tachada, casi sin tiempo) o **Appoggiatura** (se apoya en
  la nota y se queda con la mitad de su valor). La nota de adorno empieza en la altura
  de la nota; `;` añade otra, hasta cuatro, que se escriben unidas con dos barras. La
  nueva queda seleccionada: **↑/↓** cambian su altura (**⌘↑/⌘↓**, de octava) y
  **Backspace** la borra. Un clic en cualquiera de ellas la selecciona sola. No ocupan
  tiempo del compás, suenan dentro del tiempo de su nota, se reservan su sitio a la
  izquierda y se exportan e importan en MusicXML.

**Editar marcas en la partitura:** un clic sobre un arpegio, articulación, calderón, dinámica,
técnica, ligadura, regulador o línea selecciona solo esa marca (se resalta en azul; la nota no).
**Backspace** la borra sin tocar la nota. Pulsar otra marca de la misma familia en la
cinta la cambia (por ejemplo, staccato por acento, *p* por *f*, o crescendo por
diminuendo), y pulsar la misma la quita. Esc vuelve a la nota; con una marca
seleccionada, las flechas no transportan la nota. Un clic sobre la cabeza de la nota
sigue seleccionando la nota.

**Mover dinámicas, técnicas, calderones y reguladores:** su posición en el tiempo
decide cómo suenan, así que al arrastrarlos hacia los lados saltan a la nota o silencio
más cercano de su voz y, al soltar, suenan desde ahí. Un regulador se mueve entero y
conserva su duración. Mientras arrastras, una línea discontinua une la marca con la
nota o las notas a las que pertenece. En vertical solo cambia la altura. Con `⌘`
pulsado se mueven sin cambiar de nota, como las demás marcas. Con la marca
seleccionada, `⌥←` y `⌥→` la llevan a la nota anterior o siguiente. Si en el destino
ya hay una marca de la misma familia, la barra de estado lo indica y al soltar no
cambia nada.

Las articulaciones, dinámicas, técnicas y alteraciones activas aparecen resaltadas
en la cinta; volver a pulsarlas las quita. El inspector permite además quitar
indicaciones y las ligaduras que tocan la nota elegida.
Si una edición de alturas, una inserción o un borrado invalida una unión, se retira
esa ligadura; deshacer recupera tanto las notas como sus marcas. El inicio de
reproducción desde una selección recupera la dinámica vigente y las notas ligadas.

## Dibujar ligaduras y reguladores

Las ligaduras de expresión y los reguladores se escriben de cuatro maneras:

- **Sobre un pasaje:** selecciónalo con Shift+clic, con Shift+flechas o arrastrando
  desde un hueco del pentagrama (arrastrar una nota la mueve), y pulsa `S` (ligadura), `H` (crescendo) o `⇧H` (diminuendo). Si el
  pasaje empieza o acaba en silencios, la ligadura se ajusta a sus notas.
- **Mientras escribes:** con la entrada de notas activa, pulsa `S` o `H` sobre la nota
  recién escrita. La marca se alarga con cada nota nueva y en un silencio espera a la
  siguiente nota. Pulsa la misma tecla otra vez, o Esc, para terminarla. Deshacer la
  última nota devuelve la marca a su final anterior.
- **Con ⌥ y el ratón:** mantén `⌥` y arrastra desde una nota hasta otra de la misma voz
  para una ligadura; `⌥⌘` dibuja un crescendo y `⌥⇧⌘` un diminuendo. Mientras arrastras,
  la partitura muestra el resultado; si el final no es válido, la barra de estado
  explica por qué y al soltar no cambia nada.
- **Estirar una existente:** haz clic sobre una ligadura o un regulador; aparecen dos
  asas redondas en sus extremos. Arrastra un asa hasta otra nota para alargarla o
  acortarla. Arrastrar el resto de un regulador lo lleva entero a otras notas; en una
  ligadura, la desplaza. Se conserva el desplazamiento manual que tuviera.

Todas admiten Deshacer/Rehacer y Esc cancela un arrastre en curso.

## Crescendo y diminuendo con reguladores

Selecciona un pasaje con **Shift+clic** o **Shift+flechas** y pulsa **H** para un
crescendo (`<`) o **Shift+H** para un diminuendo (`>`). También puedes seleccionar
la primera nota, pulsar el regulador, seleccionar la última y volver a pulsar el
mismo botón o atajo. **Esc** cancela el inicio pendiente. Las acciones están en
**Dinámicas › Regulador**, el inspector **Expresión** y el menú **Expresión**.
Los atajos se pueden cambiar en Preferencias.

El regulador va desde el comienzo de la primera nota hasta el comienzo de la
última. Puede atravesar compases, renglones y silencios. Las dinámicas escritas
fijan los niveles de paso y llegada: por ejemplo, **p → f**. Si no escribes una
al final, sube o baja dos niveles desde la última dinámica del regulador, dentro
de los límites ppp–fff. El nivel alcanzado permanece hasta la siguiente indicación.
Las marcas explícitas tienen prioridad sobre la dirección del símbolo.

El volumen cambia mientras suenan las notas y los acordes, incluidas las cadenas
ligadas, con pausa/continuación e inicio desde selección. Cada instrumento y cada
mano del piano mantiene su propia curva. Añadir un regulador conserva el volumen
y el timbre de los pasajes que quedan fuera de él.

Para eliminarlo, selecciona una nota dentro del regulador (salvo la final, desde
la que puede empezar otro) y vuelve a pulsar **Crescendo** o **Diminuendo**. Se guarda con la partitura y admite
deshacer/rehacer. Copiar un pasaje conserva los reguladores cuyos dos extremos
están seleccionados; los que salen del pasaje se omiten. Al copiar desde dentro de
uno de estos últimos, la dinámica inicial se aproxima al nivel escrito más cercano.
No se admiten reguladores solapados en el mismo pentagrama; sí pueden compartir
un extremo. Ejemplo: [`examples/reguladores.partitura`](examples/reguladores.partitura).

## Líneas: 8va, 8vb, pedal, trino y glissando

En la cinta **Dinámicas › Líneas**, selecciona un pasaje (o una sola nota) y elige la
línea. Se dibuja desde la primera nota hasta el final de la última, se corta en los
cambios de renglón y cada tipo ocupa su propio carril, por fuera de las notas, las
ligaduras y las dinámicas.

- **8va / 8vb:** la escritura no cambia, pero el pasaje suena una octava más aguda o
  más grave. Línea discontinua con gancho al final; puede empezar o acabar en un silencio.
- **Pedal:** «Ped.» y una línea hasta que se suelta. Las notas siguen sonando hasta el
  final de la línea; puede cubrir silencios.
- **Trino:** «tr» sobre la nota. En un pasaje o en una nota de blanca o más larga se
  añade la línea ondulada. Suena alternando con la nota superior de la tonalidad,
  también bajo una 8va. Solo sobre notas.
- **Glissando:** con una nota seleccionada va hasta la siguiente; con un pasaje, de la
  primera a la última. Suena por semitonos, o por teclas blancas en teclados y arpa.

Para quitar una línea, haz clic sobre ella y pulsa **Backspace**, o selecciona una nota
de la línea y vuelve a pulsar su botón. Dos líneas del mismo tipo no se solapan en un
pentagrama (8va y 8vb cuentan como un solo tipo). Se guardan con la partitura, admiten
deshacer/rehacer y se exportan e importan en MusicXML (`octave-shift`, `pedal`,
`trill-mark` con `wavy-line` y `glissando`).

## Textos de expresión y técnica

Selecciona una nota o un silencio y pulsa **⌘E** (texto de expresión) o **⌘T** (texto de
técnica), o usa **Texto › Sobre el pentagrama** en la cinta. Se abre un panel con el campo y las
indicaciones habituales; **Enter** aplica, **Esc** cancela y un campo vacío quita el texto.
Caben unos 40 caracteres.

- **Expresión** («dolce», «espress.», «cresc.»): cursiva bajo el pentagrama, en la línea de
  las dinámicas y a continuación de la dinámica de la misma nota («*p* dolce»). Los
  reguladores le dejan sitio.
- **Técnica** («con sord.», «sul pont.»): sobre el pentagrama, en el carril de pizz. y arco.

Algunas palabras también suenan: «cresc.» y «dim.» cambian el volumen como un regulador
hasta la siguiente dinámica (o durante dos compases si no hay ninguna); «con sord.»
suaviza y oscurece el sonido hasta «senza sord.»; «legato» liga las notas hasta
«non legato»; «ord.» devuelve el modo ordinario. En cuerdas, escribir «pizz.» o «arco»
pone la técnica de verdad. Las demás palabras solo se escriben.

Un clic sobre el texto lo selecciona: **Backspace** lo borra, arrastrarlo lo lleva a otra
nota del pentagrama y volver a pulsar ⌘E o ⌘T lo edita. Los textos se copian con sus notas,
admiten deshacer/rehacer y se exportan e importan en MusicXML como `words` (debajo y en
cursiva la expresión, encima la técnica).

## Letra

Selecciona la primera nota y pulsa **⌘L** (o **Texto › Letra y cifrado › Letra**). Escribe la
sílaba y sigue con:

- **Espacio**: la palabra termina; pasa a la nota siguiente.
- **Guion (-)**: la palabra sigue; se dibuja un guion hasta la próxima sílaba.
- **Guion bajo (_)**: la sílaba se mantiene en las notas siguientes, con una línea de
  extensión hasta la última nota antes de la próxima sílaba.
- **Enter** aplica y cierra; **Esc** cierra.

La entrada salta los silencios y las notas de llegada de una ligadura de unión. Los botones
**Estrofa 1–4** cambian de verso; un clic en una sílaba la selecciona (Backspace la borra) y
⌘L la abre en su estrofa. Cada estrofa ocupa una línea a lo largo del renglón, bajo las notas
y las ligaduras; las dinámicas bajan bajo la letra. Una sílaba admite unos 20 caracteres.
La letra no se pone en silencios, se copia con sus notas y se exporta e importa en
MusicXML; las sinalefas importadas se escriben con un espacio.

## Cifrado de acordes

Selecciona una nota o silencio y pulsa **⌘K**. Escribe el acorde en cifrado americano
(**C**, **F#m7**, **Bb7/D**, **Gsus4**, **Cmaj7**, **Dø**, **G7(b9)**…) o con los nombres
del solfeo (**Do**, **Fa#m7**, **Sib7/Re**, **Lam**); se escribe siempre con letras y con
bemoles y sostenidos musicales. El panel propone los acordes de la tonalidad del compás.
**Tab** aplica y pasa a la nota siguiente, **Enter** aplica y cierra, un campo vacío quita el
cifrado. Los cifrados van sobre el pentagrama, en una línea propia por encima de las técnicas;
se arrastran a otra nota y se borran con Backspace como las demás marcas.

Al reproducir, cada cifrado suena como un acompañamiento suave hasta el siguiente cifrado
del pentagrama: en teclados, guitarra y arpa con su propio sonido; en las demás partes, con
un piano. Se escribe como lo lee el músico, así que en un instrumento transpositor suena
transportado como sus notas. **Reproducción › Sonido › Cifrados** lo silencia; la
elección se guarda con la partitura. Al transportar notas, su cifrado se transporta con ellas.

## Pizzicato y arco

Selecciona una nota, acorde o silencio de **violín, viola, violoncello o contrabajo**.
En la cinta **Articulaciones › Técnica de cuerda**, el inspector **Expresión** o el menú
**Expresión**, elige **pizz.** para tocar pulsando la cuerda y **arco** para volver al
sonido habitual. La indicación aparece en cursiva sobre el pentagrama y se mantiene
hasta la siguiente marca del mismo instrumento, también al reproducir desde una
selección. Una marca en un silencio prepara la entrada siguiente.

Volver a pulsar la técnica activa de la nota elegida quita la marca; vuelve a regir
la anterior.
Los cambios se guardan, se pueden deshacer y se conservan al copiar pasajes, incluso
si el pasaje empieza después de la marca original. Los pasajes con técnicas de arco
se pegan en instrumentos compatibles. En una nota ligada, el timbre del ataque se
mantiene hasta terminar la cadena; el cambio afecta al siguiente ataque.
Puedes asignar atajos en Preferencias. Ejemplo:
[`examples/cuerdas-pizzicato.partitura`](examples/cuerdas-pizzicato.partitura).

## Armónicos de cuerda

Selecciona una nota o un pasaje y usa **Expresión › Armónico natural** o
**Armónico artificial**, también disponibles en la cinta (**Articulaciones › Técnica de cuerda**) y en Propiedades.
Pulsa de nuevo la misma opción para quitarla; también puedes seleccionar el
círculo o el rombo y borrar únicamente esa marca. Se conserva el historial de
Deshacer, el portapapeles, el autoguardado y la exportación a PDF.

- **Natural:** escribe la altura que debe sonar. Se añade el círculo de Bravura.
- **Artificial:** escribe una sola nota pisada; el programa añade el rombo a una
  cuarta justa por encima y reproduce únicamente el sonido dos octavas más alto.
  Admite notas pisadas de C0 a B6 para mantener el sonido dentro del rango del editor.

Son marcas de cada evento, compatibles con arco, pizzicato, articulaciones,
ligaduras, dos voces y divisi. En un pasaje se aplican a todas las notas
seleccionadas en una sola operación; los silencios no admiten armónicos.
La reproducción respeta la altura sonora y usa un timbre sintetizado propio para
los armónicos: sostenido y aéreo al arco, con ataque breve y resonancia que decae
en pizzicato. Funciona también al escuchar notas y al continuar desde una
selección, tanto con GeneralUser como con VSCO CE. Es una aproximación por síntesis;
no son grabaciones de armónicos de una biblioteca orquestal.
Ejemplo: `examples/armonicos-cuerdas.partitura`.

Para comparar a la misma altura y dinámica el arco normal, el armónico al arco,
el pizzicato normal y el armónico pizzicato:

```sh
cargo run --offline --locked -p score-platform --example harmonics_compare
```

Genera `output/audio/comparacion-armonicos.wav` con el mismo motor de la app.

## Partituras con varios instrumentos

Pulsa **+ Añadir instrumentos** en el panel izquierdo para abrir el diálogo.
Marca varios instrumentos y ajusta sus cantidades con **− / +**; **Añadir** incorpora
el conjunto en una sola operación que puedes deshacer. **Cancelar** o **Esc** cierra
sin cambiar la partitura. También puedes usar el menú **Instrumentos**. El catálogo incluye **54 perfiles** de maderas,
metales, percusión, teclados, voces y cuerdas; admite hasta **48 partes**, con instrumentos repetidos.


Selecciona una parte en el panel o pulsa su pentagrama para escribir en ella.
**⌥↑ / ⌥↓** pasa al pentagrama vecino (y, en un acorde, primero recorre sus notas). Cada parte conserva sus notas; los compases
se añaden a todos los instrumentos y los tiempos coincidentes quedan alineados.
La **papelera de cada fila** elimina ese instrumento directamente con sus pentagramas,
sin seleccionarlo antes. **Deshacer** recupera sus notas, dinámicas y ligaduras.
El menú **Instrumentos** permite eliminar la parte activa.
La partitura conserva al menos un instrumento. Si eliminas la parte activa, queda
seleccionado el instrumento vecino; al eliminar otra, se conserva tu selección.

Al abrir una partitura o añadir instrumentos, el score y el panel se ordenan
por familias: maderas, metales, percusión, teclados, cuerdas pulsadas, voces y cuerdas. Dentro de
cada familia se mantiene el orden del catálogo orquestal (flauta–oboe–clarinete–fagot,
corno–trompeta–trombón, violín–viola–violoncello–contrabajo). Los instrumentos
repetidos conservan su orden relativo y sus notas; los IDs no cambian.

Cada renglón presenta **corchetes de familia** y barras de compás unidas dentro
de cada grupo, con separación entre familias. El piano conserva su **llave de dos
manos**. La barra inicial fina conecta todo el sistema. El orden se guarda en el
archivo nativo y no requiere convertir las partituras anteriores.

Cada parte usa su timbre y su clave de sol, do, fa o percusión. Las nuevas partes
transpositoras se escriben en **altura escrita** y suenan en la altura real correspondiente.
La tonalidad general se elige en altura real: en una obra en Do mayor, el clarinete en
Si♭ muestra Re mayor y su Do natural escrito suena Si♭. Se adaptan también los
cambios de tonalidad durante la pieza, las audiciones, el copiado entre partes y las
exportaciones de score y partes a PDF y MusicXML.

Hay clarinetes en Do, Si♭ y La, clarinete en Mi♭, clarinete bajo, corno inglés,
corno en Fa, trompetas en Do, Si♭ y Re, corneta y saxofones; se incluyen las
transposiciones de octava de piccolo, contrafagot, contrabajo, guitarra, celesta,
xilófono y glockenspiel. Arpa, piano, órgano, celesta, marimba y vibráfono usan dos pentagramas.
Los scores de 12 partes o más se exportan a PDF en A3; las partes individuales siguen en A4.
Las voces son **Voz** (solista), **Soprano**, **Contralto**, **Tenor** y **Bajo**; el tenor se
escribe en clave de sol una octava por encima de lo que suena. Ningún paquete de sonidos
incluye voces, así que suenan con el coro del banco General MIDI integrado.
Caja, bombo, platillos, triángulo, pandereta y tam-tam tienen clave de percusión
sin armadura y un golpe por voz. Sus sonidos se aproximan con el banco integrado
General MIDI; los perfiles con el mismo programa comparten timbre.

Cada pentagrama admite dos voces; los instrumentos de un pentagrama pueden
usar divisi. Los documentos anteriores conservan sus alturas y sonido originales,
incluido el contrabajo escrito anteriormente en altura real.
Puedes abrir [`examples/orquesta-completa.partitura`](examples/orquesta-completa.partitura)
para comprobar la plantilla y los cambios de armadura de cada instrumento.

[`examples/menuetto-y-trio-orquesta.partitura`](examples/menuetto-y-trio-orquesta.partitura)
adapta el Menuetto de cuerdas a **32 partes y 84 compases**: piccolo, maderas a dos,
corno inglés, clarinete bajo, contrafagot, cuatro cornos en Fa, dos trompetas en Si♭,
tres trombones, tuba, timbales, bombo, platillos, triángulo, arpa y las cinco secciones
de cuerdas. Conserva divisi, armónicos, pizzicato, reguladores y cambios de tempo,
métrica y tonalidad. El contrabajo pasa a escritura de octava sin cambiar su sonido.
Los compases **17 y 65** permiten comparar el tutti; el **33** inicia el Trio y el
**49** cambia a 6/8. En Re mayor real, los clarinetes y trompetas en Si♭ muestran
Mi mayor y los cornos en Fa muestran La mayor. Se puede regenerar con:

```bash
cargo run -p score-io --example menuetto_orquesta -- examples/menuetto-y-trio-cuerdas.partitura examples/menuetto-y-trio-orquesta.partitura
```

Los archivos nuevos usan el esquema 19 y guardan partes, pentagramas, acordes, alteraciones, tonalidad y expresión. Se abren los
esquemas 1–18; los esquemas 1–5 usan Do mayor para conservar su sonido original: las notas existentes de piano permanecen en la mano derecha y se
añade la izquierda vacía. Puedes abrir
[`examples/trio.partitura`](examples/trio.partitura) para escuchar un conjunto de
piano, flauta y violoncello.

## Dos voces y divisi

Elige **Voz 1** o **Voz 2** en **Escribir › Voz**, en Propiedades o con
**⌥1 / ⌥2**. Cada voz tiene sus propias notas, silencios y duraciones dentro del
mismo compás; no consume el espacio rítmico de la otra. Al elegir una voz vacía
se activa la escritura desde el inicio del compás. Si ya contiene música, se
selecciona su primera nota: usa las flechas para recorrerla y **N** para continuar
escribiendo después de la selección. Usa silencios para retrasar una entrada.

Cuando conviven ambas voces, la voz 1 lleva plicas hacia arriba y la 2 hacia abajo.
Las barras, tresillos, ligaduras y articulaciones pertenecen a su voz. Seleccionar,
copiar, pegar y borrar un pasaje actúa sobre la voz activa; al pulsar una nota se
activa su voz. En un unísono con cabezas coincidentes, se elige la voz activa.
Cada mano del piano admite también sus dos voces.

Para una sección instrumental, **div.** abre dos pentagramas desde la nota o silencio
seleccionado: voz 1 en **Divisi I** y voz 2 en **Divisi II**, conservando la clave.
**unis.** marca el regreso a la ejecución conjunta en la posición elegida. Las
notas conservan su voz, identidad, ritmo y conexiones; cambiar la presentación
no cambia la reproducción. Los botones están en **Dinámicas › Divisi** (**Dividir** y **Unir**), Propiedades
y el menú Expresión; funcionan también en un compás vacío.

Para dividir únicamente un rango, selecciona sus notas con **Mayús + clic** o
arrastrando y pulsa **div.** o **Dividir**. El tramo comienza en la primera
nota y termina al finalizar la última; allí se restaura la presentación previa.
Puedes abrir y cerrar varios tramos dentro de un mismo compás. Sin selección se
usa la posición de entrada. Para facilitar la lectura, el pentagrama inferior
se muestra completo en cada compás que contiene divisi, con su clave y armadura.
Las marcas «div.» y «unis.» conservan sus tiempos exactos dentro del compás;
fuera de los compases afectados vuelve un solo pentagrama. Las notas sostenidas
conservan su figura y su línea, sin ligaduras artificiales entre pentagramas.
El documento y la reproducción no cambian. Los cortes que necesitarían figuras
más pequeñas que las disponibles se rechazan sin modificar la música.
Pulsar otra vez una indicación activa
quita esa frontera; Deshacer recupera el cambio completo. Selecciona las líneas
con **Divisi I / II**, **⌥↑ / ⌥↓**, un clic o **⌥1 / ⌥2**.

Las dinámicas, reguladores y técnicas se comparten entre las dos voces de la
sección; ritmo y articulaciones siguen siendo independientes. El guardado, la
recuperación y el PDF conservan los pasajes. Los archivos anteriores de divisi
completo se abren como antes; al editar su primer pasaje se reúnen internamente
las líneas sin perder música. Si contienen más de dos voces, primero vacía las
sobrantes. El piano conserva sus dos manos y no usa esta conversión.

Ejemplos: [`divisi-dentro-del-compas.partitura`](examples/divisi-dentro-del-compas.partitura),
[`divisi-por-pasaje.partitura`](examples/divisi-por-pasaje.partitura),
[`voces-y-divisi.partitura`](examples/voces-y-divisi.partitura) y
[`divisi-dos-pentagramas.partitura`](examples/divisi-dos-pentagramas.partitura).

## Piano a dos manos

Cada piano es un solo instrumento con **mano derecha en clave de sol** y **mano
izquierda en clave de fa**, unidos por una llave y barras de compás. Ambas manos
se escriben y editan de forma independiente, con sus tiempos alineados.

Elige **Mano derecha** o **Mano izquierda** debajo del piano en el panel. También
puedes pulsar el pentagrama correspondiente o usar **⌥↑ / ⌥↓**. La octava inicial
es 4 para la derecha y 3 para la izquierda; **⌘↑ / ⌘↓** mueve la nota escrita una
octava y la entrada continúa desde allí.
Los compases, el guardado y Deshacer incluyen las dos manos. Reproducir las mezcla
con el mismo piano; pausar y continuar mantiene el punto exacto de ambas.

El seguimiento intenta mostrar el conjunto y, si es demasiado alto, las dos manos
del piano activo. Los unísonos superpuestos de ambas manos se sostienen como una
sola tecla hasta que terminan; todavía no se representa pedal ni escritura entre
pentagramas. Puedes abrir [`examples/piano-dos-manos.partitura`](examples/piano-dos-manos.partitura).

## Acordes en cada mano

Escribe la primera nota con `A`–`G` y añade las demás con **⇧A–⇧G** o con
intervalos **⇧2–⇧9**. Por ejemplo, `C`, `⇧3`, `⇧5` crea Do–Mi–Sol. Todas comparten la duración
de la primera nota y ocupan un único tiempo de entrada. Se admiten hasta **8 posiciones
distintas del pentagrama por acorde**, en cualquiera de las dos manos.

También puedes activar **Acorde: sí** en la barra o con **⌥A**: las letras normales
añaden notas al elemento seleccionado. Desactívalo para continuar con la siguiente
figura. Si no hay una nota seleccionada, el modo introduce la primera normalmente.

Un clic en cualquier cabeza selecciona el acorde completo. Las flechas lo transponen;
**⌥Backspace** elimina su nota más alta, conservando al menos
una. **Backspace** convierte el acorde entero en silencio conservando su duración. Puedes deshacer cada cambio.
La audición, reproducción, pausa y seguimiento incluyen todas sus notas.
Abre [`examples/piano-acordes.partitura`](examples/piano-acordes.partitura) para probarlo.

## Vistas de la partitura

El selector inferior permite alternar entre **Página**, **Continua** y
**Pentagrama**. También están en el menú **Vista**.

- **Página:** hojas numeradas con saltos entre sistemas. Usa las flechas del
  panel izquierdo para pasar de página. El zoom y el tamaño de ventana conservan
  la distribución de las hojas.
- **Continua:** renglones que se adaptan al ancho disponible y desplazamiento vertical.
- **Pentagrama:** todos los compases en una franja horizontal, con los instrumentos
  alineados y desplazamiento lateral.

**⌘0** ajusta una hoja completa a la ventana. Los botones **− / +** y los atajos de
zoom funcionan en las tres vistas. Cambiar de vista conserva la selección, el
punto de entrada y la reproducción. Si un sistema completo requiere más espacio
por su densidad o número de instrumentos, las hojas se amplían para conservarlo.

## Sostenidos, bemoles y becuadros

En **Escribir › Alteración**, elige Armadura, **♯**, **♭** o **♮** antes de escribir. Esa opción se
mantiene para las próximas notas y para las que añadas al acorde con ⇧+letra.
Armadura usa la tonalidad elegida; ♮ fuerza una nota natural con becuadro explícito.

Para cambiar una nota existente, selecciónala y pulsa **Aplicar ♯**, **Aplicar ♭**
o **Aplicar ♮**. En un acorde, elige primero su cabeza con un clic o su nombre en
**Nota elegida**; la cabeza en azul más oscuro indica cuál se va a modificar. **⌥↑ / ⌥↓** recorre
sus notas. Como en Sibelius, **8 / 9 / 7** aplican sostenido, bemol y becuadro, y
volver a pulsar el mismo quita la alteración propia para seguir la armadura. Si no
hay selección, estas teclas cambian la opción de entrada. Alterar una nota existente no cambia la entrada.
Cada cambio se puede deshacer y se escucha con la audición activada.

Una nota que sigue la armadura después de una alteración propia lleva el signo
de restitución cuando hace falta en ese compás. Las notas de otras octavas no
heredan una alteración individual. El estado de notación se reinicia en cada
compás y se mantiene separado entre manos. Los signos de los acordes se distribuyen
en columnas. La transposición con flechas conserva el tipo de alteración propia.
Aún no hay dobles alteraciones ni cambio automático de enarmonía.

Puedes abrir [`examples/piano-alteraciones.partitura`](examples/piano-alteraciones.partitura).

## Armaduras de tonalidad

Pulsa **Tonalidad: Do mayor** en la cabecera, elige **Mayores** o **Menores** y
selecciona la tonalidad del panel izquierdo. También están en el menú nativo
**Tonalidad**. Hay 30 opciones, desde siete bemoles hasta siete sostenidos,
incluidas las relativas menores. La armadura aparece después de la clave en cada
sistema, en todos los pentagramas.

Con **Escribir › Alteración › Armadura**, las notas y acordes usan sus sostenidos o bemoles en
todas las octavas: en Sol mayor, escribir F produce Fa♯ sin repetir un sostenido
ante cada nota. **Aplicar ♮** (`7`) permite escribir un Fa natural; **Seguir armadura**
o volver a pulsar `7` lo devuelve a Fa♯. Los nombres de **Nota elegida** muestran la altura que
sonará. La audición, el inicio desde selección y la pausa usan la misma tonalidad.

En **Aplicar desde**, elige **Inicio de la pieza** o el **compás seleccionado**.
El cambio afecta a todos los pentagramas desde ese compás hasta el siguiente
cambio de tonalidad, incluidas las notas escritas que siguen la armadura. Conserva
sus posiciones y las alteraciones individuales. Puedes deshacerlo o usar
**Quitar cambio de tonalidad** para recuperar la armadura anterior. Las menores
usan su armadura habitual; las alteraciones de la menor armónica o melódica se
escriben individualmente.

Prueba [`piano-sol-mayor.partitura`](examples/piano-sol-mayor.partitura) o
[`piano-mi-bemol-mayor.partitura`](examples/piano-mi-bemol-mayor.partitura).

## Cambios de métrica

Selecciona un compás, incluso vacío, y pulsa **T** o el botón **4/4** de la cabecera.
Elige **Inicio de la pieza** para cambiar la métrica inicial, o **Compás N
seleccionado** para introducir un cambio durante la pieza. Puedes elegir una
métrica habitual o ajustar numerador y denominador y pulsar **Aplicar**.

La nueva métrica rige en todos los pentagramas hasta el siguiente cambio. Conserva
las notas y los compases existentes; si alguna nota no cabe en la nueva duración,
la aplicación muestra el motivo y deja la pieza intacta. **Quitar cambio de
métrica** recupera el compás anterior. Los cambios de métrica y tonalidad se pueden
deshacer y rehacer, se guardan en el archivo y se reflejan en la reproducción,
el contador y la regla temporal.

## Indicación de tempo

Pulsa el valor de **Tempo**, **Texto › Tempo › Indicación** o **⌥⌘T**. Elige **Inicio de la
pieza** o el compás donde quieres comenzar el cambio. Escribe una indicación
como «Andante cantabile», introduce de **20 a 400 pulsos por minuto** y elige
la figura del pulso, con puntillo si lo necesitas. **Aplicar tempo** o **Enter**
escribe la indicación sobre el primer pentagrama. Puedes usar también las
indicaciones habituales del panel; sus valores son editables.

El tempo rige en todos los instrumentos hasta el siguiente cambio. Haz clic sobre
una indicación escrita para modificarla o usa **Quitar cambio de tempo** para
recuperar el anterior. Los cambios se guardan y se pueden deshacer. El transporte
muestra el tempo de su posición; **− / +** modifican ese valor y conservan la
posición, deteniendo la secuencia para reconstruirla al volver a reproducir.

## Escuchar mientras escribes

`cargo run` incluye el adaptador de **VSCO CE**, un
[paquete de sonido opcional](docs/vsco.md): si sus muestras están instaladas, sus
grabaciones originales SFZ se eligen desde **Sonidos**; si no, solo aparece GeneralUser.
Para compilar sin el adaptador, usa `--no-default-features --features runtime-shaders`.
GeneralUser sigue siendo la opción inicial; cambiar de paquete no modifica la partitura.
Los bancos implementan un [contrato común de audio](docs/sound-banks.md).

La opción **Audición al escribir** está activada al iniciar: cada nota o acorde introducido
o transpuesto suena brevemente con el sonido de su instrumento, la dinámica vigente y
su articulación. Cambiar una dinámica o articulación vuelve a escuchar la nota seleccionada.
El staccato acorta la audición y, si el banco dispone de ellas, usa muestras cortas reales.
Los acentos y marcato refuerzan el ataque; tenuto conserva la duración completa.
Los silencios y las ediciones
rechazadas no disparan una nota. Puedes desactivar la audición desde la barra de sonido.

Selecciona una nota o silencio y pulsa **Espacio** o **Reproducir** para empezar
desde ese elemento. Sin selección, comienza desde el inicio. La selección aparece en azul;
las notas y silencios activos se colorean en naranja, incluidas sus plicas, sin
recuadros que oculten el pentagrama. Un cabezal con línea fina avanza de forma
continua con el audio, también entre notas y durante silencios. La regla inferior
muestra compases y tiempos: haz clic para colocar el cabezal en el tiempo más cercano
y pulsa **Espacio** o **Reproducir** para empezar desde ahí. Durante la reproducción,
el clic salta directamente al punto elegido; en pausa, conserva la pausa en la nueva
posición. La selección de notas se conserva al usar la regla. El contador principal indica compás y tiempo y conserva
el tiempo transcurrido debajo. La vista se desplaza automáticamente cuando hace falta
para mantenerla visible, también al cambiar de renglón o en compases muy anchos.

Durante la reproducción, **Espacio** o **Pausar** suspende el sonido. **Espacio**
o **Continuar** reanuda desde el mismo punto, incluso a mitad de una nota o un
silencio. La pausa conserva el estado de todos los instrumentos y no vuelve a atacar la nota.
Durante la pausa puedes desplazarte libremente; al continuar, la vista vuelve a
seguir el punto de reproducción.
También puedes situar el cabezal haciendo **clic directamente en la partitura**:
una nota o silencio elige su entrada exacta y el espacio del pentagrama elige el
tiempo más cercano. Funciona parado, reproduciendo y en pausa; solo modifica la
posición del transporte. **Shift + clic/arrastre** conserva el cabezal para ampliar
la selección musical.

Si amplías la selección con Shift durante una pausa, Continuar conserva el punto pausado;
para empezar desde la nueva selección, pulsa **Detener** y después **Reproducir**.

**Esc** y **Detener** cancelan la reproducción y la pausa, conservando la selección.
Con la reproducción ya detenida, Esc quita la selección para volver a reproducir
desde el inicio. Al editar o deshacer se detiene la secuencia anterior y, si corresponde,
suena la nueva nota. Se respetan los silencios y la duración restante de los compases
interiores y finales que muestra la regla.

### Practicar y revisar

- **Bucle** (`⌥P`, o el botón de bucle del transporte): repite lo seleccionado. Con
  un pasaje seleccionado se repite de su primera a su última nota; con una sola nota,
  su compás; sin selección, toda la partitura. La vuelta al inicio es exacta y las
  notas que ya sonaban en ese punto vuelven a sonar. Si lo activas mientras suena
  fuera del pasaje, la reproducción salta a su comienzo. Para cambiar el pasaje,
  quita el bucle, selecciona otro y vuelve a activarlo.
- **Media velocidad** (`⇧Espacio`, o **½×**): toca al 50 % del tempo escrito, sin
  cambiar las indicaciones de la partitura. Parado, empieza a reproducir; sonando,
  cambia de velocidad en el mismo punto. Funciona también con el bucle.
- **Repetir la última reproducción** (`⌥Espacio`): vuelve a empezar desde donde
  empezó la reproducción anterior.
- **Línea de reproducción al inicio** (`⌘[`): lleva el cabezal al compás 1, sonando o parado.
- **Compás siguiente** (`⌘→`) e **Inicio del compás** (`⌘←`): seleccionan la primera
  nota del compás siguiente, o la del compás actual; si ya estás ahí, la del anterior.
- **Ir al compás…** (`⌥⌘G`, o un clic en el contador **COMPÁS · TIEMPO**): escribe el
  número y pulsa Enter. Si suena, la reproducción salta también allí.

El tempo inicial es **120 negras por minuto**. Sus indicaciones y cambios forman
parte del archivo `.partitura`. El volumen se ajusta sin detener la reproducción
y sigue siendo un ajuste de sesión.

El sonido usa la salida predeterminada del sistema. Si no está disponible, la
edición sigue funcionando y aparece **Reintentar audio** para volver a conectarla.
Para comprobar brevemente el dispositivo sin abrir la interfaz:

```sh
cargo run -p score-platform --example audio_smoke
```

Los archivos `.partitura` son JSON con esquema versionado. Se escriben mediante
reemplazo atómico; abrir, crear o cerrar pide descartar los cambios si los hay.
Los cambios se copian automáticamente a recuperación tras 750 ms sin edición
(o a los 3 segundos si sigues escribiendo). Esta copia separada no modifica tu
archivo original. Ante un cierre forzado, el próximo inicio recupera la última
copia completada, conserva la ruta original y la muestra con cambios sin guardar.
Guardar o descartar explícitamente limpia la recuperación. La salida desde el
Dock también espera al escritor. Si no se puede escribir, aparece el error en la
barra de estado. Solo una sesión puede usar el mismo directorio de recuperación.

## Corregir notas y compases

Haz clic en una nota para salir de la entrada y editarla: **A–G** reemplaza la
altura elegida; las figuras de **Escribir › Duración** cambian su duración, incluyendo el
puntillo o tresillo que ya tuviera. En acordes se conserva el resto de las notas.
Si la nueva duración no cabe, la operación se rechaza completa. **N** vuelve a
activar la entrada de notas nuevas.

**Backspace/Delete** convierte las notas seleccionadas en silencios de la misma
duración; las notas posteriores conservan su tiempo. **Edición › Eliminar notas y
su tiempo** y **Cortar** quitan los elementos y adelantan los siguientes dentro
del compás. Todo admite deshacer y rehacer.

En **Partitura › Compases**, **Antes** (⇧⌘B) añade un compás vacío antes del
actual y **Eliminar** (⌘Backspace) borra los compases que abarca la
selección, o el actual si no hay un pasaje. Afectan a todos los instrumentos y
manos; desplazan las indicaciones con la música y conservan la métrica, tonalidad
y tempo efectivos del primer compás retenido. Siempre queda al menos un compás.

## Repeticiones, casillas y saltos

En **Partitura › Repeticiones** escribe el inicio y fin de repetición, las casillas
de primera y segunda vez, la doble barra y la barra final. En **Da capo y segno** están
Segno, Coda y Fine, y el desplegable **D.C. / D.S.** reúne To Coda y los saltos; el menú **Repeticiones** reúne también
las seis variantes de D.C./D.S. y las opciones para quitar la forma.

Selecciona un pasaje para colocar las marcas iniciales en su primer compás y las
finales en el último; una casilla abarca todo el rango. Sin pasaje se usa el
compás del cursor. La selección se conserva al añadir varias marcas y cada cambio
se puede deshacer. Volver a pulsar una barra o marca activa la quita. Las casillas
se quitan desde el menú. El último compás lleva una barra final automática. Las marcas acompañan a sus compases al insertar o borrar
música y se conservan en el archivo nativo, PDF y MusicXML.

**Reproducción › Práctica › Repeticiones** está activado al iniciar la sesión. El cursor
vuelve al compás que suena y el reloj sigue contando el tiempo transcurrido.
Después de un D.C./D.S. se omiten las repeticiones y se toca la última casilla de
cada grupo. Cambiar esta opción detiene el transporte; desactivarla permite
escuchar la música en el orden escrito.

## Trémolos de una nota o acorde

Selecciona una nota, acorde o pasaje sin silencios y elige **Articulaciones › Trémolo**:
una, dos o tres barras. La marca cruza la plica; en las redondas se coloca encima
de la cabeza. Otra variante sustituye a la anterior y volver a pulsar la activa
la quita. También puedes seleccionar la marca y borrarla conservando la nota.
Cada cambio del pasaje se deshace en un paso.

La reproducción reparte ataques de corchea, semicorchea o fusa dentro de la
duración escrita, también con ligaduras de unión, calderones y repeticiones.
En la cuerda, el trémolo de tres barras usa la grabación de trémolo del paquete de
sonido cuando la tiene ([detalles](docs/sfz.md#trémolo)).
Solo el primer ataque conserva el acento. Las marcas se conservan al copiar y
pegar y en los formatos nativo, PDF y MusicXML.

## Marcas de ensayo

**Texto › Marca de ensayo › Añadir** o **⌘R** añade o quita una marca al principio del compás
seleccionado, o del primer compás del pasaje. Los controles **Letras**, **Números**
y **Nº de compás** (**Texto › Estilo de ensayo**) eligen el estilo para toda la obra: A, B… Z, AA, BB…; 1, 2, 3…; o el
número del propio compás. Las etiquetas se renumeran al añadir, quitar, insertar
o borrar compases. Cambiar de estilo conserva la posición de las barras.

Las marcas aparecen en un recuadro sobre el primer pentagrama y acompañan a las
partes extraídas. Se guardan con la obra y se conservan en PDF y MusicXML. Al
importar MusicXML, un texto propio como «Trío» se mantiene sin impedir que las
demás marcas sigan numerándose automáticamente. Añadir o quitar una marca y
cambiar de estilo admite deshacer y conserva el pasaje seleccionado.

## Exportar partitura general y partes

**Exportar…** en la cabecera o **Archivo › Exportar partitura y partes…** abre un
selector con la **partitura general** y cada **parte instrumental**. Puedes
marcarlas por separado, seleccionar todo o quitar todas las marcas. Elige **PDF**,
**MusicXML** o ambos; el diálogo muestra cuántos archivos se generarán.

**Elegir destino…** permite dar nombre y ubicación a una carpeta nueva. Cada
documento y formato produce un archivo independiente: `00 - Partitura general.pdf`,
`01 - Violín I.pdf`, `02 - Violín II.pdf`, etc. También puedes exportar solo una
parte. Si la carpeta ya existe se pide otro nombre, sin mezclar ni reemplazar su
contenido. Cancelar el destino conserva la selección.

Las partes conservan todas sus voces y pentagramas (incluidas ambas manos del
piano y los pasajes divisi), además de notas, expresiones y cambios globales de
compás, tonalidad y tempo. Mantienen los nombres de sección «Violín I» y «Violín II».
Se recalcula su presentación como partituras independientes. Las alturas son de
concierto, igual que en el editor; todavía no se generan partes transpuestas.
Exportar no modifica la obra ni marca sus cambios como guardados.

```sh
cargo run -p score-export --example export -- examples/menuetto-y-trio-cuerdas.partitura output/menuetto-partes
```

El directorio padre debe existir y `menuetto-partes` debe ser nuevo. En macOS este
ejemplo genera la partitura general y todas las partes en ambos formatos.

## Exportar PDF

Selecciona **PDF** en el diálogo de exportación para guardar todas las
páginas en A4. El PDF usa trazos vectoriales de Bravura, incluye las indicaciones
y arpegios entre las manos, y calcula las páginas independientemente de la vista
y el zoom de la pantalla. No incluye selecciones ni controles de la interfaz.
Las partes individuales llevan el instrumento en el encabezado, sin repetirlo en cada sistema. Usan un pentagrama más compacto y hasta seis compases por sistema; mantienen las etiquetas de divisi y de las manos del piano. El espacio vertical se calcula con la tinta real y las indicaciones se acercan a la música que tienen debajo. La partitura general usa el mismo espaciado compacto en la app (Página, Continua y Pentagrama) y al exportar PDF.
La app instalable para Mac queda para otra entrega.

Para exportar un ejemplo sin abrir la interfaz:

```sh
cargo run -p score-pdf --example export -- examples/piano-arpegios.partitura exports/piano.pdf
```
El directorio de destino debe existir.

## Exportar MusicXML

**Archivo › Exportar MusicXML…** abre el mismo selector con MusicXML marcado.
Cada documento seleccionado se guarda como **.musicxml** sin comprimir
(MusicXML 4.0) para intercambiar la obra con otros editores. Incluye instrumentos,
claves, notas, acordes, silencios, dos voces, puntillos y tresillos; cambios de
compás, tonalidad y tempo; dinámicas, articulaciones, ligaduras, reguladores,
fermatas, técnicas, armónicos y arpegios, incluidos los que abarcan ambas manos;
notas de adorno, líneas de 8va y 8vb, pedal, trinos y glissandi, textos de expresión
y técnica, letra (`lyric`, con sus estrofas, guiones y extensiones) y cifrado (`harmony`).

El divisi conserva las voces en un mismo instrumento, reserva un segundo
pentagrama y lo oculta en los compases fuera del pasaje. Los cambios dentro del
compás conservan su posición; si cortan una nota sostenida, sus fragmentos se
unen con ligaduras. La presentación exacta depende del editor receptor.
Las alturas se exportan en la escritura de cada instrumento, con su armadura y transposición. Los armónicos
artificiales incluyen la nota pisada y la nota de contacto en rombo; su sonido,
como el resto de la reproducción, depende del programa receptor. El timbre
sintetizado propio de Levare no se incorpora al archivo.

La exportación escribe una copia y no sustituye el documento nativo ni cambia
su estado de guardado. La exportación comprimida **.mxl** todavía no está disponible.

```sh
cargo run -p score-io --example export_musicxml -- examples/menuetto-y-trio-cuerdas.partitura output/menuetto.musicxml
```

El directorio de destino debe existir.

## Abrir MusicXML

**Archivo › Abrir…** (⌘O) admite documentos nativos **.partitura**, MusicXML
**.musicxml** y **.xml**, y archivos comprimidos **.mxl**. Se reconocen por su
contenido y se admiten las estructuras partwise y timewise.

La importación crea una copia editable sin ruta nativa y la marca como pendiente
de guardar. **Guardar** pide un archivo **.partitura**; el original MusicXML no se
modifica. La copia importada también participa en el autoguardado y recuperación.

Se conservan notas, acordes de duración uniforme, silencios, hasta dos voces por
pentagrama y dos pentagramas por instrumento; compás, armadura, tempo, dinámicas,
articulaciones básicas, ligaduras completas, reguladores, fermatas, arco/pizzicato,
armónicos y arpegios. Las partes transpositoras conservan su intervalo de transposición y se reconstruyen en altura escrita. Las
anacrusas se representan con un compás inicial de duración reducida. El divisi
puede reconstruirse desde las indicaciones div./unis. o la visibilidad del segundo
pentagrama. La distribución de página se recalcula con el grabado de Levare.

Si se omite o adapta algún elemento, aparece un aviso al abrir: por ejemplo,
letras, textos libres, adornos, cifrados, instrumentos sin equivalente
y ligaduras que afectan solo a algunas notas de un acorde. Los casos que no
pueden representar la música correctamente se rechazan con una explicación,
incluidos más de dos voces por pentagrama, ritmos fuera de la resolución admitida,
microtonos, métricas simultáneas distintas y armónicos de contacto sin altura
sonora deducible. Los armónicos naturales sin tipo de altura se interpretan a la
altura escrita con un aviso. Los armónicos artificiales admitidos son de cuarta.

```sh
cargo run -p score-io --example import_musicxml -- obra.musicxml copia-editable.partitura
```

## Verificar

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Para trabajar solo en el núcleo, sin dependencias gráficas ni de audio:

```sh
cargo test -p score-core -p score-edit -p score-layout -p score-io -p score-document -p score-keymap
```

La arquitectura, decisiones y siguientes entregas están en
[docs/desarrollo.md](docs/desarrollo.md). El inventario de referencia continúa en
[docs/herramientas.md](docs/herramientas.md).

## Alcance actual

Dos voces por pentagrama y divisi en uno o dos pentagramas, acordes de hasta ocho notas con sostenidos, bemoles y becuadros, seis duraciones (de la redonda a la fusa) con puntillos y tresillos,
y claves de sol, do y fa. El grabado incluye barrado, líneas adicionales,
articulaciones, dinámicas, ligaduras y reguladores, notas de adorno, líneas, textos,
letra y cifrado, con caché por compás.
La reproducción mezcla las partes en un reloj común, con inicio desde selección,
pausa y continuación, bucle y media velocidad. Aún no hay MIDI externo.

Se incluye [Bravura 1.392](https://github.com/steinbergmedia/bravura/tree/bravura-1.392),
copyright Steinberg Media Technologies GmbH, bajo la
[SIL Open Font License](assets/fonts/OFL.txt).
También se incluye [GeneralUser GS 2.0.3](assets/sounds/README.md) de S. Christian
Collins, con su [licencia original](assets/sounds/LICENSE.txt).

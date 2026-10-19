# Partitura

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

Windows y Linux todavía no están validados. La primera plataforma es macOS;
el modelo, la edición y el layout no dependen de GPUI.

## Interfaz del editor

La interfaz sigue la referencia visual de **Editor · Entrada de notas**: tema
oscuro, cinta de herramientas, instrumentos por familia, hoja clara, inspector y
transporte inferior. La partitura muestra sus datos reales, sin documentos de
muestra ni conexiones MIDI simuladas. Puedes abrir
[`examples/cuarteto-en-sol.partitura`](examples/cuarteto-en-sol.partitura) para
ver el conjunto de cinco instrumentos.

- **Archivo:** nueva, abrir, guardar y guardar como. Los iconos de la cabecera
  permiten guardar, deshacer y rehacer.
- **Cinta:** las pestañas siguen la organización de Sibelius y MuseScore.
  **Inicio** reúne portapapeles, instrumentos, compases y selección; **Entrada**,
  todo lo que se escribe con cada nota (modo de entrada, duración, alteraciones,
  articulaciones y ligaduras, voces y acordes con intervalos); **Notación**, clave,
  tonalidad y compás de tiempo, dinámicas, reguladores, técnica de cuerda y otros
  elementos. Deshacer y rehacer están en la cabecera. No hay botones de «Quitar»:
  selecciona la nota en la partitura y vuelve a pulsar la marca activa (resaltada)
  para quitarla, o pulsa Backspace para borrar la selección. Si falta espacio, los
  grupos menos usados se pliegan en un botón desplegable.
- **Instrumentos:** selección por familias y manos del piano; el selector de
  tonalidad aparece en este panel. Los botones de la cabecera ocultan los paneles.
- **Propiedades:** nota y duración seleccionadas, elección de nota dentro del
  acorde, alteraciones, octava de entrada, armadura y audición.
- **Mezclador / Sonidos:** volumen general, volumen y posición estéreo por instrumento
  (con varias partes, cada una parte de su sitio habitual en el escenario),
  reverberación suave con cantidad, tamaño de sala y encendido/apagado. Los deslizadores
  responden a clic, arrastre y botones −/+, incluso durante la reproducción.
  Los ajustes son de sesión, se conservan al cambiar de banco de sonidos y no se guardan
  en la partitura. Sonidos permite elegir banco, añadir/quitar parte y reconectar audio.
  Reproducción incluye tempo y audición.
- **Transporte:** reproducir/pausar, detener e iniciar desde el principio sin
  borrar la selección. Reloj y regla siguen la posición musical real.
- **Vista:** desplazamiento continuo y zoom entre 50 y 150 %, con selección y
  seguimiento ajustados a la escala. El zoom inicial es 70 %.

Los controles que llevan **TODO** son pendientes: varias
pestañas de documentos, búsqueda, exportación/impresión, entrada MIDI/tiempo real,
puntillos, tresillos, calderones, reguladores crescendo/decrescendo, voces 2–4, plicas manuales,
velocidad/duración sonora por nota, edición de duración existente, silencio/solo,
cambio de timbre, paginación y vista de pentagrama individual.
El guardado sigue siendo manual; el indicador muestra el estado real del archivo.

## Uso

Los atajos imitan el perfil **Notebook (portátil)** de Sibelius y funcionan igual
con o sin bloque numérico: la fila de números hace de teclado numérico. La lista
completa, generada desde el código, está en [docs/atajos.md](docs/atajos.md).
Puedes cambiarlos en **Partitura › Preferencias de atajos…** (`⌘,`) o con el botón
**Atajos** de la cabecera.

La barra y el inspector muestran el nombre y el atajo actual de cada control al
mantener el cursor encima. Los menús y la ventana **Atajos** conservan las teclas
visibles como referencia, sin repetirlas permanentemente junto a los botones.

| Tecla / acción | Resultado |
|---|---|
| `N` | Activar / desactivar la entrada de notas |
| `A`–`G` | Escribir La–Sol con la octava y duración actuales |
| `⇧A`–`⇧G` | Añadir esa nota encima del acorde seleccionado |
| `⇧2`–`⇧9` | Añadir una segunda … novena encima (según la armadura) |
| `2` `3` `4` `5` `6` | Semicorchea, corchea, negra, blanca, redonda |
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
| `⌥A` / `⌥⌫` | Modo acorde / quitar la nota más aguda (propios de Partitura) |
| `⌘Z` / `⌘Y` (o `⇧⌘Z`) | Deshacer / rehacer |
| `⌘+` / `⌘−` / `⌘1` | Acercar / alejar / 100 % |
| Clic en una nota | Seleccionarla; la entrada continúa después de ella |
| Clic en un compás vacío | Mover el punto de entrada allí |

Las teclas de Sibelius sin función todavía (`T` compás de tiempo, `Q` clave, `L`
líneas, `H` reguladores, `⌥1`–`⌥4` voces, `.` puntillo, `1` fusa, `⌘C`/`⌘V`…)
están reservadas y muestran «Pendiente». Las articulaciones no tienen atajo por
defecto, como en Sibelius portátil; asígnalas en Preferencias si las usas a menudo.

Un compás lleno avanza al siguiente al introducir otra figura. Si no existe,
se crea. Una figura que exceda la duración restante se rechaza sin alterar el
documento. Cambiar la duración afecta a la próxima entrada. Puedes abrir
[`examples/primera-melodia.partitura`](examples/primera-melodia.partitura) como ejemplo.

## Copiar, cortar y pegar música

- **Arrastra sobre las notas o el pentagrama** para seleccionar un pasaje completo,
  hacia delante o hacia atrás, incluso entre compases y renglones. La franja azul
  indica el pasaje y la barra superior muestra cuántos elementos contiene, con
  botones **Copiar** y **Cortar**. Un clic sigue seleccionando una sola nota.
- **Shift + clic** o **Shift + ←/→** amplía la selección dentro del mismo
  pentagrama; **⌘A** selecciona todos sus elementos escritos.
- **⌘C** copia, **⌘X** corta y **⌘V** pega. También están disponibles en la
  cinta **Portapapeles** y el menú **Edición**, con los atajos configurables.
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

El portapapeles musical es del sistema y funciona entre ventanas de Partitura.
El texto corriente o las imágenes de otras aplicaciones no se interpretan como notas.
La selección de pasajes abarca un pentagrama cada vez; puedes copiar cada mano del
piano por separado. Las ligaduras con extremos fuera del pasaje no se copian.

## Puntillos y tresillos

En **Entrada › Duración** elige la figura y activa **•** para un puntillo o **3**
para un tresillo. Atajos por defecto: **`.`** y **⌘3**, configurables en Preferencias.
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

Las corcheas y semicorcheas se unen automáticamente por pulso, también en acordes
y en cada mano del piano por separado. Las semicorcheas usan una segunda barra;
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

La cinta (**Entrada** para articulaciones y ligaduras, **Notación** para dinámicas,
reguladores y técnica), el inspector **Expresión** y el menú nativo **Expresión**
permiten editar estas marcas en notas y acordes de cualquier pentagrama. Todos los
cambios se guardan y admiten deshacer/rehacer. Ejemplo:
[`examples/piano-expresion.partitura`](examples/piano-expresion.partitura).

- **Unión (`↩`):** selecciona la primera de dos notas o acordes iguales y consecutivos.
  Se prolonga el sonido sin un nuevo ataque, también entre compases. Los acordes se
  ligan completos; los sonidos deben coincidir en todas sus alturas. Volver a pulsar
  `↩` quita solo la unión con la siguiente. Las cadenas pueden continuar varias notas.
- **Expresión (`S`):** selecciona el inicio, pulsa `S`, selecciona la nota final del mismo
  pentagrama y pulsa `S` otra vez. Esc cancela la operación. Pulsar `S` con la primera
  nota de una frase seleccionada la quita. La frase se dibuja con
  una curva, incluso entre sistemas. Esta versión no admite frases solapadas en un
  mismo pentagrama. En la reproducción, las notas de la frase se unen con un breve
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

Las articulaciones, dinámicas, técnicas y alteraciones activas aparecen resaltadas
en la cinta; volver a pulsarlas las quita. El inspector permite además quitar
indicaciones y las ligaduras que tocan la nota elegida.
Si una edición de alturas, una inserción o un borrado invalida una unión, se retira
esa ligadura; deshacer recupera tanto las notas como sus marcas. El inicio de
reproducción desde una selección recupera la dinámica vigente y las notas ligadas.

## Crescendo y diminuendo con reguladores

Selecciona un pasaje con **Shift+clic** o **Shift+flechas** y pulsa **H** para un
crescendo (`<`) o **Shift+H** para un diminuendo (`>`). También puedes seleccionar
la primera nota, pulsar el regulador, seleccionar la última y volver a pulsar el
mismo botón o atajo. **Esc** cancela el inicio pendiente. Las acciones están en
**Entrada › Reguladores**, el inspector **Expresión** y el menú **Expresión**.
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

## Pizzicato y arco

Selecciona una nota, acorde o silencio de **violín, viola, violonchelo o contrabajo**.
En la cinta **Notación › Técnica de cuerda**, el inspector **Expresión** o el menú
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

## Partituras con varios instrumentos

Pulsa **+ Añadir instrumentos** en el panel izquierdo para abrir el diálogo.
Marca varios instrumentos y ajusta sus cantidades con **− / +**; **Añadir** incorpora
el conjunto en una sola operación que puedes deshacer. **Cancelar** o **Esc** cierra
sin cambiar la partitura. También puedes usar el menú **Instrumentos**. Hay piano, guitarra, violín, viola, violonchelo,
contrabajo, flauta, oboe, clarinete, fagot, trompeta, trompa y trombón; se admiten
hasta **14 partes**, incluyendo instrumentos repetidos.

Selecciona una parte en el panel o pulsa su pentagrama para escribir en ella.
**⌥↑ / ⌥↓** pasa al pentagrama vecino (y, en un acorde, primero recorre sus notas). Cada parte conserva sus notas; los compases
se añaden a todos los instrumentos y los tiempos coincidentes quedan alineados.
La **papelera de cada fila** elimina ese instrumento directamente con sus pentagramas,
sin seleccionarlo antes. **Deshacer** recupera sus notas, dinámicas y ligaduras.
El menú **Instrumentos** permite eliminar la parte activa.
La partitura conserva al menos un instrumento. Si eliminas la parte activa, queda
seleccionado el instrumento vecino; al eliminar otra, se conserva tu selección.

Al abrir una partitura o añadir instrumentos, el score y el panel se ordenan
por familias: maderas, metales, teclados, cuerdas pulsadas y cuerdas. Dentro de
cada familia se mantiene el orden del catálogo orquestal (flauta–oboe–clarinete–fagot,
trompa–trompeta–trombón, violín–viola–violonchelo–contrabajo). Los instrumentos
repetidos conservan su orden relativo y sus notas; los IDs no cambian.

Cada renglón presenta **corchetes de familia** y barras de compás unidas dentro
de cada grupo, con separación entre familias. El piano conserva su **llave de dos
manos**. La barra inicial fina conecta todo el sistema. El orden se guarda en el
archivo nativo y no requiere convertir las partituras anteriores.

Cada parte usa su timbre y su clave de sol, do o fa. La escritura es en **altura
real**: los instrumentos transpositores todavía no cambian automáticamente la
notación. Hay una voz por pentagrama. El piano tiene un gran pentagrama de dos
manos; los demás instrumentos conservan un pentagrama.

Los archivos nuevos usan el esquema 10 y guardan partes, pentagramas, acordes, alteraciones, tonalidad y expresión. Se abren los
esquemas 1–9; los esquemas 1–5 usan Do mayor para conservar su sonido original: las notas existentes de piano permanecen en la mano derecha y se
añade la izquierda vacía. Puedes abrir
[`examples/trio.partitura`](examples/trio.partitura) para escuchar un conjunto de
piano, flauta y violonchelo.

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
una. **Backspace** elimina el acorde entero. Puedes deshacer cada cambio.
La audición, reproducción, pausa y seguimiento incluyen todas sus notas.
Abre [`examples/piano-acordes.partitura`](examples/piano-acordes.partitura) para probarlo.

## Sostenidos, bemoles y becuadros

En **Entrada**, elige Armadura, **♯**, **♭** o **♮** antes de escribir. Esa opción se
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

Con **Entrada: Armadura**, las notas y acordes usan sus sostenidos o bemoles en
todas las octavas: en Sol mayor, escribir F produce Fa♯ sin repetir un sostenido
ante cada nota. **Aplicar ♮** (`7`) permite escribir un Fa natural; **Seguir armadura**
o volver a pulsar `7` lo devuelve a Fa♯. Los nombres de **Nota elegida** muestran la altura que
sonará. La audición, el inicio desde selección y la pausa usan la misma tonalidad.

Cambiar la tonalidad afecta a **toda la partitura**, incluidas las notas ya
escritas que siguen la armadura. Conserva sus posiciones y las alteraciones
individuales. Puedes deshacer el cambio; no es una transposición cromática.
Por ahora hay una tonalidad global, sin cambios a mitad de la obra. Las menores
usan su armadura habitual; las alteraciones de la menor armónica o melódica se
escriben individualmente.

Prueba [`piano-sol-mayor.partitura`](examples/piano-sol-mayor.partitura) o
[`piano-mi-bemol-mayor.partitura`](examples/piano-mi-bemol-mayor.partitura).

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
posición. La selección de notas se conserva. El contador principal indica compás y tiempo y conserva
el tiempo transcurrido debajo. La vista se desplaza automáticamente cuando hace falta
para mantenerla visible, también al cambiar de renglón o en compases muy anchos.

Durante la reproducción, **Espacio** o **Pausar** suspende el sonido. **Espacio**
o **Continuar** reanuda desde el mismo punto, incluso a mitad de una nota o un
silencio. La pausa conserva el estado de todos los instrumentos y no vuelve a atacar la nota.
Durante la pausa puedes desplazarte libremente; al continuar, la vista vuelve a
seguir el punto de reproducción.
Si cambias la selección durante una pausa, Continuar conserva el punto pausado;
para empezar desde la nueva selección, pulsa **Detener** y después **Reproducir**.

**Esc** y **Detener** cancelan la reproducción y la pausa, conservando la selección.
Con la reproducción ya detenida, Esc quita la selección para volver a reproducir
desde el inicio. Al editar o deshacer se detiene la secuencia anterior y, si corresponde,
suena la nueva nota. Se respetan los silencios y la duración restante de los compases
interiores y finales que muestra la regla.

El tempo inicial es **120 negras por minuto**, ajustable entre 30 y 240. Cambiarlo
detiene la secuencia; vuelve a pulsar Espacio para oírla al nuevo tempo. El volumen
se ajusta sin detener la reproducción. Estos ajustes son de sesión y aún no se
guardan en `.partitura`.

El sonido usa la salida predeterminada del sistema. Si no está disponible, la
edición sigue funcionando y aparece **Reintentar audio** para volver a conectarla.
Para comprobar brevemente el dispositivo sin abrir la interfaz:

```sh
cargo run -p score-platform --example audio_smoke
```

Los archivos `.partitura` son JSON con esquema versionado. Se escriben mediante
reemplazo atómico; abrir, crear o cerrar pide descartar los cambios si los hay.
Al salir desde el Dock/sistema se intenta conservar una copia para el siguiente
inicio, que se abre como documento sin guardar. **Todavía no hay autoguardado
periódico ni recuperación ante un bloqueo o cierre forzado.** Usa una sola sesión
de la aplicación a la vez en esta primera versión.

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

Una voz por pentagrama, acordes de hasta ocho notas con sostenidos, bemoles y becuadros y silencios sin puntillo, cinco duraciones
y claves de sol, do y fa. El layout inicial añade líneas adicionales y banderas individuales, con
caché por compás; todavía no tiene barrado ni grabado profesional.
La reproducción mezcla las partes en un reloj común, con inicio desde selección,
pausa y continuación. Aún no hay repeticiones, MIDI externo, MusicXML ni PDF.

Se incluye [Bravura 1.392](https://github.com/steinbergmedia/bravura/tree/bravura-1.392),
copyright Steinberg Media Technologies GmbH, bajo la
[SIL Open Font License](assets/fonts/OFL.txt).
También se incluye [GeneralUser GS 2.0.3](assets/sounds/README.md) de S. Christian
Collins, con su [licencia original](assets/sounds/LICENSE.txt).

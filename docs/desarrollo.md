# Desarrollo de Levare

Primera entrega iniciada el 2026-10-17 a partir de `herramientas.md`.

## Objetivo de esta entrega

Cerrar el recorrido mínimo de trabajo: abrir la aplicación, escribir una melodía,
seleccionar y corregir notas, deshacer y guardar/abrir el documento.
macOS es la plataforma inicial por el entorno disponible; la arquitectura mantiene
el núcleo independiente para futuras plataformas.

## Límites entre módulos

```text
partitura (GPUI, entrada, pintado, diálogos, tareas de archivo)
 ├─ score-toolbar (modelo de presentación de la cinta y Archivo; sin GPUI)
 │   └─ score-keymap
 ├─ score-keymap (comandos, perfiles y atajos; sin GPUI)
 ├─ score-document (sesión y estado guardado)
 │   ├─ score-edit (comandos transaccionales e historial)
 │   │   └─ score-core (modelo e invariantes)
 │   └─ score-layout (geometría y caché por compás)
 │       └─ score-core
 └─ score-io (esquema nativo versionado y escritura atómica)
     └─ score-core

La interfaz también coordina `score-playback` (planes inmutables y tempo) y
`score-platform` (sintetizador, cola y salida del dispositivo). El hilo de audio
nunca recibe el documento editable ni una referencia a GPUI.
```

GPUI se usa exclusivamente en `crates/app-gpui`. Se elimina la dependencia
`score-io → score-playback`: cuando llegue MIDI, se definirá un intercambio de
eventos que no requiera cargar el motor de reproducción.

## Decisiones implementadas

- **Tiempo musical entero.** La redonda ocupa 192 unidades, evitando errores de
  coma flotante. Las duraciones actuales llegan a semicorchea; tuplets y puntillos
  necesitarán ampliar esta representación antes de implementarse.
- **Identidad.** `slotmap 1.1.1` asigna claves generacionales dentro de cada compás.
  La referencia completa es `(ID de parte, pentagrama, índice de compás, clave)`. Las partes
  tienen IDs persistentes que no cambian al quitar otra parte. Agregar compases al final
  y borrar notas conserva las referencias de los otros elementos. Insertar,
  borrar o reordenar compases requerirá identificadores estables para compases.
- **Instantáneas.** `imbl 7.0.1` comparte la secuencia de compases y sus índices.
  Cada compás comparte su almacén de eventos mediante `Arc`; al editar se copia
  solo ese pequeño almacén. Deshacer restaura partitura, cursor y selección.
  El historial vive en memoria y se libera al abrir/crear un documento.
- **Comandos atómicos.** Un error no publica cambios ni altera el historial.
  Editar después de deshacer descarta la rama de rehacer.
- **Layout.** Geometría independiente, medidas en unidades lógicas, hasta cuatro
  compases por sistema y anchuras mínimas según densidad. Espaciado inicial
  proporcional a la raíz de la duración. Una caché compara contenido y anchura
  de cada compás; un cambio de reparto puede invalidar otros compases.
  El agrupado y la colocación de sistemas aún recorren la partitura completa.
  Ligaduras y reguladores localizan sus notas en un índice construido una vez por
  layout, no recorriendo todos los compases. `Document` recuerda la última vista
  (partitura, anchura, modo y escala) y la comparte sin copiarla mientras no
  cambien: la reproducción y el puntero no vuelven a grabar la obra. La posición
  de reproducción se calcula una vez por fotograma y la usan regla, transporte,
  barra de estado y lienzo. El lienzo solo pinta lo que alcanza la
  zona visible (compases, ligaduras, reguladores, etiquetas, llaves y conectores).
  Referencia: la orquesta de 32 partes y 84 compases dibuja a 60 fps también con
  `cargo run`, cuyo perfil optimiza ligeramente el workspace y del todo las
  dependencias.
- **Tipografía.** Bravura 1.392 y sus metadatos están incluidos con la OFL.
  Grosores, anchura de cabezas y extensión de líneas adicionales vienen del JSON.
  `ttf-parser 0.25.1` extrae y cachea los contornos originales SMuFL, que se pintan
  como trazados GPUI con relleno de winding no nulo. Esto evita una limitación de
  GPUI 0.2.2 en macOS: rechaza fuentes que no contengan la letra latina `m`.
  Sus glifos compuestos resuelven plicas y banderas iniciales. Cada glifo se tesela
  una vez por tamaño y orientación con el mismo lyon y opciones que GPUI; pintar una
  instancia solo traslada sus triángulos.
  La cinta y el inspector comparten estos contornos para dinámicas, alteraciones,
  articulaciones, figuras, puntillo, tresillo, calderón y reguladores. Los controles
  centran la caja de tinta real (incluidas las dinámicas compuestas), conservan los
  atajos en la fuente de interfaz y usan cursiva editorial para `pizz.` y `arco`.
  Los menús nativos de macOS usan el mismo archivo Bravura para dinámicas,
  articulaciones, reguladores y alteraciones (incluidas las de las tonalidades).
  Sus títulos combinan texto nativo con glifos de caja de tinta acotada para evitar
  las grandes métricas verticales de SMuFL. Los símbolos se dibujan a la resolución
  de pantalla y toman el color del estado del menú; las acciones, atajos y etiquetas
  accesibles conservan sus nombres originales. El formato se reaplica al cambiar
  las asignaciones de teclado.
  El repertorio de `music_font` es común al grabado, la UI y los menús. Los textos
  mixtos convierten alteraciones Unicode a glifos Bravura y heredan tamaño y color
  del texto; letras, nombres, números de voz y términos como `pizz.` mantienen su
  tipografía de texto. Las llaves usan las variantes originales de Bravura y los
  corchetes sus terminales; líneas extensibles, ligaduras y reguladores usan las
  métricas de grabado de la fuente. Las pruebas recorren todas las marcas, las
  30 tonalidades, cada grupo musical de la cinta y los ejemplos de partituras
  para detectar símbolos sin contorno cargado o sustituciones por iconos genéricos.
- **Archivos.** `.partitura` contiene JSON con `format`, `version` y `score`.
  Se valida el esquema y las invariantes antes de abrir; versiones desconocidas
  se rechazan. Escritura temporal en el mismo directorio, sincronización del
  archivo y reemplazo atómico. Límite inicial: 16 MiB y 4096 compases.
  El esquema 7 añade marcas por evento y ligaduras con extremos estables al esquema 6,
  que guarda partes, instrumentos, pentagramas, conjuntos de alturas, alteraciones y
  una tonalidad inicial. El esquema 11 añade mapas de cambios de métrica y tonalidad
  por índice de compás; los esquemas anteriores conservan sus valores iniciales sin
  cambios intermedios. El esquema 12 añade tempo inicial y marcadores por compás;
  los anteriores usan negra = 120 sin cambios. Los formatos 1–6 se abren sin marcas de expresión; 1–5 usan
  Do mayor para conservar el sonido previo. El formato 4 migra con alturas naturales.
  El esquema 3 migra cada nota a una lista de una altura y cada silencio a una
  lista vacía, conservando sus IDs. Al abrir el esquema 1
  se crea una parte de piano; al abrir el 2 se conserva cada instrumento. Ambos
  mantienen las notas originales de piano en `Upper` y añaden `Lower` vacío.
  El esquema 15 añade el segundo orden de eventos por compás y la división de partes;
  los esquemas 1–14 se abren con una voz y sin división. La migración no modifica
  el archivo original. ZIP sigue pendiente.
- **Interfaz.** Controles pequeños con primitivas GPUI; `gpui-component` queda para
  cuando hagan falta paneles, formularios y paletas. Guardar/abrir usa diálogos
  nativos y realiza la lectura/escritura fuera del hilo de interfaz. Mientras hay
  una operación pendiente se bloquea la edición para guardar una versión coherente.
- **Recuperación.** `score-io::Recovery` posee un escritor serial y un bloqueo
  del sistema sobre el directorio de sesión. Las ediciones encolan instantáneas
  compartidas; debounce de 750 ms con máximo de 3 s. Escribe un sobre versionado
  con documento nativo y ruta original mediante temporal, sincronización y
  reemplazo atómico. Guardar/descartar encola la limpieza en el mismo escritor;
  ninguna escritura anterior puede resucitar una copia descartada. El callback
  de aplicación conserva un handle y espera al escritor al salir del Dock.
  Al iniciar se admite también la antigua recuperación nativa sin sobre y se
  restaura como documento modificado. Los errores se muestran en la interfaz;
  el archivo original solo cambia al guardar explícitamente.
- **Desarrollo macOS.** La característica `runtime-shaders` está activa por defecto
  porque el equipo tiene Xcode pero no Metal Toolchain. La compilación anticipada
  queda disponible con `--no-default-features --features vsco` al instalar ese
  componente. La característica `vsco` (adaptador de VSCO CE) también está activa
  por defecto; sin muestras instaladas no aparece y no cambia el sonido.

Las versiones se fijan con `Cargo.lock`. Los objetivos de latencia de
`herramientas.md` siguen siendo propuestas: todavía no están medidos.

## Partitura de conjunto

`Score` conserva una secuencia de partes con identidad estable e instrumento.
Cada parte tiene pentagramas con identidad `Upper`/`Lower`; el piano conserva los
dos y los demás instrumentos solo `Upper`. Todos tienen la misma cantidad de
compases. Los IDs de nota incluyen el pentagrama para distinguir claves locales
idénticas en las dos manos. Añadir o quitar una parte es una edición transaccional,
con cursor, selección e historial. La interfaz permite elegirla por el panel,
por clic en sus notas/pentagrama y por ⌥↑/⌥↓ (pentagrama vecino). Al cambiar de parte se muestra su
pentagrama. No se permite quitar la última parte.

El grabado comparte una rejilla de comienzos y finales musicales por compás entre
todas las partes. Las notas que empiezan en el mismo instante se alinean por tiempo,
no por índice de nota; también se centran las cabezas de distinta anchura. Las claves de sol, do y fa sitúan las alturas reales, con
nombres a la izquierda y una línea que une los pentagramas de cada sistema. El
ancho se adapta a la ventana; las partes de un compás conservan la misma anchura.
La caché incluye ID de parte, contenido, clave, rejilla y anchura. Cambiar una
altura no recalcula las demás partes; cambiar el ritmo puede afectar la rejilla
compartida. El gran pentagrama de piano tiene claves de sol/fa, llave y barras
unidas; su caché incluye la identidad del pentagrama. Las armaduras se calculan por parte mediante `written_key_at`.

El plan de audio mantiene intervalos por pentagrama, canales por instrumento y un
reloj común. Las dos manos comparten canal y timbre. Los unísonos superpuestos se
convierten en una sola pulsación hasta el último final, evitando ataques dobles y
NoteOff de una mano que corte la otra. La búsqueda de notas sostenidas se prepara
en el control y se entrega en un array fijo de hasta 448 notas (28 pentagramas × 2 voces × 8 alturas); no recorre la
partitura en el callback. Las notas restauradas se deduplican por canal/tecla.
La polifonía se reserva al crear el sintetizador para 256 voces, el máximo de RustySynth, incluyendo capas
de muestras y colas de liberación; con grandes conjuntos puede haber robo de voces. La duración total es el máximo de los finales de todas las partes. Al
iniciar desde una selección, también se atacan las notas de otras partes que
atraviesan ese instante y se liberan en su final original. Esto conserva su
duración restante; aún no reconstruye el punto previo de la envolvente del sonido.
Se resaltan simultáneamente las notas de todas las partes. El seguimiento muestra
el conjunto si cabe verticalmente; en partituras altas sigue la parte desde la que
se está trabajando y usa otra parte activa durante sus huecos. Antes de seguir
un único pentagrama intenta mantener juntas las dos manos del piano activo. El panel y el clic
actualizan la parte que se sigue sin reiniciar el transporte.

## Acordes

`Event` conserva una duración y `Pitches`, un conjunto ordenado de hasta ocho
posiciones diatónicas distintas en almacenamiento fijo. Una lista vacía representa
un silencio. Añadir o quitar la altura más aguda conserva el ID, la duración y el
cursor; los comandos son transaccionales y reversibles. La transposición afecta
al acorde entero. La selección del evento conserva un índice de altura en el historial para
aplicar alteraciones a una nota individual. Cada pentagrama admite dos voces rítmicas independientes.

El grabado usa cabezas SMuFL independientes, una plica común y bandera según
duración. Las segundas alternan el lado de las cabezas y reservan espacio en la
rejilla compartida. La selección y el seguimiento abarcan todas sus cabezas.
El plan programa cada altura en el mismo instante y conserva la deduplicación de
unísonos entre manos. La audición entrega hasta ocho teclas en un comando fijo;
el inicio desde selección restaura todas las alturas sostenidas sin asignaciones
en el callback. El formato 10 abre también los formatos 1–9.

## Alteraciones

`Pitch` conserva su escritura diatónica y un `Option<Accidental>`: sostenido,
bemol o becuadro. `None` sigue la armadura vigente; `Natural` fuerza la altura natural con becuadro
explícito, incluso si la armadura altera ese grado. La conversión MIDI aplica ±1 semitono;
la transposición diatónica conserva la alteración. Los acordes permiten una sola
nota por posición diatónica y admiten enarmonías entre posiciones distintas.
Los unísonos enarmónicos se deduplican tanto en el plan como en la audición.

El grabado resuelve signos por compás y pentagrama. La altura efectiva se resuelve con la armadura y el signo propio. Después de una
alteración individual se imprime la restitución apropiada si la siguiente nota
vuelve a la armadura en esa posición. Los signos explícitos se conservan incluso si son reiteraciones.
Las columnas dejan separación vertical de 35 unidades y la rejilla reserva
espacio antes de cada comienzo, también delante del primero. El seguimiento
incluye sus glifos. La selección distingue cabezas por su altura y marca la nota
que recibe la alteración, sin cambiar la selección rítmica de reproducción.
El selector de entrada es independiente del comando que altera la nota elegida.

## Métrica y armadura por compás

`KeySignature` valida de −7 a 7 quintas y distingue modo mayor/menor; las relativas
comparten alteraciones. El valor por defecto es Do mayor. Un signo propio de
`Pitch` prevalece sobre la armadura; `None` hereda el grado en todas las octavas.
Cambiar la tonalidad reinterpreta las notas que heredan, conserva las escrituras
explícitas y no transpone sus posiciones. `SetKeySignatureAt` y `SetTimeSignatureAt`
participan en el historial sin modificar IDs, cursores, selección ni índice de nota
del acorde. Los mapas privados `key_changes` y `time_changes` guardan marcadores
posteriores al compás cero; `key_signature_at` y `time_at` resuelven la herencia.
Cada cambio afecta a todas las partes hasta el siguiente marcador. El compás cero
conserva los campos iniciales de los archivos anteriores.

`measure_start_tick`, `measure_at_tick` y `total_ticks` comparten el cálculo de
posiciones con el editor, portapapeles, audio y transporte. Un cambio de métrica
valida la capacidad en cada pentagrama antes de modificar el documento. Si algún
compás rebasa la duración nueva, falla de forma atómica. Las conexiones se
revalidan al cambiar el ritmo o la altura efectiva de sus extremos. Al pegar,
las notas se dividen según la métrica local y conservan su sonido entre armaduras.

`Pitches::in_key` resuelve una copia fija sin asignaciones. El plan de audio
resuelve tanto sus mensajes como los intervalos sostenidos; la audición aplica
el mismo método antes de convertir a MIDI. El callback recibe alturas preparadas.
La notación empieza cada compás con el estado de la armadura y dibuja los signos
de restitución cuando cambia una alteración propia. La clave y armadura se repiten
al comenzar cada sistema. El espacio reservado precede a la indicación de compás
y a la primera nota, y la caché incluye la tonalidad incluso al cambiar entre
dos armaduras de igual anchura. Los cambios intermedios reservan espacio para
la nueva indicación y los becuadros de cancelación de la armadura anterior, incluso
al coincidir con un salto de sistema. Se admiten sol, do y fa en altura real.

La interfaz ofrece 15 tonalidades por modo, un selector en el panel y un menú
nativo. **Armadura** es la entrada sin alteración propia; **Seguir armadura** la
restaura en la nota elegida. Los nombres de las notas seleccionadas muestran su
altura efectiva. Ambos selectores permiten aplicar desde el inicio o desde el
compás elegido y quitar un marcador. `T` abre la métrica y `K` la tonalidad. No hay
armaduras diferentes por parte ni elevaciones automáticas de menor armónica/melódica.

Las pruebas de cambios de firma cubren herencia, cambios iniciales que preservan
marcadores posteriores, rechazo atómico de compases demasiado llenos, historial,
ligaduras entre tonalidades, portapapeles, compatibilidad del esquema 10, grabado
en ambas manos y saltos de sistema, tiempos de audio con calderón y búsqueda en
un último compás de 1/16. La comprobación en ventana confirmó el panel de métrica
y la armadura inicial; el control automatizado de clics fue intermitente y no
permitió completar la secuencia manual de cambios intermedios.

## Indicaciones de tempo

`score-core::TempoMark` conserva texto de una línea (máximo 80 caracteres), figura
de referencia y BPM (20–400). `TempoBeat` admite blanca, negra y corchea, con o sin
puntillo. El tempo inicial y `tempo_changes` se heredan por compás en todas las
partes. `SetTempoAt` y `RemoveTempoChange` son transacciones del historial que
conservan notas, IDs, cursor y selección. El esquema 12 valida y conserva estos
marcadores; los esquemas anteriores se abren con negra = 120.

El grabado reserva una franja superior y anchura para cada indicación explícita.
`Layout.tempos` queda fuera de la geometría local en caché; sus posiciones siguen
las traslaciones de los sistemas y páginas. La figura se pinta con Bravura; el
texto conserva tipografía de texto y distribuye sus posibles símbolos musicales
al mismo repertorio Bravura. El hit testing permite abrir el selector desde la
indicación escrita.

`Timeline` integra segmentos de segundos por tick, separados en cambios de tempo
y extremos de calderones. `frame_at`, `seconds_at` y `tick_at` comparten este mapa.
La figura de referencia fija la duración de cada pulso; el argumento `Tempo` de
la API de audio actúa como velocidad relativa (120 = respetar la escritura).
Los reguladores se subdividen en esas mismas fronteras para conservar su pendiente
musical. El callback de audio sigue recibiendo un plan preparado sin asignaciones.

La cabecera y el transporte abren el compás mostrado; Texto → Tempo abre el
compás del cursor. El selector fija ese destino mientras se escribe, y permite
cambiar al inicio sin perder el destino original. Los botones −/+ editan el tempo
mostrado y conservan el tick al detener el plan anterior. Los campos de texto
usan el manejador nativo de GPUI con selección, portapapeles, IME y desplazamiento
horizontal; sus teclas no ejecutan comandos de escritura musical.

En una sesión gráfica aislada se escribió «Andante cantabile» y 84 BPM en los
campos nativos, se aplicó con Enter y se comprobó la indicación Bravura sobre el
pentagrama. Se guardó un archivo temporal y se verificaron su versión 12, texto,
figura y BPM. Las pruebas cubren reapertura y compatibilidad, cambios por compás,
historial, figuras con puntillo, calderones, inversiones del reloj y crescendos
sobre notas ligadas que atraviesan distintos tempi.

Para pruebas gráficas aisladas, `LEVARE_DATA_DIR` puede indicar un directorio
absoluto alternativo para recuperación, recientes y atajos, sin cambiar archivos
de partituras ni los datos de la sesión habitual. `PARTITURA_DATA_DIR` se conserva
como alias para los scripts anteriores.

## Reproducción integrada

La segunda entrega permite escuchar la escritura y reproducir la partitura:

- **Entrada:** después de una inserción o transposición válida suena una audición
  de 250 ms con el instrumento activo; conserva su cola natural de liberación. Silencios y comandos
  rechazados no disparan notas. Se puede desactivar la audición.
- **Transporte:** Espacio o el botón inicia desde la nota/silencio seleccionado,
  o desde el comienzo si no hay selección. Espacio alterna pausa/continuación;
  Esc o Detener cancelan el transporte conservando la selección. Mientras está
  detenido, Esc quita la selección. Editar, deshacer, cambiar tempo, abrir o crear
  un documento detiene el plan anterior y descarta la pausa.
- **Pausa y selección:** los intervalos del plan conservan posiciones absolutas.
  El inicio seleccionado se busca antes de encolar Play; el callback omite los
  mensajes anteriores a esa frontera. Durante la pausa emite ceros y congela
  tanto el reloj musical como el estado completo del sintetizador, incluido su
  bloque de muestras, fase y envolventes. La continuación no necesita reconstruir
  el plan ni volver a disparar la nota. La pausa también funciona si Play todavía
  está en cola. Stop invalida el transporte pausado, incluso con la cola llena.
  Cambiar la selección en pausa no mueve su posición: para saltar a otra nota
  se detiene y se inicia otra vez.
- **Ritmo:** `score-playback` convierte las unidades musicales a posiciones de
  muestras mediante aritmética entera, usando negras por minuto. Mantiene los
  huecos de compases incompletos interiores y omite los compases vacíos finales.
  Incluye intervalos para resaltar notas y silencios sin alterar la selección.
  En una misma frontera temporal, NoteOff precede a NoteOn.
- **Sonido:** `rustysynth 1.3.7` y el banco original GeneralUser GS 2.0.3,
  revisión y SHA-256 en `assets/sounds/README.md`. Se conserva su licencia propia.
  El catálogo de 49 perfiles selecciona programas General MIDI y notas de percusión.
  Hasta 48 partes usan rutas independientes de 128 canales internos, repartidas
  en ocho sintetizadores preasignados. Se omiten los canales locales 9/15 para
  las partes; el canal anfitrión 15 queda para audición. MIDI externo sigue pendiente.
- **Dispositivo:** `cpal 0.16.0` abre la salida predeterminada y adapta la mezcla
  estéreo a su frecuencia y formato. Soporta flotantes de 32/64 bits y enteros
  de 16/32 bits, mono/estéreo y canales adicionales silenciados. Sin dispositivo
  o tras un error, la edición continúa y se ofrece reconectar el audio.
- **Tiempo real:** `rtrb 0.4.0` transporta comandos en una cola fija. El callback
  posee su sintetizador y buffers; no bloquea, abre archivos ni hace trabajo de
  interfaz. Los planes se preparan fuera del callback y el control conserva una
  referencia fuerte hasta que el audio libera la suya, para que la destrucción
  de los vectores también ocurra fuera del callback. Los propietarios se deduplican
  cuando se vuelve a usar el mismo plan. Stop cambia una generación atómica y
  cancela incluso los inicios que aún están en una cola llena.
- **Comprobación:** `assert_no_alloc 1.1.2` intercepta asignaciones y liberaciones
  en los tests del callback real con el banco incluido: reproducción, audición,
  finalización, planes obsoletos, saturación, órdenes concurrentes y conversión
  de salida. También compara muestra por muestra la continuación de una nota
  pausada contra una reproducción sin interrupciones, y comprueba la pausa
  antes del primer callback, dentro de un silencio y su cancelación por Stop.
  El ejemplo `audio_smoke` verificó la salida real a 44 100 Hz, el inicio desde
  una nota seleccionada, el reloj congelado durante la pausa, su continuación
  y finalización. No se han medido aún latencia física ni underruns.
- **Controles:** 120 BPM iniciales, rango 30–240, volumen y audición configurables
  durante la sesión. Cambiar tempo detiene el plan; cambiar volumen se aplica en
  el siguiente bloque. Estas opciones aún no forman parte del archivo nativo.

La vista sigue automáticamente la nota o silencio activo en ambos ejes, con margen
para el resaltado, las plicas y las banderas. Solo desplaza el lienzo cuando el
elemento sale del área visible. Usa el origen real pintado del lienzo y el área
del contenedor desplazable, por lo que incluye el título, el relleno y los cambios
de tamaño sin depender de alturas fijas. Compensa cambios de desplazamiento entre
fotogramas para evitar avances duplicados. Pausar o detener libera el desplazamiento
manual; continuar vuelve a mostrar la nota activa. Este seguimiento no modifica la
selección ni el documento. Los tests cubren renglones sucesivos, retorno a una nota
anterior, compases anchos, alturas extremas, silencios y límites de la vista.

La interfaz consulta el reloj de muestras cada 16 ms para actualizar el resaltado
y el seguimiento de la vista. Ese temporizador no programa ni genera audio.

`app-gpui::playhead` convierte ese reloj absoluto a ticks, compás y tiempo para
compartir una posición entre partitura, regla y contador. El layout conserva la
rejilla rítmica compartida e interpola entre sus anclas, también en huecos sin
eventos. El cabezal y su línea fina se pintan antes de la tinta musical y reemplazan
los dos indicadores anteriores. El seguimiento horizontal sigue esa posición;
la pausa conserva el reloj y permite desplazamiento manual. Las notas y silencios
activos cambian de color sin fondos individuales; las plicas identifican su evento
para compartir el color, mientras pentagramas y líneas adicionales quedan neutros.
La regla comparte su geometría de ocho píxeles de margen con la detección de clics
y ajusta la posición al tiempo de denominador más cercano. El último extremo elige
el último tiempo reproducible. `Playback` conserva ese tick al estar detenido sin
alterar el cursor de edición ni la selección; el plan del transporte incluye los
silencios implícitos hasta el final de la regla. `Control::play_from_frame` utiliza
el mismo inicio preparado que la reproducción desde selección, recuperando las
notas sostenidas y la expresión. Un salto mantiene el estado de reproducción o pausa;
detener o editar descarta la posición elegida en la regla.
La liberación natural del piano
puede continuar brevemente cuando termina la última figura. Los eventos se
entregan al sintetizador en las fronteras calculadas, con la granularidad interna
de su bloque de 64 muestras.

## Duraciones con puntillo y tresillo

`Duration` contiene `NoteValue` (redonda a semicorchea) y dos modificadores booleanos:
un puntillo y tresillo 3:2. La redonda ocupa 192 ticks y la negra 48, por lo que
ambas modificaciones y su combinación se resuelven sin fracciones ni redondeo.
El valor mínimo es 8 ticks (semicorchea de tresillo). La capacidad máxima de 16/1
permite 384 eventos y el límite de validación se deriva de estos valores.
Los fotogramas se calculan desde el tick absoluto, evitando deriva acumulada en
cadenas largas de tresillos. Las partituras anteriores conservan sus tiempos reales.

Las duraciones sencillas mantienen su cadena JSON anterior; las modificadas usan
`{value, dotted, triplet}` con campos y tipos validados. El esquema 10 abre 1–9;
el formato nativo no guarda ticks, de modo que cambiar la resolución no requiere
reescribir compases, enlaces ni IDs. El portapapeles 4 sí convierte los offsets de
1–3 multiplicándolos por tres; esos formatos antiguos rechazan modificadores nuevos.
La capa de aplicación normaliza al leer y el editor también acepta pasajes antiguos.

`SetDotted` y `SetTriplet` actúan sobre todo el rango seleccionado de forma atómica,
conservan el rango y actualizan la capacidad mediante `Score::set_duration`.
Los ajustes de entrada conservan los modificadores cuando cambia la figura base.
El pegado mantiene las figuras sin dividir; al cruzar barras usa una tabla acotada
de descomposición para figuras rectas o de tresillo compatibles, con uniones y
marcas en sus extremos correspondientes. Los huecos usan la paleta completa.
Una frontera no representable produce un error y no deja eventos ni compases
parciales. No se usan duraciones ocultas ni redondeos para hacer caber el pasaje.

El grabado añade glifos Bravura de puntillo y número 3. Los puntos de acordes usan
una columna con espacios verticales distintos, sin caer sobre líneas; el espaciado
reserva su anchura. Los grupos locales reúnen hasta tres figuras consecutivas del
mismo valor modificado; un compás nuevo o un cambio de figura los separa y los grupos
incompletos permanecen visibles. Los corchetes se sitúan sobre la tinta original de
las notas y marcas, sin realimentarse con corchetes anteriores. Se conservan huecos
entre grupos densos, límites de tinta, caché, hit testing, zoom y seguimiento.
No se modelan aún grupos anidados ni tresillos con figuras de distinto valor.

Validación de esta entrega: 149 tests aprobados del workspace, formato y Clippy sin
errores. Se comprobaron ratios exactos, historia atómica, conservación de rangos,
acordes/silencios, cadenas ligadas, ausencia de deriva en 96 compases de tresillos,
portapapeles antiguo/nuevo, fronteras no representables sin efectos parciales,
codec de duraciones y guardado del compás máximo de 384 eventos. Los tests de
grabado incluyen puntos de acordes con segundas y ocho grupos densos sin escalones
ni contacto entre corchetes. En una aplicación temporal sin recuperación de sesión
se verificaron el dibujo, los atajos de puntillo y tresillo, el menú, deshacer/rehacer
y el archivo guardado en esquema 10. La prueba CoreAudio a 44.100 Hz alineó dos
compases mixtos con la mano izquierda, con pausa/continuación. La revisión final
no dejó problemas importantes.

## Barras de unión automáticas

`score-layout::beaming` deriva los grupos del ritmo escrito y la métrica, sin
modificar el modelo, el esquema nativo 10, el portapapeles 4 ni el plan de audio.
Cada pentagrama se procesa por separado. Los compases simples agrupan por unidad
del denominador; los compuestos, por tres unidades. Los compases cortos irregulares
usan 5/8 = 3+2 y 7/8 = 2+2+3; la edición manual de estas subdivisiones queda pendiente.
Los silencios, figuras largas, notas que atraviesan un pulso y límites de compás
interrumpen las barras. Los tresillos reutilizan los grupos de figuras iguales
del grabado rítmico y nunca se unen con el grupo contiguo.

El plan fija una dirección de plica común antes de desplazar las segundas de los
acordes. El grabado sustituye los glifos completos por cabezas y plicas que llegan
a una barra de pendiente limitada, dejando espacio sobre todo el acorde. Las
semicorcheas generan barras secundarias completas o parciales. GPUI pinta los
polígonos con los colores de selección y reproducción; los límites de tinta,
articulaciones, dinámicas, ligaduras y seguimiento incluyen las nuevas plicas y
barras. Un tresillo completo con una sola barra común muestra el número 3 sin
corchete redundante; los grupos incompletos o con silencios conservan el corchete.

La caché existente ya incluye figuras y métrica, por lo que una edición recompone
el grupo correspondiente. Los tests cubren ambos pentagramas, compases simples,
compuestos e irregulares, ritmos mixtos con puntillo, silencios, tresillos, acordes
con segundas, claves sol/do/fa, ligaduras, selección y seguimiento con zoom.
El ejemplo `examples/barras.partitura` reúne estos ritmos para inspección visual.

Validación: 157 tests del workspace aprobados, formato y Clippy sin errores.
La revisión independiente no encontró problemas importantes. En una aplicación
temporal sin recuperación de sesión se comprobó el dibujo de corcheas,
semicorcheas, ritmos mixtos con puntillo y tresillos con barras simples y dobles,
incluyendo plicas ascendentes y descendentes y ambas manos del piano.

## Reguladores y curvas de dinámica

`ConnectionKind::{Crescendo, Diminuendo}` comparte los extremos estables del
modelo de ligaduras. Admite notas, acordes o silencios de un mismo pentagrama,
ordenados en el tiempo; los intervalos de reguladores no se pueden solapar y
pueden tocarse. Borrar un extremo elimina el enlace; el historial lo restaura.
El esquema 9 conserva estos tipos y migra desde 8 sin cambiar las conexiones.
Los reguladores se incorporaron en el portapapeles 3; el formato 4 actual sigue
leyendo 1–3, copia reguladores internos y remapea
su final al primer fragmento si la nota final se divide al pegar. Los reguladores
externos no se copian, como las ligaduras; su dinámica inicial se aproxima a la
marca más cercana. El contexto al insertar usa el límite izquierdo de la curva.

`score-core::DynamicEnvelope` prepara segmentos lineales por pentagrama entre
las marcas escritas. Sin marca de llegada, el objetivo es dos posiciones de
`Dynamic::ALL` por encima/debajo de la última marca del regulador, limitado a
ppp–fff. El nivel final persiste. Las marcas interiores son anclas y las marcas
explícitas prevalecen sobre la dirección gráfica.

`score-playback::dynamics` calibra la velocidad de cada ataque al máximo de su
intervalo sonoro y calcula CC11 como `nivel / referencia * 127`. Fuera de las
curvas, esto conserva exactamente la velocidad y la expresión completa anteriores.
Un crescendo sostenido reserva solo la amplitud que necesitará antes de liberar la
nota; las articulaciones mantienen su refuerzo. Los tiempos de cada transición de
CC se obtienen invirtiendo la pendiente original, antes de limitarla a 127, para
que los silencios y gates de staccato no estiren artificialmente la curva.
El plan recupera expresión actual y ataque original al empezar dentro de una unión.
Pausa congela la curva, las envolventes y el reloj del sintetizador.

Los dos pentagramas de piano, arpa, celesta y órgano con reguladores usan rutas
internas separadas. Cada grupo de 14 partes ocupa 32 canales: la ruta principal
salta el canal local 9 y la inferior añade 16. Ocho bancos cubren las 48 partes
sin reutilizar canales. Los bancos comparten reloj, se reinician
al detener y se mezclan sin reservar memoria en el callback. Al comenzar un plan
se restaura CC11 de todas las rutas para evitar volumen residual de otra partitura.

El grabado produce dos trazos por segmento de regulador, con abertura continua
entre sistemas, por debajo de notas, ligaduras y dinámicas. La reserva de espacio
se calcula por fila sin invalidar el grabado local de compases. El seguimiento
incluye el regulador de la nota activa y funciona con el zoom. Cinta, inspector,
menús y los atajos H/Shift+H comparten comandos; se puede crear desde una selección
de pasaje o marcando inicio/final en dos pasos.

Validación de esta entrega: 139 tests aprobados del workspace, formato y Clippy sin
errores. Los tests verifican audio de acordes sostenidos y ligados, crescendos y
diminuendos, restitución del volumen y timbre fuera de la curva, referencias de
ganancia a través de silencios/staccato, inicio desde selección, pausa exacta y
28 rutas de manos en 14 pianos. En la aplicación temporal sin recuperación de
sesión se comprobaron los trazos, creación en dos pasos con H, creación sobre un
pasaje con Shift+H, eliminación, deshacer/rehacer y guardado; el archivo conservó
los extremos en esquema 9. La prueba CoreAudio a 44.100 Hz reprodujo violonchelo
ligado con crescendo, diminuendo y pausa/continuación. La revisión final no dejó problemas importantes.

## Técnicas de cuerda

`Notation.technique: Option<PlayingTechnique>` guarda `Pizzicato` y `Arco` en notas,
acordes o silencios. Solo los instrumentos de la familia `Strings` aceptan estas
marcas. El esquema 8 conserva la técnica; la migración desde 7 deja las marcas
vacías y mantiene el sonido original. El setter y la validación de archivos e
inserciones comparten esta restricción. El historial conserva cada edición.

El plan resuelve la técnica por pentagrama y prepara cambios de programa antes de
los ataques: pizzicato usa el preset GM 45 (base cero), arco restaura el preset del
instrumento. Los cambios no consumen canales adicionales. Las notas ligadas
conservan el programa y la velocidad de su ataque original, incluidos los inicios
desde selección. Los mensajes se ordenan liberación → programa → ataque. El callback
solo recibe datos preparados y no reserva memoria. La audición usa la técnica
vigente. Los tests comprueban el preset real del banco, diferencias de sonido,
continuación exacta tras pausa, aislamiento por parte y las cuatro cuerdas.

El grabado utiliza etiquetas de texto serif en cursiva, con espacio horizontal y
límites de tinta propios por encima de plicas y articulaciones. Las ligaduras de
expresión siguen el contorno exterior de estas etiquetas. El portapapeles versión
2 conserva la técnica heredada y coloca la marca únicamente en la primera figura
cuando se divide al pegar; sigue leyendo la versión 1 con arco por defecto. Un
pegado en un instrumento incompatible falla antes de modificar el documento.

Validación de esta entrega: 127 tests del workspace, Clippy y formato sin errores.
En una aplicación temporal sin recuperación de sesión se comprobó el cuarteto,
las etiquetas en cursiva y el cambio a arco desde el menú nativo; el archivo guardado
conservó `Arco` en esquema 8. La salida CoreAudio se comprobó a 44.100 Hz con
pizzicato de violonchelo, pausa/continuación y regreso a arco. La revisión de código
no encontró problemas importantes ni críticos.

## Portapapeles musical y selección de pasajes

`score-edit::selection` mantiene un ancla y un foco estables por pentagrama dentro
de la instantánea del editor. Clic y navegación normal vuelven a una selección
simple; Shift+clic/flechas amplía el intervalo y Seleccionar todo recorre el
pentagrama activo. Cortar y Eliminar notas y su tiempo quitan el intervalo; Delete
convierte las notas en silencios conservando el tiempo. El historial
restaura el ancla, el foco y los eventos.

`app-gpui::selection` convierte un arrastre de al menos cinco píxeles en el mismo
intervalo del editor. Puede comenzar en una nota o en espacio de un compás escrito;
un clic breve conserva la selección individual y el cursor de entrada anteriores.
El destino se aproxima por renglón y tiempo dentro del pentagrama de origen,
independientemente de la altura de las notas o de otros instrumentos. El zoom y el
origen desplazado del lienzo se consideran al buscar extremos. Soltar el botón o
ejecutar un comando termina el gesto. La pintura reúne compases contiguos en una
franja por renglón, incluidos compases vacíos intermedios, y la barra de selección
muestra el recuento con acciones Copiar/Cortar. Los menús incluyen las acciones
de portapapeles y Seleccionar todo con sus atajos actuales.

`score-edit::clipboard` expone un pasaje portable versionado: eventos con offsets
en ticks, armadura de origen, dinámica vigente e índices de ligaduras internas.
No contiene IDs del documento. El pegado clona la instantánea, conserva las alturas
al cambiar de tonalidad, materializa huecos como silencios y divide las figuras
que cruzan barras creando uniones. Los extremos de las ligaduras se remapean a
las figuras nuevas. Las dinámicas/acento quedan al inicio de una figura dividida;
staccato/tenuto se colocan al final. La reproducción aplica el gate final a la
cadena completa para conservar el sonido original. Solo se confirma la operación
cuando toda la música y sus enlaces son válidos; una colisión de capacidad o enlace
no deja compases añadidos ni ediciones parciales.

La capa GPUI usa `ClipboardItem` con resumen textual y metadatos JSON propios,
limitados a 16 MiB y validados antes del pegado. No conserva un portapapeles interno
que pudiera pegar datos antiguos después de copiar texto de otra aplicación.
Cortar verifica la lectura del contenido escrito antes de borrar las notas.
Las acciones comparten registro, menús, cinta y atajos configurables. No actúan
sobre la partitura mientras Preferencias está abierto.

## Orden y agrupación del score

`InstrumentFamily` y `Instrument::score_order` comparten el criterio de presentación
entre editor, panel y grabador. `Score::arrange_parts` ordena partes completas de
forma estable: conserva IDs, voces de ambas manos, eventos y conexiones. La sesión
normaliza el orden al abrir y la edición lo aplica al añadir instrumentos. La
instantánea guardada usa ese mismo orden para evitar un indicador de cambios falso.
No se modificó el esquema de archivo: la secuencia de partes ya era serializable.

El grabador calcula los límites de cada familia por sistema después de colocar
los pentagramas. Genera un corchete por grupo y prolonga las barras de compás solo
dentro de esa familia; la barra inicial recorre el sistema. Las llaves de piano se
mantienen separadas de los corchetes. Los contornos se pintan con coordenadas de
presentación escaladas y las posiciones de las notas conservan su hit testing.

Eliminar desde el panel ejecuta el mismo comando registrado que el menú nativo.
Se conserva al menos una parte y se selecciona la vecina al eliminar. El historial
recupera la parte completa y sus ligaduras. La reproducción se detiene al editar
y reconstruye su mapa de canales/timbres desde el nuevo orden.

## Interfaz de referencia

`app-gpui::ui` concentra la presentación: paleta oscura, componentes comunes,
cinta por pestañas, lista de instrumentos por familia, inspector, hoja y
transporte. `ui::icons` dibuja los iconos como vectores nativos. El archivo
`main.rs` conserva las acciones, los diálogos, el estado del documento y sus
salvaguardas. La apariencia toma la referencia proporcionada por el usuario;
los controles sin función musical disponible se identifican como «Pendiente».
Los colores de la interfaz están en `ui/theme.rs`, con el contraste comprobado.

Los atajos siguen el perfil de Sibelius descrito en **Motor de atajos**; las
etiquetas de la cinta, del inspector y de los menús se leen del mapa activo.

El layout mantiene su geometría y caché en coordenadas musicales. `Layout.scale`
se aplica al pintar, seleccionar, colocar el cursor y seguir la reproducción;
no se copian ni reescriben todos los compases al cambiar de escala. El modo
compacto de `LayoutEngine` calcula límites verticales con las cajas reales de
Bravura, plicas y líneas adicionales; los guarda junto a la caché y reserva un
margen entre pentagramas. El modo estándar conserva su espaciado anterior.
`score-layout::views` añade tres presentaciones sobre el mismo grabado. Continua
adapta el ancho de los sistemas a la ventana; Pentagrama elimina los saltos y usa
anchuras naturales para todos los compases. Página conserva un ancho de 1000
unidades y proporción de hoja √2, con cabecera inicial, márgenes, número y separación
entre hojas. Un sistema indivisible más ancho o alto amplía todas las hojas del
documento de forma consistente. La paginación conserva las dos manos del piano y
las familias completas.

`Layout.systems` registra intervalos de compases y límites verticales; `pages`
registra las hojas y sus intervalos. La traslación mueve notas, etiquetas, llaves,
corchetes y barras desde sus posiciones originales en una sola pasada. Las curvas
y reguladores se recalculan después con los sistemas finales. La caché conserva
la geometría local sin incorporar la posición de página. La cabecera usa la misma
escala que la partitura. El lienzo pinta solo los compases que cruzan el área visible.

En la aplicación se verificaron el selector sin TODO, Continua, el cambio a Página
con su contador y Pentagrama con cinco instrumentos alineados. El control de
ventana falló de forma intermitente al probar el ajuste completo de la hoja; esa
interacción manual quedó sin confirmar. La distribución, selección y navegación
entre varias páginas a 28 %, 70 % y 150 % están cubiertas por pruebas automáticas.

Las vistas son estado de interfaz: no cambian el documento, historial, IDs,
selección, cursor o reloj de audio. La navegación de páginas centra las hojas que
caben y alinea al margen superior las grandes, con límites de desplazamiento.
`⌘0` activa Página y calcula el ajuste de la hoja a la ventana. Las pruebas cubren
saltos, hojas grandes, traducción de adornos, hit testing con zoom, conexiones
entre hojas, cambio de vista y navegación de ida y vuelta sin acumular deriva.

El reloj visible se actualiza desde la posición ya publicada por el control de
audio; la interfaz no consulta ni modifica el sintetizador para dibujarlo.

## Barra de herramientas

El panel de instrumentos vive en `ui/instrument_panel.rs`. El diálogo mantiene un
borrador de cantidades por instrumento, limitado a las plazas disponibles de las
48 partes; ni seleccionar ni cancelar modifica el documento. Confirmar ejecuta
`Command::AddParts` como una transacción: si falla un alta, no se guarda ninguna,
y Deshacer retira el conjunto completo. El contexto modal bloquea la entrada de
notas y los comandos globales hasta confirmar o cancelar.
Cada fila elimina por `PartId` mediante `RemovePartById`, conservando el pasaje y
la nota elegidos cuando pertenecen a otra parte. Si se elimina la parte activa se
elige su vecina; Deshacer recupera los pentagramas y conexiones originales.

La cinta y el menú Archivo siguen capas con dependencias hacia abajo:

```text
ui/toolbar.rs (GPUI)  pinta ToolbarView y MenuRow; cada clic emite un comando
   ▼
score-toolbar         TABS y FILE_MENU (datos) · project() · fit() · project_menu()
   ▼
score-keymap          registro (etiquetas, pendientes) y mapa activo (teclas)
AppView::run          punto único de ejecución · ui/command_state.rs implementa CommandStates
```

- **Descripción** (`spec.rs`): pestañas (`TabId`), grupos y controles. Cada control es
  un `CommandId` con tamaño (grande o casilla), un glifo, un nombre corto y una ayuda.
  No contiene textos de estado ni teclas.
- **Estado** (`CommandStates`): la única pregunta que el modelo hace a la app: si un
  comando está activo, habilitado o con una etiqueta temporal («Terminar expresión»).
  La app la responde en `ui/command_state.rs`; los tests usan un falso.
- **Proyección** (`project`): une descripción, registro, mapa y estado. Cada control es
  una casilla: glifo y, debajo, su nombre y su tecla. Mide con valores fijos
  (`Metrics`, anchos por carácter que cubren el texto más ancho de la cinta). Con
  sitio, un grupo se amplía y muestra los nombres; si falta, `fit` los quita (glifo y
  tecla) y después pliega grupos por prioridad (el de menor prioridad y, a igualdad,
  el de la derecha) hasta que la fila cabe. La cinta nunca hace scroll; un grupo
  plegado es un botón con desplegable.
- **Ayuda al pasar el cursor**: los controles de la cinta y los comandos del
  inspector muestran una ayuda breve con su nombre y la asignación activa después
  de la espera de GPUI (500 ms). Los desplegables y Preferencias conservan las teclas
  como referencia; el inspector ya no repite la lista completa de atajos.
- **Archivo** (`FILE_MENU`): una misma lista genera el desplegable de la ventana y el
  menú nativo de macOS, con el submenú «Abrir reciente».
- **Recientes** (`score-io::RecentFiles`): `recientes.json` en la carpeta de datos,
  máximo 10, sin duplicados, el más reciente primero; olvida los archivos que ya no
  existen. Se actualiza al abrir y guardar.

Los tests cubren que cada control apunte a un comando registrado, que las etiquetas,
teclas, estado y pendientes salgan de su fuente, que todas las pestañas quepan sin
plegar a 1420 px, el orden de plegado, el menú Archivo, la persistencia de recientes
y que la fuente musical tenga precargados todos los glifos de la cinta.

## Motor de atajos

`score-keymap` no depende de GPUI y concentra todo lo que antes estaba repartido
entre `bind_keys`, un traductor de teclas propio, etiquetas fijas en la cinta, el
panel de atajos, los menús y el README:

- **Registro de comandos** (`commands.rs`). Cada comando tiene un ID estable
  (`"entrada.nota.c"`), categoría, etiqueta, ámbito, si se repite al mantener la
  tecla y estado *listo* o *pendiente*. Las constantes de `id::` se usan en el
  perfil, en la interfaz y en el único `match` de `AppView::run`, así que un ID mal
  escrito no compila. Las extensiones futuras pueden registrar comandos propios.
- **Teclas** (`keys.rs`). Modelo propio de pulsación y secuencia, con nombres
  reservados para el bloque numérico (`numpad4`, `numpadenter`) aunque GPUI 0.2.2
  no los distingue. `Layout` traduce `⇧1`–`⇧9` al carácter que produce la
  distribución activa, porque GPUI informa ⇧2 como `"` (español) o `@` (EE. UU.) y
  descarta Shift. La app lee esos caracteres de macOS con `UCKeyTranslate` y vuelve
  a instalar los atajos al cambiar de distribución.
- **Perfil** (`profile.rs`). *Sibelius · portátil*: el conjunto *Notebook* de Sibelius,
  que funciona igual con o sin bloque numérico. Se verificó contra el tutorial
  oficial de Sibelius 2018.4 (3 corchea, 4 negra, 5 blanca, 7/8/9 ♮/♯/♭, 0 silencio,
  `.` puntillo, ↩ unión, F7–F12 distribuciones, ⇧1–9 intervalos en el modo portátil);
  el resto de teclas de una letra sigue la lista de SibAccess (Berklee). Las teclas
  sin función en Levare quedan reservadas como *pendientes*.
- **Cambios del usuario** (`user.rs`). `atajos.json` en la carpeta de datos
  (`~/Library/Application Support/Levare` en macOS) guarda, por comando, la
  lista completa que sustituye a la del perfil; una lista vacía lo deja sin atajo.
  Si existe la carpeta anterior `Partitura` y todavía no existe `Levare`, la app
  sigue usando aquella para conservar atajos, recientes y recuperación.
  Escritura atómica y número de versión.
- **Resolución** (`keymap.rs`). Perfil + cambios + distribución dan los atajos
  activos. Los del usuario ganan; los conflictos, IDs desconocidos y teclas que no
  se pueden detectar se informan en Preferencias en vez de fallar.
- **Documentación** (`docs.rs`). [docs/atajos.md](atajos.md) se genera desde el
  registro y un test falla si queda desactualizado.

En la app, `shortcuts.rs` traduce cada atajo a un `KeyBinding` de GPUI con una sola
acción `Run(CommandId)` y un predicado de contexto: `Levare` para ⌘…, `Levare
&& !Modal && !TextInput` para las teclas sueltas y `Levare && Modal` para cerrar
Preferencias. La raíz añade `Modal` mientras Preferencias está abierto. Los menús
nativos solo muestran combinaciones con ⌘/⌃ (`MenuRun` para el resto, con la tecla
en el texto), porque un equivalente de una sola tecla dejaría que macOS disparase el
menú antes que los contextos y el grabador de Preferencias. Un interceptor de GPUI
ve cada pulsación antes que los atajos: marca las repeticiones automáticas (los
comandos que alternan no se repiten) y entrega la combinación al grabador.

**Preferencias de atajos** (⌘, o el botón *Atajos*) lista los comandos por
categoría con sus teclas, estado y si están modificados. Permite cambiar, añadir,
quitar y restablecer uno o todos. Al grabar una tecla ocupada pregunta si debe
reasignarse. No hay buscador de texto: GPUI 0.2.2 no trae campo de texto y añadir
`gpui-component` queda para cuando haga falta en más pantallas.

Pendiente: perfil *Sibelius · escritorio* con bloque numérico (requiere que GPUI
distinga esas teclas), distribuciones F7–F12 del teclado numérico en pantalla,
secuencias de varias pulsaciones en el grabador y un buscador en Preferencias.

## Expresión musical

`score-core::expression` define `Notation` (dinámica y articulación opcionales) y
`Connection` (unión/expresión con dos `ElementId` serializables). Una unión exige
consecutividad musical, ausencia de huecos y el mismo conjunto de posiciones y
semitonos resueltos; ♮ explícito e implícito equivalen. Las frases se ordenan y no
se solapan dentro de un pentagrama. Se validan archivos y se eliminan enlaces
invalidados por ediciones, preservando las instantáneas de deshacer.

`score-layout::expression` graba marcas Bravura fuera de cabezas y plicas y genera
curvas por sistema. Reserva carriles para curvas y espacio bajo la línea de unión
para los ascendentes de forte. La geometría de curvas se calcula después de colocar
los compases; las marcas locales siguen la caché incremental. Los extremos siguen
la identidad musical al redistribuir renglones o cambiar el zoom.

`score-playback` conserva los `Span` escritos para selección y seguimiento, y genera
intervalos sonoros distintos para staccato y cadenas ligadas. Cada pentagrama tiene
su dinámica persistente. Las uniones conservan un único ataque; los unísonos entre
manos se combinan antes de calcular los intervalos de ataque por canal/tecla. La
recuperación desde selección usa la intensidad de ese ataque efectivo, incluso
si el evento original terminó antes de los unísonos posteriores. La pausa conserva
el estado del sintetizador y no introduce asignaciones en el callback.

La UI ofrece controles en cinta, inspector y menú nativo. ↩ alterna únicamente la
unión saliente; S marca inicio/fin de frase y Esc cancela. El estado provisional
se limpia al editar, deshacer, rehacer, crear o abrir otro documento. Las cuatro
articulaciones no tienen tecla por defecto (como en Sibelius portátil) y se pueden
asignar en Preferencias. Quedan pendientes las uniones de alturas individuales
dentro de acordes distintos.

### Catálogo de marcas

`score-core::marks` describe cada marca una sola vez. `MarkKind` enumera las 19
marcas (cuatro articulaciones, calderón, ocho dinámicas, pizz./arco, unión, expresión,
crescendo y diminuendo) con su nombre corto y largo, su familia (`MarkFamily`), su
glifo Bravura, el símbolo de botón y el lado del pentagrama (`Placement`). Una nota
lleva como mucho una marca de cada familia puntual; `set_point_mark` sustituye la de
su familia y `Command::ToggleMark` escribe, cambia o quita la de la selección.

`MarkRef` identifica una marca escrita (la nota de una marca puntual o la conexión
de una ligadura o regulador) para seleccionarla, borrarla (`Command::RemoveMark`) o
cambiarla (`Command::ReplaceConnection`). `score-layout` genera un `MarkBox` por marca
puntual, ajustado a las cajas de Bravura, y `Layout::hit` elige entre nota y marca.

`score-toolbar::marks` es la única tabla marca ↔ comando. La cinta (`mark_tile`), el
inspector, el menú **Expresión**, los símbolos de los menús, el estado resaltado y el
despacho de la app (`apply_mark`) la recorren en lugar de repetir cada marca. Un test
exige que cada marca tenga un comando propio, listo y con su nombre en la etiqueta.

Para añadir una marca puntual: su variante en `MarkKind` (y el campo en `Notation`
si es una familia nueva, con `serde(default)` para no cambiar el formato), su
comando en `score-keymap` y `score-toolbar::marks`, su colocación en
`score-layout::expression::marks` y, si suena distinto, su regla en `score-playback`.

### Calderón

`Notation.fermata` se guarda solo cuando está activo, así que los archivos anteriores
se abren sin migración. Admite notas y silencios, independiente de la articulación;
al pegar una figura que se divide queda en el último fragmento. Se dibuja centrado
sobre la nota, por fuera de la articulación, y desplaza hacia arriba la técnica.

En reproducción, `score-playback::Timeline` convierte tiempo musical en audio: el
tramo de cada calderón dura `FERMATA_HOLD` (el doble) para **todas** las partes, y
lo que sigue empieza después. Los calderones simultáneos de varias partes se unen en
un solo tramo. El plan, los reguladores, los saltos de la regla, el reloj y los
segundos del transporte usan la misma línea de tiempo (`tick_at` es su inversa).

## Verificación

El portapapeles se comprobó en una aplicación temporal con el ejemplo de piano:
Shift+flecha seleccionó dos acordes ligados de blanca; se copiaron con ⌘C y se
pegaron en un noveno compás de la mano izquierda. Se cortaron, se deshizo/rehízo
el corte y se pegaron otra vez. El archivo guardado conservó C4/E4/G4 en ambos
acordes, su dinámica mp y la unión entre los IDs nuevos. Se comprobó también el
rechazo de un destino ocupado sin modificar el score. El filtro de atajos permite
repetir Pegar para pegados consecutivos. Las pruebas automatizadas cubren selección
inversa, historial de pasajes, huecos, crecimiento alineado, compases cruzados,
ligaduras, conservación de sonido entre tonalidades, dinámica vigente, rechazo de
contenido externo y rollback. La reproducción de una figura staccato dividida
coincide en ataques, intensidad y tiempos MIDI con el original. La revisión
independiente no encontró problemas importantes.


La agrupación del score se comprobó con un archivo temporal deliberadamente
invertido de `cuarteto-en-sol.partitura`: al abrir se mostraron flauta, violines,
viola y violonchelo con corchetes independientes de maderas y cuerdas. La ventana
mostró el control persistente «Eliminar seleccionado». Desde el menú nativo se
añadió un oboe y C4, se eliminó la parte y se deshizo/rehízo su eliminación. El
archivo guardado confirmó el oboe con su nota, situado después de la flauta.
Los tests cubren orden estable, identidad de eventos y conexiones, deshacer partes,
normalización y guardado, corchetes por sistema, cortes de barra entre familias,
llaves de piano y selección espacial. La revisión independiente no encontró
problemas concretos.


La entrega de expresión se comprobó visualmente con `examples/piano-expresion.partitura`:
curvas, tres uniones de un acorde, dinámicas en ambas manos y las cuatro articulaciones.
En una aplicación temporal se escribió un noveno compás C4–C4–E4, se añadieron
una unión y una frase con T/L, staccato con S y ff desde el menú nativo. Tras
deshacer/rehacer la dinámica, el archivo guardado conservó las tres notas, las
marcas y ambos extremos de las ligaduras en formato 7. Los tests cubren rechazo
transaccional, restauración de enlaces con historial, lectura de esquemas 1–6,
curvas entre sistemas, espacio bajo forte, ausencia de nuevos ataques en cadenas,
unísonos entre manos y recuperación de intensidad. La continuidad de audio con
pausa se compara muestra por muestra sin asignaciones en el callback. La revisión
independiente comprobó las correcciones y no encontró problemas importantes.
`audio_smoke` completó ligaduras, dinámicas y articulaciones junto a las dos manos,
flauta y violonchelo en la salida real a 44 100 Hz, con selección, pausa y continuación.


La interfaz de referencia se comprobó en una aplicación temporal independiente,
con `examples/cuarteto-en-sol.partitura`: cinco pentagramas visibles, nombres de
instrumentos repetidos, paneles, inspector, cinta y transporte. La entrada con
N desactivada no añadió notas; al activarla, 5 produjo un acorde de semicorchea
y 4 un silencio de corchea. El diálogo nativo guardó estos datos, incluido un
bemol individual. Tras corregir la interpretación del carácter de teclado,
`=` aplicó y guardó un becuadro en la nota seleccionada. Los tests comprueban
también la selección y el seguimiento con zoom, la separación de acordes extremos
en el espaciado compacto y las nuevas duraciones. La revisión independiente no
encontró defectos importantes.

La armadura de Sol mayor se comprobó visualmente con su ejemplo de piano:
sostenido en ambas claves, espacio antes del compás y cancelación/restitución
en las notas de Fa. En una sesión temporal se eligió Mi♭ mayor en el menú nativo,
se deshizo y rehízo el cambio, y se escribieron acordes en un quinto compás. El
diálogo nativo guardó formato 6, tres bemoles, C4/E4/G4 sin alteración propia en
la derecha y D3/F3/B♮3 en la izquierda. La lectura del archivo confirmó tonalidad,
IDs y el becuadro individual. Las capturas de la herramienta quedaron desactualizadas
tras las interacciones; la armadura modificada se verificó por el archivo y los
tests de grabado. `audio_smoke` completó Re mayor con ambas manos, flauta y
violonchelo, selección, pausa y continuación en la salida real a 44 100 Hz.
Los tests cubren las 30 tonalidades, todas las octavas, sol/do/fa, repetición por
sistema, restitución, caché, historial, validación, migración 1–5 y notas sostenidas.
La revisión independiente no encontró defectos concretos.

La entrega de alteraciones se comprobó con `examples/piano-alteraciones.partitura`:
sostenidos, bemoles, becuadros de cancelación, signos en acordes y columnas para
segundas. En una sesión temporal se escribió un quinto compás en ambas manos y se
usaron ⌥S/⌥F/⌥N, ⌥← y ⌥0 para alterar notas individuales, además de deshacer y
rehacer. El diálogo nativo guardó C♯4/E♭4/G♮4 y C3/E♯3/G♭3 con sus respectivas
alteraciones. La revisión independiente no encontró defectos concretos. Las
pruebas cubren la selección individual, cancelaciones por compás y pentagrama,
columnas densas, seguimiento, formato 5 con migración de versiones anteriores,
semitonos MIDI, enarmonías y continuidad exacta del audio sin asignaciones. La
salida real a 44 100 Hz completó la prueba de acordes alterados, selección, pausa
y continuación.

La entrega de acordes se comprobó con `examples/piano-acordes.partitura`: cabezas,
plicas compartidas, segundas desplazadas y las dos manos. En una sesión temporal
se escribió Do–Mi–Sol en el quinto compás de cada mano, mediante Shift+letras en
la derecha y modo Acorde en la izquierda. El diálogo nativo guardó dos acordes de
negra, C4/E4/G4 y C3/E3/G3, en sus pentagramas. La vista mostró el nuevo sistema.
`audio_smoke` verificó acordes de ambas manos junto a flauta y violonchelo en la
salida real a 44 100 Hz, con inicio seleccionado, pausa y continuación. Las pruebas
automatizadas cubren límites, duplicados, historial, migración y guardado, plicas,
segundas, selección, seguimiento, unísonos y continuación exacta del audio sin
asignaciones ni liberaciones en el callback.

El gran pentagrama se verificó con `examples/piano-dos-manos.partitura`: claves
de sol/fa, llave, barras compartidas, alineación y selección de mano izquierda
con ⌥2. En una sesión temporal se añadió un quinto compás, se escribió C3 en la
izquierda y E4 en la derecha con ⌥1, y se guardó mediante el diálogo nativo. El
archivo resultante conservó las dos notas en sus respectivos pentagramas.
`audio_smoke` comprobó ambas manos junto a flauta y violonchelo en la salida real
a 44 100 Hz, con inicio seleccionado, pausa, continuación y finalización.
Los tests incluyen identidad por pentagrama, historial, migración de los esquemas
1/2, archivos mal formados, unísonos, separación de posiciones y 14 pianos con
28 manos activas. El callback mantiene la comprobación de cero asignaciones.

La entrega de conjunto añade pruebas de identidad entre partes, crecimiento de
compases, selección y deshacer de instrumentos, migración del esquema 1, guardado
del esquema 2, claves, alineación entre duraciones, espacios mínimos, canales
independientes y comienzo durante notas sostenidas de otras partes. Los 13 timbres
del catálogo tienen un preset verificado en el banco y producen audio distinto.
La pausa de un conjunto se compara muestra por muestra con la reproducción sin
interrupción, bajo la comprobación de cero asignaciones/liberaciones del callback.

Se comprobó visualmente `examples/trio.partitura`, con el panel de instrumentos,
pentagramas alineados, claves de sol/fa y resaltado simultáneo de las tres partes.
`audio_smoke` verificó piano, flauta y violonchelo sobre la salida real a 44 100 Hz,
incluyendo inicio seleccionado, pausa, continuación y finalización.

Los tests cubren capacidad de compás, notas fuera de rango, referencias estables,
aislamiento de instantáneas, transposición entre octavas, historial, rechazo
transaccional de cambios, secuencias de edición/deshacer/rehacer, estado de guardado,
archivos inválidos y futuros, guardado real en disco, selección espacial y
reutilización del layout. El flujo macOS de CI ejecuta formato, tests y Clippy.

Comprobación manual de la entrega:

1. Ejecutar `cargo run`; introducir `C D E F G` y comprobar el salto de compás.
2. Elegir corchea con `3`, introducir `A B`, insertar silencio con `0`.
3. Seleccionar una nota con clic, transponer con flechas y deshacer/rehacer.
4. Guardar, crear una partitura nueva y abrir el archivo guardado.
5. Intentar cerrar con cambios y comprobar que cancelar conserva el documento.

Se verificaron visualmente la entrada de notas y silencios, el salto de compás,
la tipografía, el guardado con diálogo nativo, la creación de un nuevo documento,
la reapertura del ejemplo y la cancelación del aviso de cambios sin guardar.
La selección espacial y el historial se verifican mediante tests. Falta completar
la prueba manual específica de salir desde el Dock y recuperar al reiniciar.

En la entrega de audio se verificaron en la ventana la escritura con audición,
el inicio con Espacio, el resaltado durante la reproducción, la finalización
automática y la parada mediante Esc. `audio_smoke` comprobó además el avance y
finalización del stream sobre la salida real del equipo a 44 100 Hz.

Para el seguimiento se comprobó visualmente una partitura temporal de 32 compases:
iniciar desde una selección fuera de pantalla mostró la nota activa y el avance
hacia renglones posteriores mantuvo visible el resaltado. La selección permaneció
en su posición original. Las comprobaciones de desplazamiento manual durante la
pausa y de límites horizontales están cubiertas por tests; la herramienta de
control de ventanas no permitió completar esas interacciones manuales.

## Bloque MVP: edición, recuperación y PDF

`score-core::measure_edit` concentra las ediciones estructurales: inserción y
eliminación sobre todos los pentagramas, reasignación de índices de eventos y
mapas de métrica/armadura/tempo, conexiones y arpegios de piano. Al borrar cambios
locales se conserva el estado efectivo de la primera música retenida. Borrar todo
crea un único compás vacío. El editor guarda cada operación en una instantánea.
`set_durations` valida los totales finales y solo después actualiza duraciones y
depura conexiones, evitando pérdidas por estados intermedios del pasaje.

Clic en nota desactiva entrada. A–G reemplaza el tono elegido conservando identidad
y duración; las figuras actúan sobre la selección. Delete borra sonido conservando
tiempo y las marcas válidas para silencios. La operación de eliminación con
reducción de tiempo sigue disponible para Cortar y el menú Edición.

`score-pdf` es un adaptador sin GPUI: construye `ViewMode::Page`, transforma cada
hoja a A4 y dibuja líneas, barrado, curvas con métricas de Bravura, reguladores,
textos y contornos originales de la fuente mediante Quartz/CoreText. Los textos
musicales comparten `score-layout::repertoire` con pantalla y menús; el texto
ordinario se forma en tramos completos para conservar Unicode. Se valida la
cantidad de páginas antes del reemplazo atómico. El destino exige extensión PDF
y la interfaz impide reemplazar el documento abierto. La exportación en segundo
plano conserva la instantánea y el estado de guardado. No se implementa empaquetado.

Pruebas: reemplazo de tono/colisiones, borrado conservando tiempo y reducción de
tiempo, duración que desborda, inserción/eliminación con cambios locales y
ligaduras, deshacer, escritura de recuperación, exclusión de sesiones, limpieza
ordenada, errores que preservan la copia anterior y proceso hijo terminado por
fuerza antes de reabrir. PDF verifica documentos de piano, cuerdas y varias
páginas, además de destinos inválidos y preservación de archivos. Comprobación
visual mediante Poppler: piano con arpegios entre manos y cuarteto de dos páginas.

## Dos voces y divisi

`Voice::{One, Two}` identifica dos secuencias por compás con un almacén común de
`EventId`. `order` conserva la voz original y `second_order` añade la segunda sin
cambiar IDs antiguos. `timed_iter` calcula el inicio dentro de cada voz; la
capacidad del compás es el máximo de sus duraciones, no la suma. La validación
rechaza IDs duplicados entre secuencias y desbordes de cada voz. El cursor,
selección de rango, portapapeles, entrada, borrado y navegación conservan la voz.
Las conexiones rítmicas y ligaduras deben unir eventos de una misma voz; las frases
de voces distintas pueden solaparse.

El grabado usa una rejilla temporal compartida, plicas opuestas, barrado y tresillos
por voz, silencios separados y desplazamiento horizontal de segundas coincidentes.
Las alteraciones se resuelven cronológicamente para todo el pentagrama. Al pulsar
cabezas de unísono coincidentes se prefiere la voz activa. Las articulaciones y
calderones inferiores usan sus glifos originales de Bravura. Los textos y dinámicas
reservan espacio frente a las plicas de ambas voces.

`Part::divisi_changes` (esquema 17) guarda fronteras por compás y tick local.
La migración del esquema 16 convierte cada cambio de compás en un cambio de tick
cero. `DivisiPoint` normaliza el final del compás al comienzo del siguiente; los
rangos son semiabiertos y terminan al finalizar la última nota seleccionada.
`div.`/`unis.` y los comandos de pasaje usan estas mismas posiciones. El borrado e
inserción de compases remapean mapas completos sin perder sus offsets internos;
la eliminación conserva el estado inicial del primer compás retenido.

El documento conserva sus voces, figuras e identidades. En cada compás con un
intervalo divisi se muestran las voces 1 y 2 en pentagramas completos separados;
«div.» y «unis.» se colocan en sus ticks reales. La presentación por compás evita
saltos de línea, continuaciones artificiales y ligaduras diagonales para notas
sostenidas. `display_source`, `displayed_staff` y `VoiceRegion` comparten ese
criterio para el grabado, el cursor, la inserción, la selección y el seguimiento.
Las fronteras temporales permanecen en el mapa del documento y no afectan al audio.
La caché conserva las regiones y las fronteras escritas, también cuando una marca
no cambia el estado. No se requiere una copia temporal con figuras fragmentadas.
Los cortes siguen validándose sobre la cuadrícula rítmica disponible antes de
confirmar la operación; las ligaduras y reguladores originales conservan sus IDs.

Los cambios de divisi no fuerzan saltos de sistema: una fila puede contener
compases con uno o dos pentagramas. Cada entrada de la fila inferior reserva
su clave y armadura; los compases inactivos no crean celdas ni destinos de clic.
La maquetación de conjuntos de tres o más partes usa un pentagrama de unos 5,3 mm
en A4. Los compases reservan anchura según la tinta de las indicaciones, incluidos
matices largos. Los reguladores comparten la altura de las dinámicas cuando cabe,
respetan las notas graves y dejan huecos horizontales alrededor de los matices.

Las indicaciones de evento heredadas se limpian al establecer un pasaje para que
no reaparezcan al sobrescribir o quitar fronteras. Los esquemas 1–16 mantienen la
presentación anterior. Las partes legacy con `Part::divided` conservan sus dos
pentagramas físicos; al editar un pasaje se normalizan a las dos voces, remapeando
selección y extremos en una transacción. Rechaza reunir más de dos voces y
convertir un piano. Dinámicas, reguladores y técnicas siguen compartidos por la
sección; las manos del piano mantienen su expresión independiente.

El plan de reproducción crea una pista por voz con sus propios intervalos, ataques
y sostenidos, alineados en el reloj común. La preparación de expresión considera
sonidos simultáneos y preserva los rangos por pista; los grupos que se solapan
reservan margen común. Hasta 56 pistas y 448 alturas sostenidas caben en buffers
fijos. La prueba de división compara todos los mensajes programados antes de
dividir, después de dividir y después de reunir, incluidas técnica, dinámica y
regulador. También se prueban edición/deshacer, capacidad por voz, copia con dinámica
simultánea, coincidencias de cabezas, alteraciones cronológicas, carga/recuperación y
exportación de los ejemplos `voces-y-divisi` y `divisi-dos-pentagramas`.

## Entregas siguientes

1. **Edición musical:** enarmonía y dobles alteraciones, voces 3–4 y edición de grupos irregulares generales.
2. **Seguridad del trabajo:** más pruebas con `proptest`,
   fixtures de documentos e importadores sometidos a fuzzing.
3. **Grabado:** refinamiento de curvas y colisiones,
   saltos de sistema/página manuales y snapshots visuales. Añadir `kurbo`, `rstar` y `rayon`
   cuando existan las operaciones que justifican su uso.
4. **Intercambio:** MIDI con `midly`, partes transpuestas y opciones de papel; ampliar la compatibilidad MusicXML existente.
5. **Sonido:** ampliar la reproducción ya implementada con repeticiones y MIDI externo con
   `midir`, pedal de piano, percusión y pruebas de desconexión/cambio de dispositivo.
6. **Medición y distribución:** benchmarks representativos antes de reclamar
   presupuestos de rendimiento, perfilado, firma y empaquetado de macOS;
   validación de Windows y Linux.

Se han incorporado Bravura y GeneralUser GS, con sus licencias originales. No se
han copiado datos de instrumentos de MuseScore ni de LilyPond. Tampoco se ha
creado un repositorio Git: el directorio inicial no tiene `.git`.


## Arpegios

`Notation.arpeggio` admite `Normal`, `Up` y `Down` en acordes de al menos dos
alturas. Es una marca del catálogo, con referencia estable, selección, borrado,
reemplazo y transacción del historial. Quitar la última nota adicional también
quita la marca; los snapshots la recuperan al deshacer. El esquema 13 conserva
arpegios y abre esquemas anteriores con acordes simultáneos. El portapapeles los
conserva en el primer fragmento de una figura dividida.

El grabado compone los segmentos Bravura `wiggleArpeggiatoUp/Down` y sus flechas
(EAA9/EAAA/EAAD/EAAE), usando `repeatOffset` y avances originales. `Glyph.rotated`
gira el conjunto 90 grados sin deformar contornos. Las métricas de tinta, el hit
testing, la caché, el seguimiento y la separación rítmica incluyen esa geometría.
Los controles usan los símbolos completos de Bravura E63C/E634/E635.

El plan y la audición comparten una secuencia acotada: alturas MIDI ordenadas,
unísonos enarmónicos deduplicados, 30 ms por intervalo, máximo 120 ms y media
puerta sonora. Los sonidos de una cadena ligada solo se atacan una vez. El seek
recupera exclusivamente los sonidos ya atacados. El renderer divide bloques en
los ataques pendientes de la audición, con almacenamiento fijo; reemplazar,
reproducir o detener cancela los ataques pendientes anteriores.


## Arpegio entre manos del piano

El esquema 14 añade `Score.piano_arpeggios`, conexiones de dos IDs estables
(superior e inferior) y dirección. Solo une ataques simultáneos de la misma parte
Piano, incluidos intervalos con una nota por mano. Se edita desde cualquiera de
los extremos; sustituye marcas locales y se poda si una edición desplaza o borra
un extremo. Deshacer restaura la relación. El portapapeles, que copia un solo
pentagrama, convierte la indicación compartida en una local cuando copia un
acorde; no exporta referencias a notas ausentes del pasaje.

`Layout.piano_arpeggios` se calcula tras colocar los pentagramas y se vuelve a
calcular tras paginar. Sus segmentos Bravura, caja seleccionable y seguimiento
usan coordenadas finales. La cuadrícula reserva espacio en ambas manos. El plan
ordena conjuntamente hasta 16 alturas con los límites anteriores; cada mano
conserva sus niveles, técnicas y finales, y las continuaciones ligadas no añaden
ataques. La audición dispone de 16 ranuras fijas con intención y final por nota;
el callback divide bloques en cada ataque y liberación sin asignar memoria.

### Armónicos naturales y artificiales

`Notation::harmonic` es una marca puntual independiente de `PlayingTechnique`.
`Harmonic::Natural` se escribe a altura sonora con `stringsHarmonic` (E614).
`Artificial` conserva una sola nota pisada: `harmonic_touch` calcula la posición
y alteración de su cuarta justa, y `sounding_pitches` resuelve la tonalidad antes
de subir dos octavas. La cabeza `noteheadDiamondWhite` (E0DD) es derivada: tiene
selección propia como `MarkRef::Harmonic`, nunca identidad de nota ni ataque MIDI.
Los dobles bemoles de la cuarta derivada se graban sin ampliar el modelo de
alteraciones de entrada. Su estado local se cancela en las notas siguientes.

El núcleo valida instrumento, presencia de notas y rango antes de insertar,
editar o abrir documentos. Los cambios de un pasaje se validan y aplican juntos,
conservando las ligaduras válidas en el resultado final. Borrar sonido quita el
armónico; copiar una nota larga lo conserva en todos sus fragmentos ligados.
El formato nativo es 19, con migración desde 18, y el portapapeles es 6 para
impedir que una app anterior pierda silenciosamente las nuevas marcas.
Plan, audición y recuperación de notas sostenidas comparten la altura sonora.
`NoteEvent::harmonic` conserva la intención desde el plan o audición hasta el
proveedor, incluidas las notas sostenidas al buscar una posición. El motor
integrado genera el timbre mediante síntesis aditiva a la altura ya resuelta:
fundamental dominante, parciales superiores escasos, ruido leve de arco y
envolventes distintas para arco y pizzicato. No utiliza el programa de armónicos
de guitarra de General MIDI ni vuelve a transponer la nota.

`platform::sound_engine::harmonics` prepara tabla seno, frecuencias y 512 voces
antes del callback. Los parciales se atenúan antes de Nyquist; las notas al arco
se sostienen y las pulsadas decaen aunque sigan escritas. CC7/10/11, silencios,
pausa, búsqueda y reverberación utilizan el contrato común. No hay asignaciones
ni carga de archivos en los ataques o el renderizado. VSCO CE rechaza estos
ataques porque no tiene muestras específicas; el motor integrado los cubre.
Un proveedor futuro puede interpretar `NoteEvent::harmonic` con grabaciones.
La síntesis es una aproximación de timbre, no una grabación orquestal.

Los unísonos se fusionan por canal, altura y clase de timbre (normal/armónico),
para que las dos voces o un divisi conserven ambos sonidos. `HarmonicOff` y
`SoundEngine::harmonic_off` liberan únicamente la voz armónica; la nota normal
usa el `NoteOff` habitual. Los proveedores que acepten armónicos deben implementar
esa liberación independiente; el valor predeterminado es inerte para proveedores
sin esas muestras. Las curvas cuadráticas de velocidad y CC11 conservan la
calibración de una dinámica cuando un regulador reserva margen de volumen.


## Importación de MusicXML

`score-io::open` detecta documentos nativos, XML partwise/timewise y contenedores
MXL por contenido. Devuelve `OpenedScore` con el indicador `imported` y avisos;
`load` conserva su contrato nativo para recuperación y herramientas existentes.
El lector `musicxml/import` posiciona los eventos con divisions, backup y forward;
el constructor traduce esas posiciones a voces, rellena huecos y divide figuras
con ligaduras cuando hace falta. `Score::new` construye instrumentos y compases
vacíos mediante la API del dominio, sin depender de la representación serializada.

La entrada y el XML descomprimido se limitan a 16 MiB; container.xml a 64 KiB y el
árbol a un millón de nodos. No se extraen archivos ni se descargan DTD o entidades
externas. ZIP solo habilita DEFLATE. Los errores conservan el documento abierto.
La interfaz importa con `Document::recover` y sin ruta nativa, presenta los avisos
y utiliza el flujo normal de Guardar como. El XML original no es destino del
autoguardado. Las pruebas cubren MXL, timewise, voces y acordes externos,
transposición, anacrusas, expresiones y el recorrido exportar/importar de los
22 ejemplos comparando ataques, alturas, duraciones, armónicos y técnicas.


## Exportación de partitura y partes

`score-export` coordina la selección de documentos (score completo y `PartId`
individuales) y formatos PDF/MusicXML. `Score::extract_part` crea una copia con
identificadores estables y conserva la parte completa: pentagramas, voces,
divisi, marcas, conexiones y arpegios de piano pertinentes. Los cambios globales
de métrica, tonalidad y tempo permanecen en la copia. Un nombre de parte opcional,
con valor por defecto para documentos anteriores, conserva la identidad «Violín II»
cuando el instrumento queda solo; `part_name` comparte ese nombre entre grabado y
MusicXML. La extracción no transforma alturas ni modifica la partitura original.

El diálogo de exportación usa el estado modal existente, bloquea la edición,
detiene el audio y ofrece selección de todos/ninguno, score y partes individuales,
y PDF/MusicXML simultáneos. Enter continúa y Esc cancela. El destino es una carpeta
nueva: se preparan todos los archivos en un directorio temporal junto al destino,
y solo después se crea la carpeta final. `create_dir` impide reemplazar conjuntos
anteriores incluso si aparecen durante el renderizado. Ante un fallo al publicar,
se retiran los archivos publicados por esa operación. Cancelar el destino o fallar
la exportación devuelve la misma selección con una explicación cuando procede.
Los nombres están ordenados y limitados por bytes UTF-8; los separadores y controles
se sustituyen. La operación no toca la ruta, el historial ni el estado de guardado
del documento.

Las pruebas comparan la extracción de todas las partes de los ejemplos con el
original, incluidas voces, cambios globales, divisi, conexiones y arpegios de piano.
También verifican nombres y orden de secciones, subconjuntos, formato nativo,
MusicXML de cada parte, selección inválida, protección de destinos existentes y
la entrega conjunta de PDF/MusicXML del Menuetto.


## Clic para situar la reproducción y arrastre de alturas

`playhead::score_tick` traduce coordenadas del lienzo a tiempo musical sobre la
misma cuadrícula rítmica que dibuja el cabezal: una cabeza/silencio usa su onset
exacto, incluido un tresillo; el espacio usa pulsos del denominador local. Acepta
las tres vistas, zoom, voces, divisi y compases vacíos. Los clics normales llaman
al transporte `seek` existente, manteniendo reproducción o pausa; no fuerzan un
desplazamiento de la vista mientras comienza un posible gesto de ratón. Shift
mantiene la posición del transporte y sigue ampliando el pasaje.

`NoteDrag` distingue el movimiento vertical inicial del horizontal con el umbral
de cinco píxeles de selección. El vertical edita una sola cabeza del acorde;
el horizontal inicia el movimiento temporal de la figura completa y Shift
conserva `PassageDrag`. Una copia persistente de `Score`
permite previsualizar la altura sin tocar documento, historial ni recuperación.
El grabado usa un `LayoutEngine` propio del gesto y la altura tentativa para el
resaltado. `layout_pitch_preview` conserva sistemas, anchos, cuadrícula rítmica y
orígenes de todos los pentagramas del layout inicial: la cabeza sigue al ratón
aunque aumente su envolvente de tinta. Solo al confirmar se reserva espacio
y se repagina de nuevo. El movimiento usa medio espacio del pentagrama por paso diatónico y
compensa el origen desplazado del lienzo. Se conservan las alteraciones explícitas;
la armadura determina las notas sin alteración explícita. Las colisiones de cabezas
y los límites de armónicos se rechazan con explicación.

Al soltar, `ReplacePitch` aplica una sola edición, con audición si está activa.
Deshacer restaura también las conexiones eliminadas por incompatibilidad de altura.
Volver a la altura original no crea historial. Escape, pérdida de foco, soltar fuera
de la ventana u otro comando cancelan la vista previa. Empezar el arrastre de altura
detiene el audio; seleccionar/clicar sin arrastrar conserva su estado.

Las pruebas verifican resolución de gesto, zoom, desplazamiento, acordes, límites,
armónicos y un único deshacer/rehacer con estado de guardado correcto. Los clics se
comparan con los onsets grabados de todos los eventos de piano, Menuetto y divisi
interno en las tres vistas y cuatro escalas; también se prueban compases vacíos con
cambios de métrica y coordenadas fuera de la partitura.


## Arrastre horizontal entre tiempos y compases

`Score::move_event` modifica una copia y publica solo tras validar. Conserva
onsets de las figuras vecinas y la voz/pentagrama de origen. Rellena huecos con
silencios representables (descomposición mínima por programación dinámica),
recorta silencios de destino sin indicaciones e intercambia figuras completas
si la ocupación tiene igual duración. Rechaza solapamientos, destinos fuera de
capacidad, silencios con marcas y huecos no representables sin tocar el original.
Los IDs de elementos que permanecen en el compás se conservan; al cruzar de compás
se reasignan solo los IDs desplazados y se remapean sus enlaces antes de depurar
los incompatibles. Los arpegios entre manos que pierden simultaneidad pasan a
marcas locales cuando hay acordes. Las alturas se reescriben entre armaduras
para conservar su sonido. Compases, tempo, tonalidad, divisi, otras voces e
instrumentos conservan sus posiciones.

`MoveSelectedEvent` actualiza selección/cursor y crea una sola entrada en el
historial. `NoteDrag` fija el eje al superar cinco píxeles: vertical cambia una
cabeza, horizontal mueve la figura completa. Calcula el destino sobre el layout
inicial, compensando zoom, scroll y distancia del punto agarrado a la cuadrícula
rítmica. Se ajusta a la cuadrícula de semicorcheas o tresillos según la duración,
y a los onsets de la voz original. Comprueba el movimiento en una copia y dibuja
una figura azul con Bravura y un indicador en destino; no usa el layout de
previsualización de alturas para duraciones/onsets cambiados. Escape, pérdida de
foco o soltar fuera cancelan. Shift no arma `NoteDrag` y conserva la selección.

Las pruebas cubren tiempos libres, intercambio de acordes y marcas, silencios
partidos, movimiento solapado con la propia posición anterior, rechazo atómico,
armaduras, voces y manos independientes, enlaces, una acción de deshacer/rehacer
y destinos del ratón en las tres vistas, con zoom, scroll y voz de divisi proyectada.


## Orquesta y transposición

`Instrument` reúne 49 perfiles y una plantilla `ORCHESTRA` de 34 partes. El selector
ordena por `score_order` y admite cantidades para cada instrumento.
`Part::transposition` es el intervalo de altura escrita a sonora (pasos diatónicos y
semitonos). Los eventos almacenan alturas escritas; la tonalidad global permanece en
altura real. `written_key_at` deriva la armadura para cada parte; `sounding_pitches`
resuelve la armadura escrita, los armónicos y después el intervalo instrumental.
Audio, audición y búsqueda de notas sostenidas consumen ese resultado.

El esquema 19 usa intervalo cero por defecto al leer documentos antiguos; no cambia
sus notas ni sonido. Las partes nuevas reciben el intervalo del catálogo.
El portapapeles 6 conserva el intervalo de origen y convierte por altura real al
pegar en otro instrumento. MusicXML exporta alturas y armaduras escritas junto con
`transpose`, incluyendo octavas, y agrupa canales mediante `midi-device`/`port`.
La importación resuelve primero a altura real y convierte al perfil escrito de destino,
conservando el intervalo explícito inicial. Las armaduras enarmónicas se limitan a
siete alteraciones; las notas que exigirían dobles alteraciones se reescriben con una
enarmonía simple equivalente. La percusión sin altura usa su nota General MIDI fija,
clave Bravura E069, posición central y banco 128 independiente en cada ruta.

Las pruebas de orquesta cubren 48 rutas, catálogo completo, cambios de tonalidad,
compatibilidad de archivos 18, MusicXML externo y exportado, copiado entre perfiles,
las tres vistas y la ausencia de recorte al imprimir sistemas altos.

Para orquestas de 12 partes o más, la vista de página adapta también su anchura al sistema más alto, evitando una columna estrecha al reducir todo el score. El PDF general usa A3; las partes y conjuntos pequeños conservan A4.

El selector reserva un ancho flexible para el nombre y mantiene los segmentos de texto musical en una sola línea, con alturas de fila estables y controles de cantidad fuera del área de texto.

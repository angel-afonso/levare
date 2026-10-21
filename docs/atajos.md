# Atajos de teclado

Perfil por defecto: **Sibelius · portátil**. Atajos de Sibelius para teclados sin bloque numérico; también funcionan con él.

Puedes cambiarlos en **Levare › Preferencias de atajos…** (⌘,). Tus cambios se guardan aparte, en `atajos.json` dentro de la carpeta de datos de la aplicación.

Los comandos *pendientes* ya tienen su tecla reservada y muestran un aviso hasta que la función exista.

> Archivo generado desde `crates/keymap`. Para actualizarlo: `LEVARE_ACTUALIZAR_ATAJOS=1 cargo test -p score-keymap`.

## Archivo

| Atajo | Comando | Estado |
|---|---|---|
| `⌘N` | Nueva partitura |  |
| `⌘O` | Abrir… |  |
| `⌘S` | Guardar |  |
| `⇧⌘S` | Guardar como… |  |
| `⌘W` | Cerrar ventana |  |
| `⌘Q` | Salir de Levare |  |
| `⌘P` | Imprimir… | Pendiente |
| — | Exportar partitura y partes… |  |
| — | Exportar MusicXML… |  |
| — | Información de la partitura… |  |
| `⌘,` | Preferencias de atajos… |  |
| — | Borrar la lista de recientes |  |
| `Esc` | Cerrar el panel abierto |  |

## Edición

| Atajo | Comando | Estado |
|---|---|---|
| `⌘Z` | Deshacer |  |
| `⌘Y` · `⇧⌘Z` | Rehacer |  |
| `⌘X` | Cortar |  |
| `⌘C` | Copiar |  |
| `⌘V` | Pegar |  |
| `⌘A` | Seleccionar todo el pentagrama |  |
| `⌫` · `⌦` | Convertir en silencios |  |
| — | Eliminar notas y su tiempo |  |
| `⇧⌘P` | Restablecer posición |  |
| `Esc` | Detener, cancelar o quitar la selección |  |
| `R` | Repetir la nota, acorde o silencio |  |

## Entrada de notas

| Atajo | Comando | Estado |
|---|---|---|
| `N` | Activar o desactivar la entrada de notas |  |
| `C` | Escribir Do |  |
| `D` | Escribir Re |  |
| `E` | Escribir Mi |  |
| `F` | Escribir Fa |  |
| `G` | Escribir Sol |  |
| `A` | Escribir La |  |
| `B` | Escribir Si |  |
| `⇧C` | Añadir Do encima |  |
| `⇧D` | Añadir Re encima |  |
| `⇧E` | Añadir Mi encima |  |
| `⇧F` | Añadir Fa encima |  |
| `⇧G` | Añadir Sol encima |  |
| `⇧A` | Añadir La encima |  |
| `⇧B` | Añadir Si encima |  |
| `⇧1` | Añadir unísono |  |
| `⇧2` | Añadir una segunda encima |  |
| `⇧3` | Añadir una tercera encima |  |
| `⇧4` | Añadir una cuarta encima |  |
| `⇧5` | Añadir una quinta encima |  |
| `⇧6` | Añadir una sexta encima |  |
| `⇧7` | Añadir una séptima encima |  |
| `⇧8` | Añadir una octava encima |  |
| `⇧9` | Añadir una novena encima |  |
| `0` | Escribir un silencio |  |
| `⌥A` | Modo acorde |  |
| `⌥⌫` | Quitar la nota más aguda del acorde |  |
| — | Subir la octava de entrada |  |
| — | Bajar la octava de entrada |  |
| `⌥1` | Voz 1 |  |
| `⌥2` | Voz 2 |  |
| `⌥3` | Voz 3 | Pendiente |
| `⌥4` | Voz 4 | Pendiente |
| `;` | Convertir en nota de adorno | Pendiente |
| `⇧⌘F` | Entrada en tiempo real | Pendiente |
| — | Entrada paso a paso |  |
| — | Entrada con teclado MIDI | Pendiente |

## Duración

| Atajo | Comando | Estado |
|---|---|---|
| `1` | Fusa | Pendiente |
| `2` | Semicorchea |  |
| `3` | Corchea |  |
| `4` | Negra |  |
| `5` | Blanca |  |
| `6` | Redonda |  |
| `⌘3` | Tresillo (3:2) |  |
| `.` | Puntillo |  |

## Alteraciones

| Atajo | Comando | Estado |
|---|---|---|
| `7` | Becuadro |  |
| `8` | Sostenido |  |
| `9` | Bemol |  |
| — | Seguir la armadura |  |

## Expresión

| Atajo | Comando | Estado |
|---|---|---|
| `↩` | Ligadura de unión con la siguiente |  |
| `S` | Inicio o fin de ligadura de expresión |  |
| — | Quitar ligadura de unión |  |
| — | Quitar ligadura de expresión |  |
| — | Staccato |  |
| — | Tenuto |  |
| — | Acento |  |
| — | Marcato |  |
| `H` | Crescendo: inicio o fin |  |
| `⇧H` | Diminuendo: inicio o fin |  |
| — | Quitar crescendo |  |
| — | Quitar diminuendo |  |
| — | Pizzicato |  |
| — | Arco |  |
| — | Armónico natural |  |
| — | Armónico artificial |  |
| — | Quitar indicación de técnica |  |
| — | Quitar articulación |  |
| — | Arpegio en ambas manos |  |
| — | Arpegio |  |
| — | Arpegio ascendente |  |
| — | Arpegio descendente |  |
| — | Calderón |  |
| — | Trémolo de una barra |  |
| — | Trémolo de dos barras |  |
| — | Trémolo de tres barras |  |
| — | Dinámica ppp |  |
| — | Dinámica pp |  |
| — | Dinámica p |  |
| — | Dinámica mp |  |
| — | Dinámica mf |  |
| — | Dinámica f |  |
| — | Dinámica ff |  |
| — | Dinámica fff |  |
| — | Quitar dinámica |  |
| `⌘E` | Texto de expresión | Pendiente |
| `⌘T` | Texto de técnica | Pendiente |
| `⌥⌘T` | Indicación de tempo… |  |
| `⌘L` | Letra | Pendiente |
| `⌘K` | Cifrado de acordes | Pendiente |
| `⌘R` | Marca de ensayo |  |
| — | Marcas de ensayo con letras |  |
| — | Marcas de ensayo con números |  |
| — | Marcas de ensayo con números de compás |  |

## Notación

| Atajo | Comando | Estado |
|---|---|---|
| — | Indicación div. |  |
| — | Indicación unis. |  |
| `K` | Tonalidad… |  |
| `T` | Compás de tiempo… |  |
| `Q` | Clave… |  |
| `L` | Líneas… | Pendiente |
| `Z` | Símbolo… | Pendiente |

## Compases

| Atajo | Comando | Estado |
|---|---|---|
| `⌘B` | Añadir un compás al final |  |
| `⇧⌘B` | Insertar un compás antes del actual |  |
| `⌘⌫` | Eliminar compases seleccionados |  |
| `⌥⌘G` | Ir al compás… | Pendiente |
| — | Inicio de repetición |  |
| — | Fin de repetición |  |
| — | Primera casilla |  |
| — | Segunda casilla |  |
| — | Quitar casilla |  |
| — | Doble barra |  |
| — | Barra final |  |
| — | Segno |  |
| — | Coda |  |
| — | To Coda |  |
| — | Fine |  |
| — | D.C. |  |
| — | D.C. al Fine |  |
| — | D.C. al Coda |  |
| — | D.S. |  |
| — | D.S. al Fine |  |
| — | D.S. al Coda |  |
| — | Quitar repeticiones y saltos |  |

## Instrumentos

| Atajo | Comando | Estado |
|---|---|---|
| — | Dividir el pasaje en dos pentagramas |  |
| — | Volver a un pentagrama desde esta posición |  |
| `I` | Añadir instrumentos… |  |
| — | Añadir piano a dos manos |  |
| — | Ossia | Pendiente |
| — | Eliminar el instrumento seleccionado |  |
| — | Instrumento siguiente |  |
| — | Instrumento anterior |  |
| — | Piano: mano derecha |  |
| — | Piano: mano izquierda |  |

## Navegación

| Atajo | Comando | Estado |
|---|---|---|
| `⇧←` | Ampliar selección hacia atrás |  |
| `⇧→` | Ampliar selección hacia delante |  |
| `←` · `⇧⇥` | Elemento anterior |  |
| `→` · `⇥` | Elemento siguiente |  |
| `↑` | Subir un grado |  |
| `↓` | Bajar un grado |  |
| `⌘↑` | Subir una octava |  |
| `⌘↓` | Bajar una octava |  |
| `⌥↑` | Nota superior del acorde o pentagrama de arriba |  |
| `⌥↓` | Nota inferior del acorde o pentagrama de abajo |  |
| `⌘←` | Inicio del compás | Pendiente |
| `⌘→` | Compás siguiente | Pendiente |

## Reproducción

| Atajo | Comando | Estado |
|---|---|---|
| `Espacio` | Reproducir, pausar o continuar |  |
| `P` | Reproducir desde la selección |  |
| — | Reproducir desde el inicio |  |
| `⇧O` | Detener la reproducción |  |
| `⌘[` | Llevar la línea de reproducción al inicio | Pendiente |
| `⌥P` | Reproducción en bucle | Pendiente |
| `⌥Espacio` | Repetir la última reproducción | Pendiente |
| `⇧Espacio` | Reproducir a media velocidad | Pendiente |
| `M` | Mostrar el mezclador |  |
| — | Mostrar los sonidos |  |
| — | Audición al escribir |  |
| — | Subir el tempo |  |
| — | Bajar el tempo |  |
| — | Reproducir repeticiones |  |

## Revisión

| Atajo | Comando | Estado |
|---|---|---|
| — | Comprobar toda la partitura |  |
| — | Nuevo comentario sobre la selección… |  |
| — | Mostrar comentarios |  |
| — | Resolver o reabrir el comentario activo |  |
| — | Comprobar la selección |  |
| — | Mostrar avisos de revisión |  |
| — | Aviso anterior |  |
| — | Aviso siguiente |  |
| — | Mostrar notas fuera de rango en rojo |  |
| — | Editar rango de la parte activa… |  |
| — | Restablecer rango de la parte activa |  |

## Vista

| Atajo | Comando | Estado |
|---|---|---|
| `⌘+` · `⌘=` | Acercar |  |
| `⌘-` | Alejar |  |
| `⌘1` | Tamaño real (100 %) |  |
| `⌘0` | Ajustar a la página |  |
| `⌘U` | Pantalla completa |  |
| — | Mostrar u ocultar instrumentos |  |
| — | Mostrar u ocultar propiedades |  |
| — | Vista de página |  |
| — | Vista continua |  |
| — | Vista de pentagrama |  |
| `⌘F` | Buscar comandos… |  |
| `⌥⌘1` | Pestaña Inicio |  |
| `⌥⌘2` | Pestaña Entrada |  |
| `⌥⌘3` | Pestaña Notación |  |
| `⌥⌘4` | Pestaña Texto |  |
| `⌥⌘5` | Pestaña Reproducción |  |
| `⌥⌘6` | Pestaña Revisión |  |
| `⌥⌘7` | Pestaña Vista |  |
| — | Plegar o desplegar la cinta |  |

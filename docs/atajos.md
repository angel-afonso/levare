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
| Sin atajo | Exportar partitura y partes… |  |
| Sin atajo | Exportar MusicXML… |  |
| Sin atajo | Información de la partitura… |  |
| `⌘,` | Preferencias de atajos… |  |
| Sin atajo | Borrar la lista de recientes |  |
| Sin atajo | Buscar actualizaciones… |  |
| Sin atajo | Reiniciar para actualizar |  |
| Sin atajo | Activar o desactivar la búsqueda automática de actualizaciones |  |
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
| Sin atajo | Eliminar notas y su tiempo |  |
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
| Sin atajo | Subir la octava de entrada |  |
| Sin atajo | Bajar la octava de entrada |  |
| `⌥1` | Voz 1 |  |
| `⌥2` | Voz 2 |  |
| `⌥3` | Voz 3 | Pendiente |
| `⌥4` | Voz 4 | Pendiente |
| `;` | Añadir nota de adorno |  |
| Sin atajo | Acciaccatura |  |
| Sin atajo | Appoggiatura |  |
| `⇧⌘F` | Entrada en tiempo real | Pendiente |
| Sin atajo | Entrada paso a paso |  |
| Sin atajo | Entrada con teclado MIDI | Pendiente |

## Duración

| Atajo | Comando | Estado |
|---|---|---|
| `1` | Fusa |  |
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
| Sin atajo | Seguir la armadura |  |

## Expresión

| Atajo | Comando | Estado |
|---|---|---|
| `↩` | Ligadura de unión con la siguiente |  |
| `S` | Inicio o fin de ligadura de expresión |  |
| Sin atajo | Quitar ligadura de unión |  |
| Sin atajo | Quitar ligadura de expresión |  |
| Sin atajo | Staccato |  |
| Sin atajo | Tenuto |  |
| Sin atajo | Acento |  |
| Sin atajo | Marcato |  |
| `H` | Crescendo: inicio o fin |  |
| `⇧H` | Diminuendo: inicio o fin |  |
| Sin atajo | Quitar crescendo |  |
| Sin atajo | Quitar diminuendo |  |
| Sin atajo | Pizzicato |  |
| Sin atajo | Arco |  |
| Sin atajo | Armónico natural |  |
| Sin atajo | Armónico artificial |  |
| Sin atajo | Quitar indicación de técnica |  |
| Sin atajo | Quitar articulación |  |
| Sin atajo | Arpegio en ambas manos |  |
| Sin atajo | Arpegio |  |
| Sin atajo | Arpegio ascendente |  |
| Sin atajo | Arpegio descendente |  |
| Sin atajo | Calderón |  |
| Sin atajo | Trémolo de una barra |  |
| Sin atajo | Trémolo de dos barras |  |
| Sin atajo | Trémolo de tres barras |  |
| Sin atajo | Dinámica ppp |  |
| Sin atajo | Dinámica pp |  |
| Sin atajo | Dinámica p |  |
| Sin atajo | Dinámica mp |  |
| Sin atajo | Dinámica mf |  |
| Sin atajo | Dinámica f |  |
| Sin atajo | Dinámica ff |  |
| Sin atajo | Dinámica fff |  |
| Sin atajo | Quitar dinámica |  |
| `⌘E` | Texto de expresión… |  |
| `⌘T` | Texto de técnica… |  |
| `⌥⌘T` | Indicación de tempo… |  |
| `⌘L` | Letra… |  |
| `⌘K` | Cifrado de acordes… |  |
| `⌘R` | Marca de ensayo |  |
| Sin atajo | Marcas de ensayo con letras |  |
| Sin atajo | Marcas de ensayo con números |  |
| Sin atajo | Marcas de ensayo con números de compás |  |

## Notación

| Atajo | Comando | Estado |
|---|---|---|
| Sin atajo | Indicación div. |  |
| Sin atajo | Indicación unis. |  |
| Sin atajo | Octava alta 8va |  |
| Sin atajo | Octava baja 8vb |  |
| Sin atajo | Pedal de resonancia |  |
| Sin atajo | Trino |  |
| Sin atajo | Glissando hasta la nota siguiente |  |
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
| `⌥⌘G` | Ir al compás… |  |
| Sin atajo | Inicio de repetición |  |
| Sin atajo | Fin de repetición |  |
| Sin atajo | Primera casilla |  |
| Sin atajo | Segunda casilla |  |
| Sin atajo | Quitar casilla |  |
| Sin atajo | Doble barra |  |
| Sin atajo | Barra final |  |
| Sin atajo | Segno |  |
| Sin atajo | Coda |  |
| Sin atajo | To Coda |  |
| Sin atajo | Fine |  |
| Sin atajo | D.C. |  |
| Sin atajo | D.C. al Fine |  |
| Sin atajo | D.C. al Coda |  |
| Sin atajo | D.S. |  |
| Sin atajo | D.S. al Fine |  |
| Sin atajo | D.S. al Coda |  |
| Sin atajo | Quitar repeticiones y saltos |  |
| Sin atajo | Salto de sistema tras el compás |  |
| Sin atajo | Salto de página tras el compás |  |

## Instrumentos

| Atajo | Comando | Estado |
|---|---|---|
| Sin atajo | Dividir el pasaje en dos pentagramas |  |
| Sin atajo | Volver a un pentagrama desde esta posición |  |
| `I` | Añadir instrumentos… |  |
| Sin atajo | Añadir piano a dos manos |  |
| Sin atajo | Ossia | Pendiente |
| Sin atajo | Eliminar el instrumento seleccionado |  |
| Sin atajo | Instrumento siguiente |  |
| Sin atajo | Instrumento anterior |  |
| Sin atajo | Piano: mano derecha |  |
| Sin atajo | Piano: mano izquierda |  |

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
| `⌘←` | Inicio del compás |  |
| `⌘→` | Compás siguiente |  |

## Reproducción

| Atajo | Comando | Estado |
|---|---|---|
| `Espacio` | Reproducir, pausar o continuar |  |
| `P` | Reproducir desde la selección |  |
| Sin atajo | Reproducir desde el inicio |  |
| `⇧O` | Detener la reproducción |  |
| `⌘[` | Llevar la línea de reproducción al inicio |  |
| `⌥P` | Reproducción en bucle |  |
| `⌥Espacio` | Repetir la última reproducción |  |
| `⇧Espacio` | Reproducir a media velocidad |  |
| Sin atajo | Hacer sonar los cifrados |  |
| `M` | Mezcla de las partes |  |
| Sin atajo | Mostrar el sonido |  |
| Sin atajo | Audición al escribir |  |
| Sin atajo | Subir el tempo |  |
| Sin atajo | Bajar el tempo |  |
| Sin atajo | Reproducir repeticiones |  |

## Revisión

| Atajo | Comando | Estado |
|---|---|---|
| Sin atajo | Comprobar toda la partitura |  |
| Sin atajo | Nuevo comentario sobre la selección… |  |
| Sin atajo | Mostrar comentarios |  |
| Sin atajo | Resolver o reabrir el comentario activo |  |
| Sin atajo | Comprobar la selección |  |
| Sin atajo | Mostrar avisos de revisión |  |
| Sin atajo | Aviso anterior |  |
| Sin atajo | Aviso siguiente |  |
| Sin atajo | Mostrar notas fuera de rango en rojo |  |
| Sin atajo | Editar rango de la parte activa… |  |
| Sin atajo | Restablecer rango de la parte activa |  |

## Vista

| Atajo | Comando | Estado |
|---|---|---|
| `⌘+` · `⌘=` | Acercar |  |
| `⌘-` | Alejar |  |
| `⌘1` | Tamaño real (100 %) |  |
| `⌘0` | Ajustar a la página |  |
| `⌘U` | Pantalla completa |  |
| Sin atajo | Mostrar u ocultar las partes |  |
| Sin atajo | Mostrar u ocultar el panel derecho |  |
| Sin atajo | Panel de agente |  |
| Sin atajo | Vista de página |  |
| Sin atajo | Vista continua |  |
| Sin atajo | Vista de pentagrama |  |
| `⌘F` | Buscar comandos… |  |
| `⌥⌘1` | Pestaña Escribir |  |
| `⌥⌘2` | Pestaña Articulaciones |  |
| `⌥⌘3` | Pestaña Dinámicas |  |
| `⌥⌘4` | Pestaña Texto |  |
| `⌥⌘5` | Pestaña Partitura |  |
| Sin atajo | Pestaña Partitura (saltos de página) |  |
| `⌥⌘6` | Pestaña Reproducción |  |
| `⌥⌘7` | Pestaña Revisión |  |
| `⌥⌘8` | Pestaña Vista |  |
| Sin atajo | Plegar o desplegar la cinta |  |

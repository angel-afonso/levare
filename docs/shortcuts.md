# Keyboard shortcuts

Default profile: **Sibelius · portátil**. Atajos de Sibelius para teclados sin bloque numérico; también funcionan con él.

Change them in **Archivo › Preferencias de atajos…** (`⌘,` on macOS, `Ctrl+,` on Windows and Linux). Your changes are kept apart, in `atajos.json` in the app's data folder, and work on every platform: a shortcut saved with ⌘ on macOS uses Ctrl elsewhere. One with ⌥⌘ and a letter, digit or symbol is left out on Windows and Linux, where Ctrl+Alt is AltGr or a desktop shortcut; Preferences says so and suggests Ctrl+Mayús.

*Pending* commands already have their key and show a notice until the feature exists.

> Generated from `crates/keymap`. To update it: `LEVARE_UPDATE_SHORTCUTS=1 cargo test -p score-keymap`.

## Archivo

| macOS | Windows and Linux | Command | Status |
|---|---|---|---|
| `⌘N` | `Ctrl+N` | Nueva partitura |  |
| `⇧⌘N` | `Ctrl+Mayús+N` | Nueva partitura desde plantilla… |  |
| `⌘O` | `Ctrl+O` | Abrir… |  |
| `⌘S` | `Ctrl+S` | Guardar |  |
| `⇧⌘S` | `Ctrl+Mayús+S` | Guardar como… |  |
| `⌘W` | `Ctrl+W` | Cerrar ventana |  |
| `⌘Q` | `Ctrl+Q` | Salir de Levare |  |
| `⌘P` | `Ctrl+P` | Imprimir… | Pending |
| — | — | Exportar partitura y partes… |  |
| — | — | Exportar MusicXML… |  |
| — | — | Información de la partitura… |  |
| `⌘,` | `Ctrl+,` | Preferencias de atajos… |  |
| — | — | Borrar la lista de recientes |  |
| — | — | Buscar actualizaciones… |  |
| — | — | Reiniciar para actualizar |  |
| — | — | Activar o desactivar la búsqueda automática de actualizaciones |  |
| — | — | Acerca de Levare… |  |
| — | — | Mostrar registros e informes de fallos |  |
| — | — | Informar de un problema… |  |
| `Esc` | `Esc` | Cerrar el panel abierto |  |

## Edición

| macOS | Windows and Linux | Command | Status |
|---|---|---|---|
| `⌘Z` | `Ctrl+Z` | Deshacer |  |
| `⌘Y` · `⇧⌘Z` | `Ctrl+Y` · `Ctrl+Mayús+Z` | Rehacer |  |
| `⌘X` | `Ctrl+X` | Cortar |  |
| `⌘C` | `Ctrl+C` | Copiar |  |
| `⌘V` | `Ctrl+V` | Pegar |  |
| `⌘A` | `Ctrl+A` | Seleccionar todo el pentagrama |  |
| `⌫` · `⌦` | `Retroceso` · `Supr` | Convertir en silencios |  |
| — | — | Eliminar notas y su tiempo |  |
| `⇧⌘P` | `Ctrl+Mayús+P` | Restablecer posición |  |
| `Esc` | `Esc` | Detener, cancelar o quitar la selección |  |
| `R` | `R` | Repetir la nota, acorde o silencio |  |
| `⇧T` | `Mayús+T` | Transportar… |  |
| `X` | `X` | Invertir plica o ligadura |  |
| `J` | `J` | Enarmonizar |  |
| `⇧⌘↑` | `Ctrl+Mayús+↑` | Pasar al pentagrama superior |  |
| `⇧⌘↓` | `Ctrl+Mayús+↓` | Pasar al pentagrama inferior |  |

## Entrada de notas

| macOS | Windows and Linux | Command | Status |
|---|---|---|---|
| `N` | `N` | Activar o desactivar la entrada de notas |  |
| `C` | `C` | Escribir Do |  |
| `D` | `D` | Escribir Re |  |
| `E` | `E` | Escribir Mi |  |
| `F` | `F` | Escribir Fa |  |
| `G` | `G` | Escribir Sol |  |
| `A` | `A` | Escribir La |  |
| `B` | `B` | Escribir Si |  |
| `⇧C` | `Mayús+C` | Añadir Do encima |  |
| `⇧D` | `Mayús+D` | Añadir Re encima |  |
| `⇧E` | `Mayús+E` | Añadir Mi encima |  |
| `⇧F` | `Mayús+F` | Añadir Fa encima |  |
| `⇧G` | `Mayús+G` | Añadir Sol encima |  |
| `⇧A` | `Mayús+A` | Añadir La encima |  |
| `⇧B` | `Mayús+B` | Añadir Si encima |  |
| `⇧1` | `Mayús+1` | Añadir unísono |  |
| `⇧2` | `Mayús+2` | Añadir una segunda encima |  |
| `⇧3` | `Mayús+3` | Añadir una tercera encima |  |
| `⇧4` | `Mayús+4` | Añadir una cuarta encima |  |
| `⇧5` | `Mayús+5` | Añadir una quinta encima |  |
| `⇧6` | `Mayús+6` | Añadir una sexta encima |  |
| `⇧7` | `Mayús+7` | Añadir una séptima encima |  |
| `⇧8` | `Mayús+8` | Añadir una octava encima |  |
| `⇧9` | `Mayús+9` | Añadir una novena encima |  |
| `0` | `0` | Escribir un silencio |  |
| `⌥A` | `Alt+A` | Modo acorde |  |
| `⌥⌫` | `Alt+Retroceso` | Quitar la nota más aguda del acorde |  |
| — | — | Subir la octava de entrada |  |
| — | — | Bajar la octava de entrada |  |
| `⌥1` | `Alt+1` | Voz 1 |  |
| `⌥2` | `Alt+2` | Voz 2 |  |
| `⌥3` | `Alt+3` | Voz 3 |  |
| `⌥4` | `Alt+4` | Voz 4 |  |
| `;` | `;` | Añadir nota de adorno |  |
| — | — | Acciaccatura |  |
| — | — | Appoggiatura |  |
| `⇧⌘F` | `Ctrl+Mayús+F` | Entrada en tiempo real | Pending |
| — | — | Entrada paso a paso |  |
| — | — | Entrada con teclado MIDI | Pending |

## Duración

| macOS | Windows and Linux | Command | Status |
|---|---|---|---|
| — | — | Semifusa |  |
| `1` | `1` | Fusa |  |
| `2` | `2` | Semicorchea |  |
| `3` | `3` | Corchea |  |
| `4` | `4` | Negra |  |
| `5` | `5` | Blanca |  |
| `6` | `6` | Redonda |  |
| `⌘2` | `Ctrl+2` | Dosillo (2:3) |  |
| `⌘3` | `Ctrl+3` | Tresillo (3:2) |  |
| `⌘4` | `Ctrl+4` | Cuatrillo (4:3) |  |
| `⌘5` | `Ctrl+5` | Quintillo (5:4) |  |
| `⌘6` | `Ctrl+6` | Seisillo (6:4) |  |
| `⌘7` | `Ctrl+7` | Septillo (7:4) |  |
| `⌘8` | `Ctrl+8` | Octillo (8:6) |  |
| `⌘9` | `Ctrl+9` | Nonillo (9:8) |  |
| — | — | Otro grupillo… |  |
| `.` | `.` | Puntillo |  |
| — | — | Doble puntillo |  |

## Alteraciones

| macOS | Windows and Linux | Command | Status |
|---|---|---|---|
| `7` | `7` | Becuadro |  |
| `8` | `8` | Sostenido |  |
| `9` | `9` | Bemol |  |
| — | — | Doble sostenido |  |
| — | — | Doble bemol |  |
| — | — | Seguir la armadura |  |

## Expresión

| macOS | Windows and Linux | Command | Status |
|---|---|---|---|
| `↩` | `Intro` | Ligadura de unión con la siguiente |  |
| `S` | `S` | Inicio o fin de ligadura de expresión |  |
| — | — | Quitar ligadura de unión |  |
| — | — | Quitar ligadura de expresión |  |
| — | — | Staccato |  |
| — | — | Tenuto |  |
| — | — | Acento |  |
| — | — | Marcato |  |
| — | — | Staccatissimo |  |
| `H` | `H` | Crescendo: inicio o fin |  |
| `⇧H` | `Mayús+H` | Diminuendo: inicio o fin |  |
| — | — | Quitar crescendo |  |
| — | — | Quitar diminuendo |  |
| — | — | Pizzicato |  |
| — | — | Arco |  |
| — | — | Armónico natural |  |
| — | — | Armónico artificial |  |
| — | — | Quitar indicación de técnica |  |
| — | — | Quitar articulación |  |
| — | — | Arpegio en ambas manos |  |
| — | — | Arpegio |  |
| — | — | Arpegio ascendente |  |
| — | — | Arpegio descendente |  |
| — | — | Calderón |  |
| — | — | Trémolo de una barra |  |
| — | — | Trémolo de dos barras |  |
| — | — | Trémolo de tres barras |  |
| — | — | Dinámica ppp |  |
| — | — | Dinámica pp |  |
| — | — | Dinámica p |  |
| — | — | Dinámica mp |  |
| — | — | Dinámica mf |  |
| — | — | Dinámica f |  |
| — | — | Dinámica ff |  |
| — | — | Dinámica fff |  |
| — | — | Dinámica sfz |  |
| — | — | Dinámica rfz |  |
| — | — | Dinámica fp |  |
| — | — | Dinámica sfp |  |
| — | — | Dinámica niente |  |
| — | — | Quitar dinámica |  |
| `⌘E` | `Ctrl+E` | Texto de expresión… |  |
| `⌘T` | `Ctrl+T` | Texto de técnica… |  |
| `⌥⌘T` | `Ctrl+Mayús+T` | Indicación de tempo… |  |
| `⌘L` | `Ctrl+L` | Letra… |  |
| `⌘K` | `Ctrl+K` | Cifrado de acordes… |  |
| `⌘R` | `Ctrl+R` | Marca de ensayo |  |
| — | — | Marcas de ensayo con letras |  |
| — | — | Marcas de ensayo con números |  |
| — | — | Marcas de ensayo con números de compás |  |

## Notación

| macOS | Windows and Linux | Command | Status |
|---|---|---|---|
| — | — | Indicación div. |  |
| — | — | Indicación unis. |  |
| — | — | Mordente |  |
| — | — | Mordente superior |  |
| — | — | Grupeto |  |
| — | — | Respiración |  |
| — | — | Cesura |  |
| — | — | Octava alta 8va |  |
| — | — | Octava baja 8vb |  |
| — | — | Pedal de resonancia |  |
| — | — | Trino |  |
| — | — | Glissando hasta la nota siguiente |  |
| — | — | Cabeza en x |  |
| — | — | Cabeza de rombo |  |
| — | — | Cabeza diagonal |  |
| — | — | Iniciar barra de corchea |  |
| — | — | Unir a la barra anterior |  |
| — | — | Sin barra de corchea |  |
| `K` | `K` | Tonalidad… |  |
| `T` | `T` | Compás de tiempo… |  |
| `Q` | `Q` | Clave… |  |
| `L` | `L` | Líneas… | Pending |
| `Z` | `Z` | Símbolo… | Pending |

## Compases

| macOS | Windows and Linux | Command | Status |
|---|---|---|---|
| `⌘B` | `Ctrl+B` | Añadir un compás al final |  |
| `⇧⌘B` | `Ctrl+Mayús+B` | Insertar un compás antes del actual |  |
| `⌘⌫` | `Ctrl+Retroceso` | Eliminar compases seleccionados |  |
| `⌥⌘G` | `Ctrl+G` | Ir al compás… |  |
| — | — | Inicio de repetición |  |
| — | — | Fin de repetición |  |
| — | — | Primera casilla |  |
| — | — | Segunda casilla |  |
| — | — | Quitar casilla |  |
| — | — | Doble barra |  |
| — | — | Barra final |  |
| — | — | Barra discontinua |  |
| — | — | Barra punteada |  |
| — | — | Barra de marca |  |
| — | — | Barra corta |  |
| — | — | Barra invisible |  |
| — | — | Compás de repetición (%) |  |
| — | — | Segno |  |
| — | — | Coda |  |
| — | — | To Coda |  |
| — | — | Fine |  |
| — | — | D.C. |  |
| — | — | D.C. al Fine |  |
| — | — | D.C. al Coda |  |
| — | — | D.S. |  |
| — | — | D.S. al Fine |  |
| — | — | D.S. al Coda |  |
| — | — | Quitar repeticiones y saltos |  |
| — | — | Salto de sistema tras el compás |  |
| — | — | Salto de página tras el compás |  |
| — | — | Número de compás en cada sistema |  |
| — | — | Número en cada compás |  |
| — | — | Número de compás cada 5 compases |  |
| — | — | Número de compás cada 10 compases |  |
| — | — | Sin números de compás |  |
| — | — | Número de página al pie |  |
| — | — | Número de página en la esquina |  |
| — | — | Sin números de página |  |
| — | — | Papel automático |  |
| — | — | Papel A4 |  |
| — | — | Papel carta |  |
| — | — | Papel A3 |  |
| — | — | Papel tabloide |  |
| — | — | Página horizontal |  |
| — | — | Ocultar pentagramas vacíos |  |
| — | — | Silencios de varios compases |  |
| — | — | Márgenes normales |  |
| — | — | Márgenes anchos |  |
| — | — | Márgenes muy anchos |  |
| — | — | Tamaño de pentagrama automático |  |
| — | — | Pentagrama de 7 mm |  |
| — | — | Pentagrama de 6 mm |  |
| — | — | Pentagrama de 5 mm |  |
| — | — | Pentagrama de 4 mm |  |

## Instrumentos

| macOS | Windows and Linux | Command | Status |
|---|---|---|---|
| — | — | Dividir el pasaje en dos pentagramas |  |
| — | — | Volver a un pentagrama desde esta posición |  |
| `I` | `I` | Añadir instrumentos… |  |
| — | — | Añadir piano a dos manos |  |
| — | — | Ossia | Pending |
| — | — | Eliminar el instrumento seleccionado |  |
| — | — | Instrumento siguiente |  |
| — | — | Instrumento anterior |  |
| — | — | Piano: mano derecha |  |
| — | — | Piano: mano izquierda |  |

## Navegación

| macOS | Windows and Linux | Command | Status |
|---|---|---|---|
| `⇧←` | `Mayús+←` | Ampliar selección hacia atrás |  |
| `⇧→` | `Mayús+→` | Ampliar selección hacia delante |  |
| `←` · `⇧⇥` | `←` · `Mayús+Tab` | Elemento anterior |  |
| `→` · `⇥` | `→` · `Tab` | Elemento siguiente |  |
| `↑` | `↑` | Subir un grado |  |
| `↓` | `↓` | Bajar un grado |  |
| `⌘↑` | `Ctrl+↑` | Subir una octava |  |
| `⌘↓` | `Ctrl+↓` | Bajar una octava |  |
| `⌥↑` | `Alt+↑` | Nota superior del acorde o pentagrama de arriba |  |
| `⌥↓` | `Alt+↓` | Nota inferior del acorde o pentagrama de abajo |  |
| `⌘←` | `Ctrl+←` | Inicio del compás |  |
| `⌘→` | `Ctrl+→` | Compás siguiente |  |

## Reproducción

| macOS | Windows and Linux | Command | Status |
|---|---|---|---|
| `Espacio` | `Espacio` | Reproducir, pausar o continuar |  |
| `P` | `P` | Reproducir desde la selección |  |
| — | — | Reproducir desde el inicio |  |
| `⇧O` | `Mayús+O` | Detener la reproducción |  |
| `⌘↖` · `⌘[` | `Ctrl+Inicio` · `Ctrl+[` | Llevar la línea de reproducción al inicio |  |
| `⌥P` | `Alt+P` | Reproducción en bucle |  |
| `⌥Espacio` | Windows: `Alt+Espacio` · Linux: `Mayús+Alt+Espacio` | Repetir la última reproducción |  |
| `⇧Espacio` | `Mayús+Espacio` | Reproducir a media velocidad |  |
| — | — | Metrónomo |  |
| — | — | Cuenta previa de un compás |  |
| — | — | Hacer sonar los cifrados |  |
| `M` | `M` | Mezcla de las partes |  |
| — | — | Silenciar parte |  |
| — | — | Solo de la parte |  |
| — | — | Quitar silencios y solos |  |
| — | — | Mostrar el sonido |  |
| — | — | Audición al escribir |  |
| — | — | Subir el tempo |  |
| — | — | Bajar el tempo |  |
| — | — | Reproducir repeticiones |  |

## Revisión

| macOS | Windows and Linux | Command | Status |
|---|---|---|---|
| — | — | Comprobar toda la partitura |  |
| — | — | Nuevo comentario sobre la selección… |  |
| — | — | Mostrar comentarios |  |
| — | — | Resolver o reabrir el comentario activo |  |
| — | — | Comprobar la selección |  |
| — | — | Mostrar avisos de revisión |  |
| — | — | Aviso anterior |  |
| — | — | Aviso siguiente |  |
| — | — | Mostrar notas fuera de rango en rojo |  |
| — | — | Editar rango de la parte activa… |  |
| — | — | Restablecer rango de la parte activa |  |

## Vista

| macOS | Windows and Linux | Command | Status |
|---|---|---|---|
| `⌘+` · `⌘=` | `Ctrl++` · `Ctrl+=` | Acercar |  |
| `⌘-` | `Ctrl+-` | Alejar |  |
| `⌘1` | `Ctrl+1` | Tamaño real (100 %) |  |
| `⌘0` | `Ctrl+0` | Ajustar a la página |  |
| `⌘U` | `F11` | Pantalla completa |  |
| — | — | Mostrar u ocultar las partes |  |
| — | — | Mostrar u ocultar el panel derecho |  |
| — | — | Panel de agente |  |
| — | — | Vista de página |  |
| — | — | Vista continua |  |
| — | — | Vista de pentagrama |  |
| `⇧⌘T` | — | Partitura en Do (altura de concierto) |  |
| `⌘F` | `Ctrl+F` | Buscar comandos… |  |
| `⌥⌘1` | `Ctrl+Mayús+1` | Pestaña Inicio |  |
| `⌥⌘2` | `Ctrl+Mayús+2` | Pestaña Entrada de notas |  |
| `⌥⌘3` | `Ctrl+Mayús+3` | Pestaña Notaciones |  |
| `⌥⌘4` | `Ctrl+Mayús+4` | Pestaña Texto |  |
| — | — | Pestaña Texto (dinámicas) |  |
| `⌥⌘5` | `Ctrl+Mayús+5` | Pestaña Reproducción |  |
| `⌥⌘6` | `Ctrl+Mayús+6` | Pestaña Diseño |  |
| `⌥⌘7` | `Ctrl+Mayús+7` | Pestaña Revisión |  |
| `⌥⌘8` | `Ctrl+Mayús+8` | Pestaña Vista |  |
| — | — | Plegar o desplegar la cinta |  |

## Mouse

Hold these keys and drag from one note to another of the same voice. Linux uses Ctrl because Xfce, Cinnamon and MATE move the window on Alt+drag.

| macOS | Windows | Linux | Line |
|---|---|---|---|
| `⌥` | `Alt` | `Ctrl` | Ligadura de expresión |
| `⌥⌘` | `Ctrl+Alt` | `Ctrl+Alt` | Crescendo |
| `⌥⇧⌘` | `Ctrl+Mayús+Alt` | `Ctrl+Mayús+Alt` | Diminuendo |

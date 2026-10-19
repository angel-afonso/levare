# Atajos de teclado

Perfil por defecto: **Sibelius · portátil**. Atajos de Sibelius para teclados sin bloque numérico; también funcionan con él.

Puedes cambiarlos en **Partitura › Preferencias de atajos…** (⌘,). Tus cambios se guardan aparte, en `atajos.json` dentro de la carpeta de datos de la aplicación.

Los comandos *pendientes* ya tienen su tecla reservada y muestran un aviso hasta que la función exista.

> Archivo generado desde `crates/keymap`. Para actualizarlo: `PARTITURA_ACTUALIZAR_ATAJOS=1 cargo test -p score-keymap`.

## Archivo

| Atajo | Comando | Estado |
|---|---|---|
| `⌘N` | Nueva partitura |  |
| `⌘O` | Abrir… |  |
| `⌘S` | Guardar |  |
| `⇧⌘S` | Guardar como… |  |
| `⌘W` | Cerrar ventana |  |
| `⌘Q` | Salir de Partitura |  |
| `⌘P` | Imprimir… | Pendiente |
| — | Exportar… | Pendiente |
| `⌘,` | Preferencias de atajos… |  |
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
| `⌫` · `⌦` | Eliminar la selección |  |
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
| `⌥1` | Voz 1 | Pendiente |
| `⌥2` | Voz 2 | Pendiente |
| `⌥3` | Voz 3 | Pendiente |
| `⌥4` | Voz 4 | Pendiente |
| `;` | Convertir en nota de adorno | Pendiente |
| `⇧⌘F` | Entrada en tiempo real | Pendiente |

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
| — | Quitar indicación de técnica |  |
| — | Quitar articulación |  |
| `⌘E` | Texto de expresión | Pendiente |
| `⌘T` | Texto de técnica | Pendiente |
| `⌥⌘T` | Texto de tempo | Pendiente |
| `⌘L` | Letra | Pendiente |
| `⌘K` | Cifrado de acordes | Pendiente |
| `⌘R` | Letra de ensayo | Pendiente |

## Notación

| Atajo | Comando | Estado |
|---|---|---|
| `K` | Tonalidad… |  |
| `T` | Compás de tiempo… | Pendiente |
| `Q` | Clave… | Pendiente |
| `L` | Líneas… | Pendiente |
| `Z` | Símbolo… | Pendiente |

## Compases

| Atajo | Comando | Estado |
|---|---|---|
| `⌘B` | Añadir un compás al final |  |
| `⇧⌘B` | Insertar un compás | Pendiente |
| `⌘⌫` | Eliminar compases | Pendiente |
| `⌥⌘G` | Ir al compás… | Pendiente |

## Instrumentos

| Atajo | Comando | Estado |
|---|---|---|
| `I` | Añadir instrumentos… |  |
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

## Vista

| Atajo | Comando | Estado |
|---|---|---|
| `⌘+` · `⌘=` | Acercar |  |
| `⌘-` | Alejar |  |
| `⌘1` | Tamaño real (100 %) |  |
| `⌘0` | Ajustar a la página | Pendiente |
| `⌘U` | Pantalla completa |  |
| — | Mostrar u ocultar instrumentos |  |
| — | Mostrar u ocultar propiedades |  |

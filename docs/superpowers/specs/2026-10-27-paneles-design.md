# Paneles con sentido

Rediseño de los paneles laterales de Levare para que sean realmente prácticos. El usuario aprobó
la maqueta navegable <https://claude.ai/artifact/QH3h8MK2WJ4ac5urJxoPwc> (privada; su fuente está
junto a este documento: `2026-10-27-paneles-maqueta.html`). Este documento fija lo que decide.

## Problemas de hoy

1. Lo mismo en tres o cuatro sitios: puntillo, tresillo, alteración (dos veces dentro de
   Propiedades y en otro orden que en la cinta), voz, div./unis., silencio, acorde, octava,
   articulaciones, dinámicas, reguladores y ligaduras están en la cinta y en Propiedades; compás,
   tiempo, tempo, métrica, tonalidad y voz, en la tira de arriba, el transporte, la barra de estado y
   Propiedades; «Añadir instrumento» cuatro veces.
2. El panel izquierdo hace dos trabajos: lista de instrumentos y anfitrión de los editores de clave,
   tonalidad, métrica, tempo, texto, letra, cifrado e información de la partitura, que empujan la
   lista. Bajo ella, un texto fijo «Orden automático por familias» y un bloque «Vista» que repite la
   cinta.
3. Propiedades no cambia con lo seleccionado, está llena de párrafos de ayuda, tiene dos deslizadores
   «Pendiente» que no hacen nada y altura y duración de solo lectura.
4. Con 32 partes, la lista y el Mezclador (un bloque con dos deslizadores por parte) son pantallas de
   desplazamiento.
5. No se recuerda nada: paneles abiertos y pestaña elegida se pierden al cerrar.
6. Las pestañas del inspector no caben en una línea.

## Decisiones

### Izquierda: «¿Qué hay en la partitura?»

- Cabecera: «Partes» con el número de partes y un único botón «+» (Añadir instrumentos, tecla I).
  Debajo, un selector de dos opciones «Lista | Mezcla» y un campo «Buscar parte» que filtra por
  nombre (sin distinguir mayúsculas ni acentos).
- Partes agrupadas por familia; cada familia es una cabecera plegable con su número de partes. Una
  búsqueda muestra las familias desplegadas.
- **Lista:** una fila de 28px por parte: nombre a la izquierda; a la derecha, en `MUTED`, su clave o
  claves y la transposición si la tiene («Si♭ · Sol», «Fa · 8vb»). Pulsar la fila la selecciona como
  hoy. La fila elegida lleva el aspecto seleccionado de la cinta. Solo la fila elegida muestra, si la
  parte tiene dos pentagramas, sus chips (M. derecha / M. izquierda o Divisi I / II) y el botón de
  eliminar.
- **Mezcla:** cada fila añade volumen y posición estéreo (los deslizadores que hoy están en el
  Mezclador) y, para cuerda, madera y metal, «Sección | Solista». Los ajustes de volumen y posición
  siguen siendo de sesión; Solista o Sección se guarda y se deshace como hoy.
- Pie: una línea discreta con la parte elegida y cómo moverse («⌥↑ ⌥↓»), o en Mezcla el volumen
  general.
- Fuera de la izquierda: todos los editores, el texto «Orden automático por familias» y el bloque
  «Vista»/«Páginas». La navegación por páginas sigue disponible donde ya está en la cinta y los menús;
  si hoy solo vive aquí, pasa a la barra de estado inferior.

### Derecha: «¿Qué tengo seleccionado y qué le cambio?»

Tres pestañas en una línea, como selector de tres opciones: **Selección · Sonido · Revisión**.

**Selección** cambia con lo seleccionado:

- **Nada:** «Partitura». Título, subtítulo y compositor editables (el editor de información de la
  partitura). Filas que abren su editor: Tonalidad, Métrica, Tempo inicial. Datos: Compases, Partes,
  Duración.
- **Nota o acorde:** el nombre de la parte; «Compás N, tiempo T · voz V»; la altura grande (escrita) y
  la duración; «Suena …» si la parte transporta; el aviso de registro si lo hay; **Marcas en esta
  nota**: una fila por marca (articulación, dinámica, técnica, ligaduras y reguladores que empiezan o
  acaban en ella, adornos, textos) con su glifo, su nombre y un botón ✕ que la quita (deshacible);
  **Texto en esta nota**: Letra ⌘L, Cifrado ⌘K, Comentario.
- **Silencio:** como la nota, sin altura ni «Suena», con sus marcas.
- **Pasaje:** «Pasaje» con partes y compases que abarca; Notas, Partes, Marcas; **Hacer con el
  pasaje**: comandos que ya existen (subir/bajar un grado, subir/bajar una octava, Copiar,
  Convertir en silencios, Comprobar selección); **Marcas en el pasaje** con ✕, cada una con su compás.
- **Editor abierto** (clave, tonalidad, métrica, tempo, texto, letra, cifrado, información): el editor
  ocupa la pestaña, con Aplicar y Cancelar (Esc). Abrirlo desde la cinta, el menú o la tira de arriba
  muestra el panel derecho y la pestaña Selección.
- Sin párrafos de ayuda (su texto va a la ayuda al pasar el cursor) ni controles que ya están en la
  cinta, ni los deslizadores «Pendiente».

**Sonido:** salida (volumen general, reverberación y sus ajustes), sonido de la parte elegida (banco,
sonido, interpretación natural/literal) y Más sonidos, como hoy. El ajuste por parte de volumen y
posición vive en Mezcla, a la izquierda.

**Revisión:** lo que hoy tiene la pestaña Revisión (comprobar, avisos, rangos, comentarios), con sus
párrafos de ayuda movidos a la ayuda al pasar el cursor.

### Arriba: «¿Dónde estoy?»

La tira sobre la partitura es el único sitio de posición y contexto: Compás, Tiempo | Tempo, Métrica,
Tonalidad (editables: abren su editor en el panel derecho) | Parte · Voz | resumen de la selección.
Tonalidad deja de abrir el panel izquierdo.

### Abajo

La barra de estado se queda con el mensaje de estado, la navegación de páginas en modo Página y el
estado de guardado y autoguardado. Deja de repetir voz, tonalidad, métrica, instrumentos y modo de
vista.

### Memoria

Se guardan en la carpeta de datos, en `paneles.json`: qué paneles están abiertos, la pestaña derecha,
Lista o Mezcla y las familias plegadas. Si el archivo falta o está dañado, se usan los valores por
defecto (izquierda y derecha abiertas, Selección, Lista, nada plegado).

## Fuera

- Funciones nuevas (transponer por intervalo libre, silencio/solo por parte, paneles redimensionables).
- La cinta, la barra de título y el transporte salvo lo dicho arriba.

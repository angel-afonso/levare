# Cinta clara

Rediseño de la cinta (toolbar) de Levare para que sea muy fácil de usar y entender. El usuario
aprobó la maqueta navegable: <https://claude.ai/artifact/Uu2pdnXpMEo2Mx2KBbYJyq> (privada; su
fuente está en el plan). Este documento fija lo que la maqueta decide.

## Problemas de hoy

1. Cinco formas de dibujar un botón (segmentos hundidos, cuadros con borde cuando están
   apagados, glifos sueltos, botones grandes con texto, filas pequeñas) y cada grupo en una
   tarjeta gris: todo tiene caja y no se distingue qué está activo, qué es elección y qué es acción.
2. Símbolos musicales sin nombre ni tecla visibles.
3. Pestañas desequilibradas: Disposición con un grupo de dos botones; Notación con diez grupos
   que se pliegan; Inicio mezcla portapapeles, instrumentos y compases.

## Decisiones

### Una sola gramática de control

- En reposo: el glifo o el texto sobre el panel, sin fondo ni borde.
- Hover: un paso de tono (`HOVER`).
- Activo o elegido (toggle o choice, igual): relleno `SELECT`, borde `SELECT_BORDER`, tinta
  `ACCENT_TEXT`.
- Las opciones que se excluyen (duración, alteración, voz, estilo de ensayo, modo de vista)
  comparten un carril `FIELD` con 2px de relleno.
- Acción: sin estado; solo hover y pulsación.
- Los grupos no tienen tarjeta: se separan con espacio (22px) y llevan su nombre debajo, 11px,
  `MUTED`, alineado a la izquierda.
- Pendiente: sigue con borde discontinuo (`PENDING_OUTLINE`).

### Nombre y tecla visibles

Cada control es una baldosa: glifo arriba (28px de alto), debajo el nombre (10,5px, `SECONDARY`) y
bajo el nombre la tecla (10px, `MUTED`), p. ej. «Negra» y «4». Nombre y tecla van en dos líneas para
que la baldosa mida lo que el más ancho de los dos, no su suma: en una línea, Escribir no cabía a
1440px. Donde el título del grupo ya da el contexto, el nombre es corto («Unión», «Arriba»). Cuando falta ancho, el grupo pasa a modo
normal: glifo y tecla, sin nombre. Si aún falta, se pliega en su botón desplegable, como hoy. La
ayuda al pasar el cursor muestra nombre, tecla y una frase de qué hace.

### Pestañas por tarea

| Pestaña | Grupos |
|---|---|
| Escribir | Entrada (Escribir N, octava ↑ ↓) · Duración · Modificar (puntillo, tresillo, silencio) · Alteración · Ligadura (unión, expresión) · Voz · Acorde (modo acorde; intervalos en desplegable) |
| Articulaciones | Articulación (staccato, tenuto, acento, marcato, calderón) · Nota de adorno · Arpegio · Trémolo · Técnica de cuerda |
| Dinámicas | Dinámica (ppp…fff) · Regulador · Líneas (8va, 8vb, pedal, trino, glissando) · Divisi |
| Texto | Partitura (título y créditos) · Tempo · Sobre el pentagrama · Letra y cifrado · Marca de ensayo · Estilo de ensayo |
| Partitura | Instrumentos · Compases · Cambios (clave, tonalidad, compás) · Repeticiones · Da capo y segno (D.C./D.S. en desplegable) · Sistema y página |
| Reproducción | Reproducir · Práctica · Sonido |
| Revisión | Comprobar · Avisos · Registro del instrumento · Comentarios |
| Vista | Modo · Zoom · Cinta |

El portapapeles sale de la cinta (menú Edición y teclas). Los paneles salen de Vista (están en la
cabecera). A 1440px de ancho ninguna pestaña se pliega.

### Pestañas y estado

- Pestañas de texto con un indicador azul de 2px bajo la elegida; sin fondo redondeado.
- La pastilla de estado a la derecha de las pestañas dice siempre qué se va a escribir:
  «Escribiendo · Negra con puntillo · Voz 2», con un punto azul cuando la entrada está activa.

### Atajos de pestaña

⌥⌘1–8 siguen el orden nuevo. Los IDs publicados no cambian de nombre: `TAB_NOTE_INPUT` abre
Escribir, `TAB_NOTATION` Articulaciones, `TAB_HOME` Partitura, `TAB_TEXT`, `TAB_PLAYBACK`,
`TAB_REVIEW` y `TAB_VIEW` las suyas; se añade `TAB_DYNAMICS`. `TAB_LAYOUT` sigue registrado,
abre Partitura y queda sin tecla por defecto.

## Fuera

- Cambios en los menús, el panel de propiedades o la barra de estado inferior.
- Nuevas funciones: solo se reorganiza y se redibuja lo que existe.

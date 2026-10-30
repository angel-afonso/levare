# Probar Levare

Gracias por probar Levare. Esta guía es para quien lo prueba escribiendo música de verdad:
cómo instalarlo, qué conviene probar, qué todavía no hace y cómo contar lo que falla.

Levare está en desarrollo. Funciona y guarda tus partituras, pero vas a encontrar fallos:
encontrarlos es justo lo que buscamos.

## Instalar

Descarga la última versión desde [la página de versiones](https://github.com/angel-afonso/levare/releases/latest).

- **macOS** (11 o posterior): abre el `.dmg` y arrastra Levare a Aplicaciones. La
  primera vez macOS avisa de que no puede comprobar la app. Cierra el aviso, ve a
  **Ajustes del Sistema › Privacidad y seguridad**, baja hasta el mensaje sobre Levare y
  pulsa **Abrir igualmente**. En macOS 14 o anterior basta con clic derecho › Abrir.
- **Windows** (64 bits): descomprime el `.zip` en una carpeta tuya (Documentos, por
  ejemplo, no Archivos de programa) y abre `levare.exe`. Si SmartScreen avisa: **Más
  información › Ejecutar de todas formas**.
- **Linux** (x86_64): descomprime el `.tar.gz` y ejecuta `./install.sh` para añadir
  Levare al menú de aplicaciones.

Levare te avisa cuando hay una versión nueva y se actualiza solo. Instala siempre la
última antes de informar de un fallo: puede que ya esté arreglado.

## Antes de empezar

- **Guarda copia** de cualquier partitura que te importe. Levare guarda una recuperación
  automática mientras trabajas, pero es una versión de prueba.
- Un archivo `.levare` guardado con una versión nueva no se abre en una anterior. Si
  alguien más prueba Levare contigo, usad la misma versión.
- Los atajos siguen a Sibelius. **Buscar comandos** (`⌘F` en macOS, `Ctrl+F` en Windows y
  Linux) encuentra cualquier orden por su nombre, y la [lista de atajos](atajos.md) los
  tiene todos.

## Qué probar

Lo más útil es que trabajes como siempre, con un encargo real o uno reciente. Si
necesitas un punto de partida:

1. **Un arreglo desde cero.** Añade los instrumentos (`I`), escribe con la entrada de
   notas (`N`), pon dinámicas, articulaciones, ligaduras, cifrado, ensayos y
   repeticiones. Fíjate en lo que te frena o te obliga a usar el ratón.
2. **Traer algo que ya tienes.** Exporta un arreglo a MusicXML desde Sibelius, Dorico,
   Finale o MuseScore y ábrelo en Levare (**Archivo › Abrir…**). Di qué se pierde o
   cambia.
3. **Las partes.** **Archivo › Exportar partitura y partes…** crea una carpeta con la
   partitura general y una parte por instrumento, en PDF, MusicXML o ambos. Revisa las
   partes como si fueras a repartirlas: transposición, armaduras, saltos de página,
   legibilidad.
4. **Escucharlo.** Reproduce (`P` desde la selección), prueba la mezcla (`M`) y los
   sonidos de **Más sonidos**, en la pestaña **Sonido** del panel derecho.
5. **Llevarlo de vuelta.** Exporta MusicXML desde Levare y ábrelo en tu programa
   habitual.

## Qué todavía no hace

Esto ya lo sabemos; no hace falta informar de ello, aunque sí nos ayuda saber cuál te
hace más falta:

- Silencios de varios compases en las partes.
- Grupillos que no sean tresillos (dosillos, cuatrillos, quintillos, seisillos…).
- Ver la partitura en Do o transpositora a voluntad.
- MIDI: tocar desde un teclado MIDI, importar o exportar `.mid`.
- Voces 3 y 4, plicas manuales.
- Imprimir directamente: exporta el PDF e imprímelo.
- Elegir tamaño de papel (Carta, A3…) o el tamaño del pentagrama.
- Varias partituras abiertas en pestañas, buscar en la partitura, silencio y solo por
  parte.

## Cómo informar de un problema

En macOS, **Ayuda › Informar de un problema…**; en Windows y Linux está en el menú
**Archivo**. Abre un formulario en GitHub (hace falta una cuenta gratuita) con tu
versión y tu sistema ya puestos. Cuenta:

- qué hiciste, paso a paso;
- qué esperabas y qué pasó;
- si se repite.

Adjunta la partitura si puedes: GitHub no acepta archivos `.levare` tal cual, así que
comprímela en un `.zip`. Una captura o el PDF exportado también ayudan mucho.

Si no tienes cuenta de GitHub, envía lo mismo a quien te pasó Levare.

Un fallo por informe, por favor: si encuentras tres, son tres informes.

## Si Levare se cierra de golpe

Al volver a abrirlo, Levare recupera lo que no habías guardado (revísalo y guárdalo) y
te avisa de que guardó un informe del fallo. **Mostrar registros e informes de fallos** (en el mismo
menú que Informar de un problema) abre la carpeta donde están. Adjunta el `fallo-….txt`
más reciente y `registro.log` a tu informe.

Esa carpeta está en:

| Sistema | Carpeta |
| --- | --- |
| macOS | `~/Library/Application Support/Levare` |
| Windows | `%LOCALAPPDATA%\Levare` |
| Linux | `~/.local/share/Levare` |

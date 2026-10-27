# Bibliotecas SFZ en la pestaña Sonido

Levare puede cargar varias bibliotecas SFZ locales con el mismo motor sfizz. Instala
cada biblioteca y su manifiesto en una carpeta propia bajo `soundpacks/` junto al
ejecutable, en alguno de sus directorios antecesores, o bajo la carpeta de datos de
Levare (`soundpacks/`). La app descubre cada `manifest.json`, muestra los paquetes
válidos en la pestaña **Sonido** del panel derecho y conserva los errores de los paquetes inválidos para que
puedas corregirlos. GeneralUser GS sigue disponible como respaldo para instrumentos
o teclas que un paquete no cubra. Las bibliotecas y sus licencias pertenecen a sus
respectivos autores.

## Más sonidos

**Sonido › Más sonidos** instala paquetes del catálogo que viene dentro de la app,
uno cada vez y en segundo plano, sin dejar de escribir. Cada paquete muestra su
descarga, lo que ocupa instalado y su licencia; **Instalar…** enseña la licencia y
**Aceptar e instalar** empieza. La descarga se comprueba con el SHA-256 publicado en el
catálogo, se descomprime en una carpeta temporal y solo pasa a
`soundpacks/<id>` de la carpeta de datos cuando su manifiesto es válido; una
actualización que falla deja la versión anterior. **Cancelar** no deja nada a medias.
**Quitar…** pide confirmación y borra solo paquetes instalados desde aquí: los que
copias a mano no se tocan.

Un paquete solo trae muestras y texto SFZ. Al descomprimir se descartan bibliotecas,
guiones y cualquier archivo que no sea audio, SFZ o documentación, y una ruta que
intente salir de su carpeta rechaza el paquete entero. El motor sfizz va dentro de
Levare (`Contents/Frameworks`): un manifiesto instalado desde el catálogo no puede
indicar `engine`. Fuera de la app, en desarrollo, se usa el sfizz de
`soundpacks/vsco-ce/` o el que indique `LEVARE_SFIZZ`.

Hay dos clases de entradas. Las que Levare publica, como **VSCO CE · Alta calidad**,
solo se hacen con bibliotecas que permiten redistribuirse; se construyen con
`scripts/build_soundpack_vsco.py`, que convierte a FLAC sin pérdida las grabaciones que
Levare usa y escribe un zip reproducible con su SHA-256. Las bibliotecas cuya licencia
pide no redistribuirlas se instalan con una receta: Levare las descarga de la web de su
autor, comprueba el checksum y añade su propio manifiesto. Si el autor cambia el archivo,
la receta falla hasta que se actualice el checksum del catálogo.

Para publicar un paquete: construye el zip, súbelo como recurso de una versión de GitHub
con la etiqueta `sonidos-<id>-<versión>` y añade su URL, tamaño y SHA-256 a
`crates/sound-store/src/catalog.json`. Una versión nueva de un paquete sube `version`, y
los usuarios ven **Actualizar**.

## Manifiesto de formato 2

Este ejemplo usa rutas verificadas de una instalación local de Virtual Playing
Orchestra 3. Las rutas son relativas al manifiesto, excepto las de los instrumentos,
que son relativas a `samples`:

```json
{
  "format": 2,
  "kind": "sfz",
  "id": "vpo",
  "name": "Virtual Playing Orchestra · Performance",
  "samples": "Virtual-Playing-Orchestra3",
  "license": "Virtual-Playing-Orchestra3/Documentation/license.htm",
  "instruments": {
    "Violin": {
      "sustain": "Strings/1st-violin-SEC-PERF.sfz",
      "pizzicato": "Strings/1st-violin-SEC-PERF-pizzicato.sfz",
      "staccato": "Strings/1st-violin-SEC-PERF-staccato.sfz",
      "dynamics": "cc1",
      "spread_layers": false,
      "second": {
        "sustain": "Strings/2nd-violin-SEC-PERF.sfz",
        "pizzicato": "Strings/2nd-violin-SEC-PERF-pizzicato.sfz",
        "staccato": "Strings/2nd-violin-SEC-PERF-staccato.sfz"
      },
      "tremolo": "Strings/1st-violin-SEC-PERF-tremolo.sfz",
      "solo": {
        "sustain": "Strings/1st-violin-SOLO-PERF.sfz",
        "pizzicato": "Strings/1st-violin-SOLO-PERF-pizzicato.sfz",
        "staccato": "Strings/1st-violin-SOLO-PERF-staccato.sfz",
        "second": {
          "sustain": "Strings/2nd-violin-SOLO-PERF.sfz",
          "pizzicato": "Strings/2nd-violin-SOLO-PERF-pizzicato.sfz",
          "staccato": "Strings/2nd-violin-SOLO-PERF-staccato.sfz"
        }
      }
    },
    "Flute": {
      "sustain": "Woodwinds/flute-SOLO-PERF.sfz",
      "staccato": "Woodwinds/flute-SOLO-PERF-staccato.sfz",
      "dynamics": "cc1",
      "section": {
        "sustain": "Woodwinds/flute-SEC-PERF.sfz",
        "staccato": "Woodwinds/flute-SEC-PERF-staccato.sfz"
      }
    }
  }
}
```

| Campo | Uso |
| --- | --- |
| `format`, `kind` | Deben ser `2` y `"sfz"`. |
| `id`, `name` | Identificador estable y nombre visible. `sfz-combinadas` está reservado. |
| `samples` | Carpeta raíz de los SFZ y las muestras que estos referencian. |
| `engine` | Opcional: biblioteca dinámica de sfizz. Sin él se usa el motor de Levare; los paquetes del catálogo no pueden indicarlo. |
| `license` | Ruta opcional; si se declara, el archivo debe existir. |
| `instruments` | Claves con los nombres de instrumento del modelo de Levare. |
| `sustain` | SFZ obligatorio de cada instrumento declarado. |
| `pizzicato`, `staccato` | SFZ opcionales para esas técnicas. |
| `tremolo` | SFZ opcional de trémolo de arco, para los trémolos de tres barras. |
| `dynamics` | `"velocity"` por defecto o `"cc1"` para bibliotecas de modulación. |
| `spread_layers` | `false` por defecto; activa el reparto de capas de velocidad de VSCO cuando corresponda. |
| `solo`, `section` | Grabaciones opcionales para una parte de solista o de sección, con los mismos campos. Sin ellas se usan las principales. |
| `second` | Grabaciones opcionales para la segunda parte del mismo instrumento, como los violines II, dentro del conjunto principal, de `solo` o de `section`. |

`dynamics` y `spread_layers` se heredan si no se repiten: `solo` y `section` toman los
del instrumento, y `second` los de su conjunto.

## Solista o sección

Cada parte de cuerda, madera o metal tiene en su fila de **Mezcla** (panel de partes, a la
izquierda) el selector **Sección | Solista**. Por defecto, las cuerdas son secciones si la partitura tiene
más de cuatro partes de cuerda y solistas en música de cámara; una parte llamada
«solista» o «solo», y cualquier madera o metal, es solista. La elección se guarda en la
partitura y se puede deshacer. Si dos partes del mismo instrumento tocan del mismo modo,
la segunda usa `second` cuando el paquete lo declara, de modo que dos secciones al
unísono no repiten las mismas muestras; una tercera vuelve al conjunto principal.

Cuando una parte repite las grabaciones de otra (una segunda flauta sin `second`, o los
violines III), se afina unos cents aparte: +5 la primera repetición, −5 la siguiente,
luego ±9 y como mucho ±13. Dos intérpretes al unísono nunca están exactamente afinados
entre sí, y dos copias idénticas de una muestra suenan como una sola que vibra. El ajuste
es fijo para cada parte, así que dos reproducciones suenan igual.

## Trémolo

Un trémolo de tres barras en la cuerda no está medido: la reproducción sigue escribiendo
fusas para cualquier banco, pero si el parche del instrumento declara `tremolo`, sostiene
esa grabación durante toda la nota y deja pasar las repeticiones. Los trémolos de una y
dos barras siguen siendo ataques medidos. Sin grabación de trémolo, o con pizzicato, se
oyen las repeticiones como antes.

## Comparar sonidos

`examples/ab-tremolo-y-unisonos.partitura` reúne flautas y trompas al unísono y un
acorde de cuerdas con trémolo de tres barras y luego de dos. Para escucharlo con un
paquete concreto:

```bash
cargo run --release --example export_audio -- examples/ab-tremolo-y-unisonos.partitura salida.wav vpo
```

El cargador comprueba el motor, la licencia declarada y todos los SFZ declarados.
Para elegir parches, escucha sus registros y articulaciones reales: no basta con el
nombre del archivo. En VPO, los parches *Performance* usan `gain_cc1` además de
fundidos `xfin_locc1`; por eso conviene `dynamics: "cc1"`. Otras bibliotecas pueden
usar `amp_velcurve` o capas de velocidad y requieren `"velocity"`. El control CC1 se
envía antes de cada ataque cuando cambia el nivel de esa voz, incluso en el primer
ataque a nivel cero. CC11 sigue controlando la expresión continua del transporte.

**Más sonidos** instala Virtual Playing Orchestra 3 con una receta: descarga de la web
de su autor las muestras 3.2 (616 MB, alojadas en su Google Drive) y los guiones
Performance 3.3, comprueba sus SHA-256, los descomprime en la misma carpeta, como pide
el autor, y añade este manifiesto, con solistas, secciones, segundos violines y
trémolos. Lo que sigue describe una instalación hecha a mano.

En la instalación VPO local, los SFZ Performance están en
`Virtual-Playing-Orchestra3/Virtual-Playing-Orchestra3/`, mientras que sus WAV están
en la carpeta hermana `Virtual-Playing-Orchestra3 2/libs/`. Un enlace relativo
`Virtual-Playing-Orchestra3/Virtual-Playing-Orchestra3/libs` hacia
`../Virtual-Playing-Orchestra3 2/libs` permite resolver las referencias `../libs`
del SFZ sin mover ni modificar los archivos originales. Comprueba siempre las rutas
de tu propia descarga. Los parches de caja y platillos disponibles en esta instalación
no cubren las teclas General MIDI 38 y 49 usadas por Levare, por lo que no se
declaran y esas notas se reproducen con GeneralUser.

## Prioridad y compatibilidad

Si hay dos o más paquetes, **Bibliotecas combinadas** elige el primero que tenga
parche para cada instrumento; si ninguno cubre el ataque, se usa GeneralUser.
Los botones de prioridad de la pestaña Sonido cambian ese orden. La lista se guarda en
`sonidos.json` de la carpeta de datos como `{"version":1,"priority":["vpo","vsco-ce"]}`;
los paquetes nuevos no enumerados se añaden por identificador en orden alfabético.
Cambiar de banco o de prioridad detiene la reproducción y no altera la partitura.

El paquete VSCO CE existente conserva su manifiesto de formato 1 (`kind: "vsco-ce"`)
y sus rutas originales. [Instalación y detalles de VSCO](vsco.md). Sonatina y otras
bibliotecas SFZ gratuitas pueden añadirse con su propio manifiesto, comprobando
primero las rutas, rangos de notas, articulaciones y condiciones de licencia reales.

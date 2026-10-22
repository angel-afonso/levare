# Bibliotecas SFZ en Sonidos

Levare puede cargar varias bibliotecas SFZ locales con el mismo motor sfizz. Instala
cada biblioteca y su manifiesto en una carpeta propia bajo `soundpacks/` junto al
ejecutable, en alguno de sus directorios antecesores, o bajo la carpeta de datos de
Levare (`soundpacks/`). La app descubre cada `manifest.json`, muestra los paquetes
válidos en **Sonidos** y conserva los errores de los paquetes inválidos para que
puedas corregirlos. GeneralUser GS sigue disponible como respaldo para instrumentos
o teclas que un paquete no cubra. Las bibliotecas y sus licencias pertenecen a sus
respectivos autores; Levare no las distribuye.

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
  "engine": "../vsco-ce/libsfizz.dylib",
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
| `engine` | Biblioteca dinámica de sfizz; varios paquetes pueden compartirla. |
| `license` | Ruta opcional; si se declara, el archivo debe existir. |
| `instruments` | Claves con los nombres de instrumento del modelo de Levare. |
| `sustain` | SFZ obligatorio de cada instrumento declarado. |
| `pizzicato`, `staccato` | SFZ opcionales para esas técnicas. |
| `dynamics` | `"velocity"` por defecto o `"cc1"` para bibliotecas de modulación. |
| `spread_layers` | `false` por defecto; activa el reparto de capas de velocidad de VSCO cuando corresponda. |
| `solo`, `section` | Grabaciones opcionales para una parte de solista o de sección, con los mismos campos. Sin ellas se usan las principales. |
| `second` | Grabaciones opcionales para la segunda parte del mismo instrumento, como los violines II, dentro del conjunto principal, de `solo` o de `section`. |

`dynamics` y `spread_layers` se heredan si no se repiten: `solo` y `section` toman los
del instrumento, y `second` los de su conjunto.

## Solista o sección

Cada parte de cuerda, madera o metal tiene en **Mezclador › Intérpretes** la opción
**Solista** o **Sección**. Por defecto, las cuerdas son secciones si la partitura tiene
más de cuatro partes de cuerda y solistas en música de cámara; una parte llamada
«solista» o «solo», y cualquier madera o metal, es solista. La elección se guarda en la
partitura y se puede deshacer. Si dos partes del mismo instrumento tocan del mismo modo,
la segunda usa `second` cuando el paquete lo declara, de modo que dos secciones al
unísono no repiten las mismas muestras; una tercera vuelve al conjunto principal.

El cargador comprueba el motor, la licencia declarada y todos los SFZ declarados.
Para elegir parches, escucha sus registros y articulaciones reales: no basta con el
nombre del archivo. En VPO, los parches *Performance* usan `gain_cc1` además de
fundidos `xfin_locc1`; por eso conviene `dynamics: "cc1"`. Otras bibliotecas pueden
usar `amp_velcurve` o capas de velocidad y requieren `"velocity"`. El control CC1 se
envía antes de cada ataque cuando cambia el nivel de esa voz, incluso en el primer
ataque a nivel cero. CC11 sigue controlando la expresión continua del transporte.

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
Los botones de prioridad de Sonidos cambian ese orden. La lista se guarda en
`sonidos.json` de la carpeta de datos como `{"version":1,"priority":["vpo","vsco-ce"]}`;
los paquetes nuevos no enumerados se añaden por identificador en orden alfabético.
Cambiar de banco o de prioridad detiene la reproducción y no altera la partitura.

El paquete VSCO CE existente conserva su manifiesto de formato 1 (`kind: "vsco-ce"`)
y sus rutas originales. [Instalación y detalles de VSCO](vsco.md). Sonatina y otras
bibliotecas SFZ gratuitas pueden añadirse con su propio manifiesto, comprobando
primero las rutas, rangos de notas, articulaciones y condiciones de licencia reales.

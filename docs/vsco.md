# VSCO CE: paquete de sonido opcional

La integración experimental lee los SFZ y WAV originales de **VSCO 2 Community
Edition 1.1.0** con **sfizz 1.2.3**. No convierte las grabaciones a SoundFont ni
reemplaza GeneralUser. No es un host de plugins VST/AU: es un paquete local con
un motor de reproducción intercambiable desde la aplicación.

## Probarlo

El adaptador está en el paquete independiente `score-sound-vsco`. La app básica no
lo incluye: se habilita expresamente con `cargo run --features vsco` o
`cargo build -p partitura --features vsco`. El bundle local de esta prueba se compila
con esa opción. Instalar las muestras sigue siendo opcional incluso en esa compilación.

1. Abre **Sonidos** en el inspector, o pulsa el nombre del sonido junto al transporte.
2. Elige **VSCO CE · experimental**. Se cargan los instrumentos de la partitura y
   el de la audición al escribir. La primera carga puede tardar unos segundos.
3. Reproduce, pausa o cambia la posición en la regla como de costumbre.
4. Elige **GeneralUser GS** para comparar. Cambiar el paquete detiene el transporte;
   no modifica la partitura. La elección es de sesión; al abrir la app se usa GeneralUser.

En **Mezclador**, ambos bancos comparten una reverberación estéreo suave (14 % por
defecto), con cantidad, tamaño de sala y apagado para comparar en seco. Cada parte
tiene volumen (0–100 %) y posición estéreo (izquierda/centro/derecha), además de
Restablecer. Con varias partes, cada una parte de su sitio habitual en el escenario
(violines a la izquierda, violas y graves a la derecha, maderas y metales repartidos
detrás); un instrumento repetido, como un segundo violín, se coloca un 20 % más a la
derecha. Una parte sola queda centrada. Restablecer vuelve a esa posición. Los controles funcionan durante la reproducción, también para pizzicato,
las dos manos del piano y las notas de GeneralUser usadas como respaldo.
Los ajustes se conservan al cambiar de banco o reconectar audio; los de cada parte
se restablecen al abrir o crear una partitura y no se guardan en el documento.
La pausa congela también el ambiente; Detener y los saltos limpian la cola anterior.

El paquete instalado en esta máquina está en `soundpacks/vsco-ce/`; las muestras
están en `soundpacks/VSCO-2-CE-1.1.0/`. Ambos están excluidos de Git y no aumentan el
ejecutable. La app busca el manifiesto en los directorios antecesores de su ejecutable,
incluido el proyecto cuando se abre `target/Partitura.app`.

## Cobertura de esta primera integración

- Piano vertical; violín solista; secciones de viola, violonchelo y contrabajo;
  flauta, oboe, clarinete, fagot, trompeta, trompa y trombón.
- El cambio **pizzicato / arco** usa grabaciones distintas de cada instrumento de
  cuerda. Se conservan las capas de intensidad y las variaciones que contiene el SFZ.
- Los reguladores modifican la expresión de cada canal, también en notas sostenidas.
  El nivel sigue a cada paso del regulador de forma continua (unos 4 ms), sin saltos
  audibles; tras una parada o un salto se aplica de inmediato.
- **Capas de dinámica:** VSCO reparte sus grabaciones en 2–4 capas por velocidad
  (cortes en 63, 42/84 o 31/63/95), pensadas para 0–127, pero las dinámicas escritas
  van de ppp (32) a fff (124). Al cargar cada SFZ los cortes se reescalan a ese rango,
  de modo que con dos capas ppp–mp usan la grabación suave y mf–fff la fuerte (antes
  mp ya usaba la fuerte). Solo cambian los límites de las regiones: el volumen sigue
  la misma velocidad. El piano conserva sus cortes originales (61 y 111), que ya
  coinciden con p, mp–f y ff.
- La guitarra y las notas fuera del rango numérico de los parches cargados usan
  GeneralUser automáticamente. Instrumentos repetidos tienen sintetizadores separados.
- El staccato usa grabaciones **staccato** en flauta, oboe, clarinete, fagot,
  trompeta, trompa y trombón, y **spiccato** en violín, viola, violonchelo y contrabajo.
  Esto se aplica a la audición breve y a la reproducción completa. La marca escrita
  sigue siendo staccato; spiccato es la muestra corta elegida por este adaptador.
- Un staccato bajo una ligadura de expresión es portato: usa la grabación sostenida
  con la duración del 75 %, no la muestra corta.
- Pizzicato tiene prioridad sobre la muestra corta de arco. Si falta una muestra
  corta o no cubre la nota, se utiliza la sostenida con duración acortada; si tampoco
  la cubre, el contrato común permite que GeneralUser reproduzca la nota.
- Los acentos y marcato modifican la intensidad; tenuto conserva la duración completa.
  No se añaden automáticamente tremolo ni transiciones de legato.
- VSCO es una biblioteca de muestras: no añade por sí sola transiciones de legato,
  respiración musical, pedal de piano ni una interpretación humana completa.

La preparación y la carga del motor/muestras ocurren antes de abrir el nuevo flujo
de audio. Editar notas reutiliza el motor; cambiar la plantilla de instrumentos o
el instrumento de audición puede necesitar una nueva carga. Si falla la activación,
se conserva el motor anterior y se muestra el error en Sonidos.

## Instalación local reproducible

Se conserva el archivo del usuario en Downloads. El importador no descarga ni instala
nada en el sistema; recibe el archivo y una biblioteca sfizz ya compilada:

```sh
python3 scripts/prepare_vsco.py ~/Downloads/VSCO-2-CE-1.1.0.tar.gz \
  --engine target/native/sfizz-build/library/lib/libsfizz.1.2.3.dylib \
  --licenses target/native/sfizz-1.2.3
```

Requiere Python 3.12 o posterior. Comprueba los archivos de muestras referenciados por
los 27 parches usados antes de publicar `manifest.json`, y copia los avisos de licencia.
El manifiesto versión 1 contiene `kind=vsco-ce`, `samples` y `engine`, con rutas relativas.

El motor se obtuvo del [archivo oficial sfizz 1.2.3](https://github.com/sfztools/sfizz/releases/download/1.2.3/sfizz-1.2.3.tar.gz).
SHA-256: `a9339eac7620d7f0f6b44bdfe860680fab73e66efad4b5f15b21198dd9436822`.
Se compiló como biblioteca dinámica, sin JACK, renderer CLI ni plugins:

```sh
cmake -S target/native/sfizz-1.2.3 -B target/native/sfizz-build \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_OSX_ARCHITECTURES=arm64 \
  -DPROJECT_SYSTEM_PROCESSOR=aarch64 -DCMAKE_OSX_DEPLOYMENT_TARGET=11.0 \
  -DSFIZZ_JACK=OFF -DSFIZZ_RENDER=OFF -DSFIZZ_TESTS=OFF \
  -DSFIZZ_SHARED=ON -DENABLE_LTO=OFF
cmake --build target/native/sfizz-build -j 4
```

Con Apple Clang actual, el `atomic_queue.h` incluido necesita retirar cuatro
calificadores `template` redundantes: `Base::template do_pop_any(` y
`Base::template do_push_any(` pasan a `Base::do_pop_any(` y `Base::do_push_any(`.
Esta corrección de compilación no modifica el procesamiento de audio.
`PROJECT_SYSTEM_PROCESSOR=aarch64` evita los indicadores ARM de 32 bits que esta
versión antigua selecciona para el nombre `arm64`.

## Validación y comparación

```sh
cargo test --workspace
cargo test -p score-sound-vsco -- --ignored --nocapture
cargo test -p partitura --features vsco
cargo run -p score-sound-vsco --example vsco_compare
```

Los tests del paquete instalado requieren muestras locales; el resto del proyecto
se compila y prueba sin ellas ni sfizz. Las pruebas nativas incluyen audio distinto
de GM, pausa, salto en pausa, reanudación, Stop, instrumentos repetidos, pizzicato,
los once instrumentos con muestras cortas, fallback de guitarra, el mezclador y
ausencia de asignaciones Rust en el callback.
`exports/vsco-comparison/` contiene pares WAV de la misma melodía con GeneralUser y VSCO.
Para exportar más rápido que el tiempo real, el ejemplo usa el modo offline de sfizz;
el flujo de audio en vivo no lo activa.

## Licencias

- VSCO CE: CC0; se conserva su `LICENSE` y los créditos a Sam Gossner, Simon Dalzell
  y Elan Hickler/Soundemote. [Sitio oficial](https://versilian-studios.com/vsco-community/).
- sfizz: BSD-2-Clause, con sus avisos y los de sus dependencias en
  `soundpacks/vsco-ce/licenses/`. El repositorio original está archivado; para esta
  prueba se fija 1.2.3, sin afirmar mantenimiento futuro del motor.

Para desinstalar, cierra la app y retira `soundpacks/vsco-ce/`. Las partituras no
guardan dependencias de este paquete y siguen funcionando con GeneralUser.

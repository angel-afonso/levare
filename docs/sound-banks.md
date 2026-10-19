# Contrato común de bancos de sonido

La interpretación musical vive en `score-playback`: resuelve alturas, dinámicas,
articulaciones, duración sonora, técnica, ligaduras de expresión y reguladores. Sus
reglas están reunidas en `interpretation.rs`. `Audition::for_element` prepara una
escucha breve usando la dinámica vigente (incluido el nivel de un regulador), la
técnica heredada, la ligadura y la articulación de la nota. Comparte las reglas de
intensidad y duración con el plan completo; su duración base se limita a 250 ms para
escribir con fluidez. Los acordes enarmónicos no duplican ataques.

## Duración sonora

| Indicación | Duración sonora |
| --- | --- |
| Sin indicación, acento o marcato | 92 % de la escrita, con un hueco máximo de 60 ms |
| Tenuto | Completa |
| Staccato | 50 % |
| Bajo ligadura de expresión | Completa y solapada 25 ms con la nota siguiente |
| Staccato bajo ligadura (portato) | 75 % |
| Última nota de la ligadura | Según su propia indicación |

El solapamiento solo se aplica si la nota siguiente empieza justo al terminar la
anterior y no repite ninguna tecla: una nota repetida se vuelve a atacar a tiempo,
sin fundirse como una unión. Las cadenas ligadas aplican la indicación final a la
duración conjunta.

## NoteEvent

Cada ataque es un `NoteEvent`, igual en `Message::NoteOn`, `HeldNote` y la audición:

- `key`: tecla MIDI.
- `level`: intensidad interpretada (1–127): dinámica escrita o de regulador, más el
  refuerzo de acento (+16) o marcato (+24).
- `instrument` y `technique`: instrumento de la parte y técnica heredada (arco o
  pizzicato). El host no envía cambios de programa; cada banco elige el timbre a
  partir de estos campos. GeneralUser los traduce a su programa General MIDI.
- `articulation`: articulación escrita del ataque.
- `legato`: la nota está bajo una ligadura de expresión. El plan ya programa el
  solapamiento; un banco puede preferir grabaciones sostenidas o de legato.

Cada `NoteEvent` conserva su articulación y técnica por ataque. Cambiar de
articulación en otro pentagrama no modifica esa nota. Los saltos recuperan la
articulación del ataque original, y las notas partidas al pegar conservan el staccato
del pasaje ligado. Un staccato interior de una cadena ligada no interrumpe la unión.

## Interface y adapters

`score-platform::sound_engine` define dos contratos Rust:

- **SoundBank**: identidad, nombre, descripción y `prepare(score, audition, rate)`.
  Prepara un `Box<dyn SoundEngine>` o devuelve `AudioError`. Toda lectura de archivos,
  carga de bibliotecas, reserva de memoria y creación de sintetizadores ocurre aquí,
  antes de abrir el flujo de audio. Un fallo deja disponible la salida anterior.
- **SoundEngine**: controladores MIDI comunes, `note_on(channel, NoteEvent)`,
  `note_off`, `silence` y render estéreo. Recibe intensidad y duración ya interpretadas;
  decide qué grabación usar, sin reinterpretar el ritmo ni aplicar el acento dos veces.
  Un banco que no reconoce el instrumento del ataque devuelve `false`.

`GeneralMidi` es el adapter integrado. Los bancos extra tienen prioridad por ataque:
si `note_on` devuelve `false`, no debe haber modificado su sonido y el host envía
esa nota a GeneralUser. Los `note_off` pueden llegar a ambos adapters, incluidos
aquellos que rechazaron el ataque, y deben ser inocuos para claves no activas.

El render **suma** audio seco a dos planos del mismo tamaño, de un máximo de 64 frames.
Todos los métodos de audio deben ser acotados, sin bloqueos, lectura de disco, reservas
de memoria ni cambios de propietario. El sintetizador preparado tiene un único dueño
en el hilo de audio; solo se libera cuando su flujo se ha detenido. El host conserva
el transporte, la mezcla, los formatos de salida y la reverberación compartida.

Los canales son 0–31: 15 se reserva para audición y 9 para percusión. Los
controladores son 7 (volumen), 10 (panorámica) y 11 (expresión). El segundo grupo de canales permite separar las
manos del piano cuando tienen reguladores independientes.

## VSCO como extra

`score-sound-vsco` depende del contrato de `score-platform`, y contiene el manifiesto,
las rutas SFZ, el enlace sfizz y la selección de muestras. `score-platform` no depende
de ese paquete. El editor, la interfaz y el transporte solo conocen `SoundBank`.

La única inscripción concreta está en `app-gpui/src/sound_banks.rs`, protegida por
la característica opcional `vsco`. Sin esa opción, la app no compila ni incorpora
el adapter ni su enlace SFZ. Las partituras no contienen dependencias de proveedores.

Para añadir otro banco, implementa `SoundBank` y `SoundEngine` en su paquete y
regístralo en ese punto de composición. El editor no necesita nuevas condiciones.
Este contrato es interno a la compilación Rust; no es una ABI para cargar plugins
Rust arbitrarios ni un host VST/AU.

## Verificación

```sh
cargo test --workspace
cargo test -p partitura --features vsco
cargo test -p score-sound-vsco -- --ignored
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Los tests de contrato utilizan un banco independiente sin VSCO para comprobar el
ataque, la duración de audición, el salto, la pausa y el respaldo sin duplicar notas.
Los tests de audio de GeneralUser verifican diferencias reales entre dinámicas y
acentos. Los tests nativos de VSCO son opcionales y requieren las muestras instaladas.

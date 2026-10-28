# VSCO CE: optional sound pack

VSCO remains a compatible format 1 SFZ pack. Levare also supports format 2 packs and a
combination by priority; see [SFZ libraries](sfz.md).

The experimental integration reads the original SFZ and WAV files of **VSCO 2 Community
Edition 1.1.0** with **sfizz 1.2.3**. It does not convert the recordings to SoundFont or
replace GeneralUser. It is not a VST/AU plugin host: it is a local pack played by the SFZ
engine that Levare ships.

## Trying it

The adapter lives in the separate package `score-sound-vsco`. The `vsco` feature of
`levare` is on by default, so `cargo run` and `cargo build` include it. Installing the
samples remains optional: without them, the **Sonido** tab offers only GeneralUser. To
build without the adapter: `cargo run --no-default-features --features runtime-shaders`.

1. Open the **Sonido** tab of the right panel, or click the sound name next to the
   transport.
2. Choose **VSCO CE · experimental**. The score's instruments and the one used for the
   typing preview are loaded. The first load can take a few seconds.
3. Play, pause or change the position on the ruler as usual.
4. Choose **GeneralUser GS** to compare. Changing the pack stops the transport; it does not
   modify the score. The choice lasts for the session; when the app opens, GeneralUser is
   used.

In the **Sonido** tab, both banks share a soft stereo reverb (14 % by default), with
amount, room size and an off switch to compare dry. In **Mezcla**, on the left, each part
row has a volume (0–100 %) and a stereo position (left/center/right); clicking a slider's
value returns it to the usual one. With several parts, each starts from its usual place on
the stage (violins on the left, violas and low instruments on the right, woodwinds and brass
spread behind); a repeated instrument, such as a second violin, is placed 20 % further to
the right. A single part stays centered. Clicking the position value returns it to that
place. The controls work during playback, also for pizzicato, the two hands of the piano and
the GeneralUser notes used as a fallback. The settings are kept when changing bank or
reconnecting audio; those of each part are reset when a score is opened or created and are
not saved in the document. Pause also freezes the ambience; Stop and jumps clear the
previous tail.

## Where the pack lives

Every pack, VSCO included, plays through the sfizz Levare ships. The manifest's `engine`
field, which older VSCO manifests carry, is ignored.

An installed Levare looks for the pack only in its own `soundpacks` folder
(`Contents/Resources/soundpacks` inside the app bundle on macOS, `soundpacks` next to the
executable on Windows and Linux) and in the `soundpacks` folder of Levare's data folder.
Folders above the app are not searched. Debug builds also search every folder above the
executable, so `cargo run` finds the repository's `soundpacks/`, and so does a debug build
opened from `target/Levare.app`. `LEVARE_SFIZZ` overrides the sfizz library in development;
a debug build otherwise falls back to the library in `soundpacks/vsco-ce/`.

The pack installed on this machine is in `soundpacks/vsco-ce/`; the samples are in
`soundpacks/VSCO-2-CE-1.1.0/`. Both are excluded from Git and do not enlarge the
executable.

## Coverage of this first integration

- Upright piano; solo violin and violin section (according to **Sección | Solista** in
  Mezcla); viola and cello sections; solo double bass; flute, oboe, clarinet, bassoon,
  trumpet, horn and trombone.
- Three-bar tremolos on strings use VSCO's tremolo recordings (solo violin and section,
  viola, cello and double bass).
- The **pizzicato / arco** switch uses separate recordings for each string instrument. The
  intensity layers and the variations the SFZ contains are kept.
- Hairpins change each channel's expression, also on sustained notes. The level follows
  each step of the hairpin continuously (about 4 ms), with no audible jumps; after a stop or
  a jump it applies at once.
- **Dynamic layers:** VSCO splits its recordings into 2–4 velocity layers (cuts at 63,
  42/84 or 31/63/95), meant for 0–127, but written dynamics go from ppp (32) to fff (124).
  When each SFZ is loaded, the cuts are rescaled to that range, so with two layers ppp–mp
  use the soft recording and mf–fff the strong one (before, mp already used the strong one).
  Only the region limits change: the volume follows the same velocity. The piano keeps its
  original cuts (61 and 111), which already match p, mp–f and ff.
- The guitar and notes outside the numeric range of the loaded patches use GeneralUser
  automatically. Repeated instruments have separate synthesizers.
- Staccato uses **staccato** recordings on flute, oboe, clarinet, bassoon, trumpet, horn and
  trombone, and **spiccato** on violin, viola, cello and double bass. This applies to the
  short preview and to full playback. The written mark is still staccato; spiccato is the
  short sample this adapter chooses.
- A staccato under a slur is portato: it uses the sustained recording at 75 % of the
  duration, not the short sample.
- Pizzicato takes priority over the short bowed sample. If a short sample is missing or does
  not cover the note, the sustained one is used with a shortened duration; if that does not
  cover it either, the common contract lets GeneralUser play the note.
- Accents and marcato change the intensity; tenuto keeps the full duration. Tremolo and
  legato transitions are not added automatically.
- VSCO is a sample library: by itself it adds no legato transitions, musical breathing,
  piano pedal or complete human interpretation.

Preparing and loading the engine and samples happen before the new audio stream opens.
Editing notes reuses the engine; changing the instrument template or the preview
instrument may need a new load. If activation fails, the previous engine is kept and the
error is shown in Sonidos.

## Reproducible local installation

The user's archive in Downloads is kept as is. The importer downloads and installs nothing
on the system; it takes the archive and an sfizz library that is already built:

```sh
python3 scripts/prepare_vsco.py ~/Downloads/VSCO-2-CE-1.1.0.tar.gz \
  --engine target/native/sfizz-build/library/lib/libsfizz.1.2.3.dylib \
  --licenses target/native/sfizz-1.2.3
```

It requires Python 3.12 or later. It checks the sample files referenced by the 27 patches
in use before publishing `manifest.json`, and copies the license notices. The version 1
manifest contains `kind=vsco-ce`, `samples` and `engine`, with relative paths. `engine` is a
legacy field that Levare ignores; the script also copies the library into
`soundpacks/vsco-ce/`, which is where a debug build finds it when `LEVARE_SFIZZ` is not set.

The engine came from the [official sfizz 1.2.3 archive](https://github.com/sfztools/sfizz/releases/download/1.2.3/sfizz-1.2.3.tar.gz).
SHA-256: `a9339eac7620d7f0f6b44bdfe860680fab73e66efad4b5f15b21198dd9436822`.
It was built as a dynamic library, without JACK, the CLI renderer or plugins:

```sh
cmake -S target/native/sfizz-1.2.3 -B target/native/sfizz-build \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_OSX_ARCHITECTURES=arm64 \
  -DPROJECT_SYSTEM_PROCESSOR=aarch64 -DCMAKE_OSX_DEPLOYMENT_TARGET=11.0 \
  -DSFIZZ_JACK=OFF -DSFIZZ_RENDER=OFF -DSFIZZ_TESTS=OFF \
  -DSFIZZ_SHARED=ON -DENABLE_LTO=OFF
cmake --build target/native/sfizz-build -j 4
```

With current Apple Clang, the bundled `atomic_queue.h` needs four redundant `template`
qualifiers removed: `Base::template do_pop_any(` and `Base::template do_push_any(` become
`Base::do_pop_any(` and `Base::do_push_any(`. This build fix does not change audio
processing. `PROJECT_SYSTEM_PROCESSOR=aarch64` avoids the 32-bit ARM flags that this old
version selects for the name `arm64`.

## Validation and comparison

```sh
cargo test --workspace
cargo test -p score-sound-vsco -- --ignored --nocapture
cargo test -p levare
cargo run -p score-sound-vsco --example vsco_compare
```

The installed-pack tests require local samples; the rest of the project builds and tests
without them or sfizz. The native tests include audio different from GM, pause, a jump
while paused, resume, Stop, repeated instruments, pizzicato, the eleven instruments with
short samples, the guitar fallback, the mixer and the absence of Rust allocations in the
callback. `exports/vsco-comparison/` contains WAV pairs of the same melody with GeneralUser
and VSCO. To export faster than real time, the example uses sfizz's offline mode; the live
audio stream does not turn it on.

## Licenses

- VSCO CE: CC0; its `LICENSE` is kept, along with the credits to Sam Gossner, Simon Dalzell
  and Elan Hickler/Soundemote. [Official site](https://versilian-studios.com/vsco-community/).
- sfizz: BSD-2-Clause, with its notices and those of its dependencies in
  `soundpacks/vsco-ce/licenses/`. The original repository is archived; for this trial 1.2.3
  is pinned, without claiming future maintenance of the engine.

To uninstall, close the app and remove `soundpacks/vsco-ce/`. Scores do not store
dependencies on this pack and keep working with GeneralUser.

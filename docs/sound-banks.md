# Common sound bank contract

Musical interpretation lives in `score-playback`: it resolves pitches, dynamics,
articulations, sounding duration, technique, slurs and hairpins. Its rules are
gathered in `interpretation.rs`. `Audition::for_element` prepares a short preview
using the current dynamic (including the level of a hairpin), the inherited
technique, the slur and the note's articulation. It shares the intensity and
duration rules with the full plan; its base duration is capped at 250 ms so that
writing stays fluid. Enharmonic chords do not duplicate attacks.

## Sounding duration

| Marking | Sounding duration |
| --- | --- |
| No marking, accent or marcato | 92% of the written duration, with a maximum gap of 60 ms |
| Tenuto | Full |
| Staccato | 50% |
| Under a slur | Full, and overlapped 25 ms with the next note |
| Staccato under a slur (portato) | 75% |
| Last note of the slur | According to its own marking |

The overlap only applies if the next note starts exactly when the previous one ends
and does not repeat any key: a repeated note is struck again on time, without
blending like a tie. Tied chains apply the final marking to the combined duration.

## Natural interpretation

The transport uses **Natural** when the session starts; in «Sonidos» (Sounds) you can
choose **Literal (exacta)** (exact). The literal option keeps the previous plan
exactly, and the short audition always stays literal. Natural interpretation works
with any sound bank.

| Rule | Change in Natural |
| --- | --- |
| Metric accent | First beat +4; other beats +1 (third beat of 4/4 +2); subdivisions −2. In 6/8, 9/8 and 12/8 the beat groups three eighth notes. |
| Phrase contour | In phrases of three or more notes, up to +4 depending on the pitch relative to the extremes of the phrase. |
| Phrase ending | −3 on the last note, except accent or marcato. |
| Human variation | Delay of 0–5 ms and intensity change of −3 to +3; a chord and both piano hands share the delay. |

The result is deterministic for a part and a written time: playing the same score
again produces the same attacks. The final intensity is limited to 1–127. The CC11
expression messages and their reference levels stay the same in both styles; Natural
adjusts only the attacks, without changing the tempo or the written duration.
Arpeggios keep their internal spacing.

## NoteEvent

Each attack is a `NoteEvent`, the same in `Message::NoteOn`, `HeldNote` and the
audition:

- `key`: MIDI key.
- `level`: interpreted intensity (1–127): written or hairpin dynamic, plus the
  accent (+16) or marcato (+24) boost.
- `instrument` and `technique`: the part's instrument and the inherited technique
  (arco or pizzicato). The host does not send program changes; each bank chooses the
  timbre from these fields. GeneralUser translates them to its General MIDI program.
- `articulation`: the attack's written articulation.
- `legato`: the note is under a slur. The plan already schedules the overlap; a bank
  may prefer sustained or legato recordings.

Each `NoteEvent` keeps its articulation and technique per attack. Changing the
articulation on another staff does not modify that note. Jumps recover the
articulation of the original attack, and notes split when pasting keep the staccato
of the slurred passage. An inner staccato in a tied chain does not break the tie.

## Interface and adapters

`score-platform::sound_engine` defines two Rust contracts:

- **SoundBank**: identity, name, description and `prepare(score, audition, rate)`.
  It prepares a `Box<dyn SoundEngine>` or returns `AudioError`. All file reading,
  library loading, memory allocation and synthesizer creation happens here, before
  the audio stream is opened. A failure leaves the previous output available.
- **SoundEngine**: common MIDI controllers, `note_on(channel, NoteEvent)`,
  `note_off`, `silence` and stereo render. It receives intensity and duration that
  are already interpreted; it decides which recording to use, without reinterpreting
  the rhythm or applying the accent twice. A bank that does not recognize the
  attack's instrument returns `false`.

`GeneralMidi` is the built-in adapter. Extra banks take priority per attack: if
`note_on` returns `false`, it must not have modified its sound, and the host sends
that note to GeneralUser. `note_off` messages may reach both adapters, including
those that rejected the attack, and must be harmless for inactive keys.

Render **adds** dry audio into two planes of the same size, of at most 64 frames.
All audio methods must be bounded, with no locks, disk reads, memory allocations or
ownership changes. The prepared synthesizer has a single owner on the audio thread;
it is only released once its stream has stopped. The host keeps the transport, the
mix, the output formats and the shared reverb.

Channels are 0–31: 15 is reserved for audition and 9 for percussion. The controllers
are 7 (volume), 10 (pan) and 11 (expression). The second group of channels makes it
possible to separate the piano hands when they have independent hairpins.

## SFZ libraries as an extra

`score-sound-vsco` depends on the `score-platform` contract, and holds the
manifests, the SFZ paths, the sfizz binding and the sample selection. It can load
VSCO CE and other format 2 packages, including Virtual Playing Orchestra. When there
are several, the combined bank queries each package according to the priority saved
in `sonidos.json`, before the GeneralUser fallback. In `cc1` packages, the level is
sent to each voice before its attack; CC11 keeps the continuous expression control.
See the [SFZ libraries guide](sfz.md). `score-platform` does not depend on that
package. The editor, the interface and the transport only know `SoundBank`.

The only concrete registration is in `app-gpui/src/sound_banks.rs`, guarded by the
`vsco` feature, enabled by default. Without that option
(`--no-default-features --features runtime-shaders`), the app neither compiles nor
includes the adapter or its SFZ binding. Since sfizz is loaded at runtime, including
the adapter does not require any native library to build. Scores contain no vendor
dependencies.

To add another bank, implement `SoundBank` and `SoundEngine` in its package and
register it at that composition point. The editor needs no new conditions. This
contract is internal to the Rust build; it is not an ABI for loading arbitrary Rust
plugins nor a VST/AU host.

## Verification

```sh
cargo test --workspace
cargo test -p levare --no-default-features --features runtime-shaders
cargo test -p score-sound-vsco -- --ignored
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The contract tests use a standalone bank without VSCO to check the attack, the
audition duration, the jump, the rest and the fallback without duplicating notes.
The GeneralUser audio tests verify real differences between dynamics and accents.
The native VSCO tests are optional and require the samples to be installed.

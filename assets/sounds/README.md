# GeneralUser GS

**GeneralUser GS 2.0.3** SoundFont bank, by S. Christian Collins, unmodified.
Levare initially uses the General MIDI acoustic piano (program 0).

- Official repository: <https://github.com/mrbumpy409/GeneralUser-GS>.
- Pinned revision: `684543d5e5efaef08d02be50dcda8d552478fa60`.
- Original file: `GeneralUser-GS.sf2` (32,319,396 bytes).
- SHA-256: `9575028c7a1f589f5770fccc8cff2734566af40cd26ed836944e9a5152688cfe`.
- Original license: [LICENSE.txt](LICENSE.txt), copied from `documentation/LICENSE.txt`
  at that same revision. It allows its use in software projects and music creation;
  it also keeps the author's information about the origin of the samples.
  The sounds keep this own license, independent of the code's MIT license.

The bank is included in the executable. Nothing is downloaded when the application
starts. `rustysynth` reads it once and shares it among the session's synthesizers.

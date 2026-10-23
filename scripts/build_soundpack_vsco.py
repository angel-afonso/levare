#!/usr/bin/env python3
"""Build the «VSCO CE · Alta calidad» pack for Sonidos › Más sonidos.

Takes the VSCO 2 Community Edition folder, keeps only the SFZ patches Levare
plays, converts their samples to lossless FLAC and writes a deterministic zip
with its SHA-256, ready for GitHub Releases and the built-in catalog.
Requires ffmpeg.
"""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import zipfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PACK_ID = "vsco-alta-calidad"
VERSION = 1


def patch(sustain, pizzicato=None, staccato=None, tremolo=None, layers=True):
    spec = {"sustain": sustain, "dynamics": "velocity", "spread_layers": layers}
    for key, value in (("pizzicato", pizzicato), ("staccato", staccato), ("tremolo", tremolo)):
        if value:
            spec[key] = value
    return spec


# The same patches as the built-in VSCO mapping in crates/sound-vsco.
INSTRUMENTS = {
    "Piano": patch("UprightPiano.sfz", layers=False),
    "Violin": {
        **patch("SViolinVib.sfz", "SViolinPizz.sfz", "SViolinSpic.sfz", "SViolinTrem.sfz"),
        "section": patch("ViolinEnsSusVib.sfz", "ViolinEnsPizz.sfz", "ViolinEnsSpic.sfz", "ViolinEnsTrem.sfz"),
    },
    "Viola": patch("ViolaEnsSusVib.sfz", "ViolaEnsPizz.sfz", "ViolaEnsSpic.sfz", "ViolaEnsTrem.sfz"),
    "Cello": patch("CelloEnsSusVib.sfz", "CelloEnsPizz.sfz", "CelloEnsSpic.sfz", "CelloEnsTrem.sfz"),
    "DoubleBass": patch("ContrabassSusVB.sfz", "ContrabassPizz.sfz", "ContrabassSpic.sfz", "ContrabassTrem.sfz"),
    "Flute": patch("FluteSusVib.sfz", staccato="FluteStac.sfz"),
    "Oboe": patch("OboeSusVib.sfz", staccato="OboeStac.sfz"),
    "Clarinet": patch("ClarinetSus.sfz", staccato="ClarinetStac.sfz"),
    "Bassoon": patch("BassoonSus.sfz", staccato="BassoonStac.sfz"),
    "Trumpet": patch("TrumpetSus.sfz", staccato="TrumpetStac.sfz"),
    "Horn": patch("FHornSus.sfz", staccato="FHornStac.sfz"),
    "Trombone": patch("TromboneSus.sfz", staccato="TromboneStac.sfz"),
}


def sfz_files():
    files = set()
    for spec in INSTRUMENTS.values():
        for variant in (spec, spec.get("section")):
            for key in ("sustain", "pizzicato", "staccato", "tremolo"):
                if variant and key in variant:
                    files.add(variant[key])
    return sorted(files)


def samples_of(sfz_path):
    """Sample paths of an SFZ, relative to the library root."""
    base = ""
    for line in sfz_path.read_text(encoding="utf-8", errors="replace").splitlines():
        found = re.search(r"default_path=(.+?)\s*$", line)
        if found:
            base = found.group(1).replace("\\", "/")
        for sample in re.findall(r"sample=(.+?\.wav)", line, re.IGNORECASE):
            yield os.path.normpath(os.path.join(base, sample.replace("\\", "/")))


def convert(source, target):
    if target.is_file():
        return
    target.parent.mkdir(parents=True, exist_ok=True)
    partial = target.with_suffix(".partial.flac")
    subprocess.run(
        ["ffmpeg", "-nostdin", "-loglevel", "error", "-y", "-i", str(source),
         "-c:a", "flac", "-compression_level", "8", str(partial)],
        check=True,
    )
    partial.replace(target)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("vsco", type=Path, help="carpeta VSCO-2-CE-1.1.0")
    parser.add_argument("--work", type=Path, default=ROOT / "target" / "soundpacks" / PACK_ID)
    args = parser.parse_args()
    if shutil.which("ffmpeg") is None:
        sys.exit("Hace falta ffmpeg")
    pack = args.work
    samples = pack / "samples"
    samples.mkdir(parents=True, exist_ok=True)

    wavs = set()
    for name in sfz_files():
        text = (args.vsco / name).read_text(encoding="utf-8", errors="replace")
        (samples / name).write_text(
            re.sub(r"(sample=.+?)\.wav", r"\1.flac", text, flags=re.IGNORECASE), encoding="utf-8")
        wavs.update(samples_of(args.vsco / name))
    with ThreadPoolExecutor(max_workers=os.cpu_count() or 4) as pool:
        list(pool.map(
            lambda wav: convert(args.vsco / wav, samples / Path(wav).with_suffix(".flac")),
            sorted(wavs),
        ))
    shutil.copy2(args.vsco / "LICENSE", pack / "LICENSE")
    manifest = {
        "format": 2, "kind": "sfz", "id": PACK_ID, "name": "VSCO CE · Alta calidad",
        "samples": "samples", "license": "LICENSE", "instruments": INSTRUMENTS,
    }
    (pack / "manifest.json").write_text(
        json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")

    archive = pack.parent / f"{PACK_ID}-{VERSION}.zip"
    files = sorted(path for path in pack.rglob("*") if path.is_file() and ".partial." not in path.name)
    installed = 0
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_STORED) as output:
        for path in files:
            # Fixed dates keep the zip, and so its checksum, reproducible.
            info = zipfile.ZipInfo(path.relative_to(pack).as_posix(), date_time=(1980, 1, 1, 0, 0, 0))
            info.external_attr = 0o644 << 16
            output.writestr(info, path.read_bytes())
            installed += path.stat().st_size
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    print(json.dumps({
        "archive": str(archive), "size": archive.stat().st_size,
        "sha256": digest, "installed_size": installed, "samples": len(wavs),
    }, indent=2))


if __name__ == "__main__":
    main()

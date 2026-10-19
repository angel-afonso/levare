#!/usr/bin/env python3
"""Install the user's VSCO archive and an already-built sfizz engine locally.

No system installation or downloads. The resulting soundpacks directory is ignored
by Git and can be removed to uninstall the optional library.
"""
import argparse
import json
from pathlib import Path
import re
import shutil
import tarfile

PATCHES = [
    "UprightPiano", "SViolinVib", "SViolinPizz", "ViolaEnsSusVib", "ViolaEnsPizz",
    "CelloEnsSusVib", "CelloEnsPizz", "ContrabassSusVB", "ContrabassPizz",
    "FluteSusVib", "OboeSusVib", "ClarinetSus", "BassoonSus", "TrumpetSus",
    "FHornSus", "TromboneSus",
    "SViolinSpic", "ViolaEnsSpic", "CelloEnsSpic", "ContrabassSpic",
    "FluteStac", "OboeStac", "ClarinetStac", "BassoonStac",
    "TrumpetStac", "FHornStac", "TromboneStac",
]


def audit(root):
    """All SFZ sample paths used by this integration must actually exist."""
    checked = 0
    for patch in PATCHES:
        text = (root / (patch + ".sfz")).read_text()
        default = re.search(r"(?m)^default_path=(.*)$", text)
        base = root / (default.group(1).strip().replace("\\", "/") if default else "")
        samples = re.findall(r"(?m)^sample=(.*)$", text)
        if not samples:
            raise ValueError(f"No samples in {patch}")
        for sample in samples:
            path = base / sample.strip().replace("\\", "/")
            if not path.is_file():
                raise FileNotFoundError(path)
            checked += 1
    return checked


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--engine", type=Path, required=True, help="Built libsfizz dynamic library")
    parser.add_argument("--licenses", type=Path, help="sfizz source directory, to copy dependency notices")
    parser.add_argument("--destination", type=Path, default=Path(__file__).resolve().parents[1] / "soundpacks")
    args = parser.parse_args()
    if not args.archive.is_file() or not args.engine.is_file():
        parser.error("The archive and sfizz library must exist")
    destination = args.destination.resolve()
    root = destination / "VSCO-2-CE-1.1.0"
    destination.mkdir(parents=True, exist_ok=True)
    if not root.exists():
        with tarfile.open(args.archive, "r:gz") as archive:
            for member in archive.getmembers():
                parts = Path(member.name).parts
                if not parts or parts[0] != root.name or ".." in parts or not (member.isfile() or member.isdir()):
                    raise ValueError(f"Unexpected archive member: {member.name}")
            archive.extractall(destination, filter="data")
    count = audit(root)
    pack = destination / "vsco-ce"
    pack.mkdir(exist_ok=True)
    extension = args.engine.suffix
    engine = pack / ("libsfizz" + extension)
    if args.engine.resolve() != engine.resolve():
        shutil.copy2(args.engine, engine)
    licenses = pack / "licenses"
    licenses.mkdir(exist_ok=True)
    shutil.copy2(root / "LICENSE", licenses / "VSCO-CC0.txt")
    if args.licenses:
        for path in args.licenses.rglob("*"):
            if path.is_file() and path.name.upper().startswith(("LICENSE", "COPYING", "NOTICE", "AUTHORS")):
                relative = path.relative_to(args.licenses)
                target = licenses / "sfizz" / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(path, target)
    manifest = {"format": 1, "kind": "vsco-ce", "samples": "../" + root.name, "engine": engine.name}
    (pack / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"VSCO CE ready: {count} sample references checked.\n{pack / 'manifest.json'}")


if __name__ == "__main__":
    main()

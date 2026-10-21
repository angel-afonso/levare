#!/usr/bin/env python3
"""Build the macOS app with Levare's icon, including Retina representations."""

import argparse
import json
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parent.parent
BRANDING = ROOT / "assets" / "branding" / "levare"


def prepare_icon():
    # ICNS is a big-endian container of PNG representations. Package them
    # directly so generation also works without iconutil's GUI services.
    representations = [
        (b"icp4", 16), (b"icp5", 32), (b"icp6", 64),
        (b"ic07", 128), (b"ic08", 256), (b"ic09", 512), (b"ic10", 1024),
        (b"ic11", 32), (b"ic12", 64), (b"ic13", 256), (b"ic14", 512),
    ]
    chunks = []
    with tempfile.TemporaryDirectory(prefix="levare-icon-") as temp:
        for tag, pixels in representations:
            png = Path(temp) / f"{tag.decode()}.png"
            subprocess.run(
                ["sips", "-z", str(pixels), str(pixels),
                 str(BRANDING / "icon-macos-v2.png"), "--out", str(png)],
                check=True, stdout=subprocess.DEVNULL,
            )
            data = png.read_bytes()
            chunks.append(tag + struct.pack(">I", len(data) + 8) + data)
    payload = b"".join(chunks)
    icon = BRANDING / "Levare.icns"
    icon.write_bytes(b"icns" + struct.pack(">I", len(payload) + 8) + payload)
    return icon


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--release", action="store_true")
    args = parser.parse_args()
    if sys.platform != "darwin":
        parser.error("This bundle requires macOS and sips.")

    icon = prepare_icon()
    command = ["cargo", "build", "--locked", "-p", "levare",
               "--message-format=json-render-diagnostics"]
    if args.release:
        command.append("--release")
    build = subprocess.run(command, cwd=ROOT, check=True, capture_output=False,
                           stdout=subprocess.PIPE, text=True)
    binary = None
    for line in build.stdout.splitlines():
        message = json.loads(line)
        if (message.get("reason") == "compiler-artifact"
                and message.get("target", {}).get("name") == "levare"
                and "bin" in message["target"]["kind"]
                and message.get("executable")):
            binary = Path(message["executable"])
    if binary is None:
        raise RuntimeError("Cargo did not report the levare executable.")
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=ROOT,
    ))
    target = Path(metadata["target_directory"])
    bundle = target / "Levare.app"
    contents = bundle / "Contents"
    executable = contents / "MacOS" / "levare"
    resources = contents / "Resources"
    executable.parent.mkdir(parents=True, exist_ok=True)
    resources.mkdir(parents=True, exist_ok=True)
    staging = executable.with_suffix(".new")
    shutil.copy2(binary, staging)
    staging.replace(executable)
    shutil.copy2(ROOT / "packaging" / "macos" / "Info.plist", contents / "Info.plist")
    shutil.copy2(icon, resources / icon.name)
    subprocess.run(["plutil", "-lint", str(contents / "Info.plist")], check=True)
    print(bundle)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Package a previously tested native binary for the current operating system."""

import argparse
import hashlib
from pathlib import Path
import plistlib
import shutil
import subprocess
import tarfile
import json

ROOT = Path(__file__).resolve().parents[1]


def package(platform: str, architecture: str, output: Path) -> Path:
    metadata = json.loads(subprocess.check_output([
        "cargo", "metadata", "--no-deps", "--format-version", "1",
        "--manifest-path", str(ROOT / "native/Cargo.toml"),
    ], text=True))
    version = metadata["packages"][0]["version"]
    output.mkdir(parents=True, exist_ok=True)
    target = ROOT / "native/target/release"
    if platform == "windows":
        artifact = output / f"RPP-{version}-windows-{architecture}.exe"
        shutil.copy2(target / "rpp.exe", artifact)
        return artifact

    if platform == "macos":
        app = output / f"RPP-{version}-macos-{architecture}" / "RPP.app"
        contents = app / "Contents"
        executable = contents / "MacOS/rpp"
        executable.parent.mkdir(parents=True, exist_ok=True)
        (contents / "Resources").mkdir(exist_ok=True)
        shutil.copy2(target / "rpp", executable)
        executable.chmod(0o755)
        info = {
            "CFBundleName": "RPP",
            "CFBundleDisplayName": "RPP — Ran Plan Plan",
            "CFBundleIdentifier": "io.github.bagyette.rpp",
            "CFBundleExecutable": "rpp",
            "CFBundlePackageType": "APPL",
            "CFBundleShortVersionString": version,
            "CFBundleVersion": version,
            "LSMinimumSystemVersion": "11.0",
            "NSHighResolutionCapable": True,
            "NSPrincipalClass": "NSApplication",
        }
        with (contents / "Info.plist").open("wb") as plist:
            plistlib.dump(info, plist)
        # An ad-hoc signature enables local execution; this is not Apple notarization.
        subprocess.run(["codesign", "--force", "--deep", "--sign", "-", str(app)], check=True)
        subprocess.run(["codesign", "--verify", "--deep", "--strict", str(app)], check=True)
        artifact = output / f"RPP-{version}-macos-{architecture}.zip"
        subprocess.run(["ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", str(app), str(artifact)], check=True)
        return artifact

    directory = output / f"RPP-{version}-linux-{architecture}"
    directory.mkdir(exist_ok=True)
    shutil.copy2(target / "rpp", directory / "rpp")
    (directory / "rpp").chmod(0o755)
    shutil.copy2(ROOT / "README.md", directory / "README.md")
    shutil.copy2(ROOT / "LICENSE", directory / "LICENSE")
    artifact = output / f"{directory.name}.tar.gz"
    with tarfile.open(artifact, "w:gz") as archive:
        archive.add(directory, arcname=directory.name)
    return artifact


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", choices=["windows", "linux", "macos"], required=True)
    parser.add_argument("--architecture", choices=["x64", "arm64"], required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "dist")
    args = parser.parse_args()
    artifact = package(args.platform, args.architecture, args.output.resolve())
    digest = hashlib.sha256(artifact.read_bytes()).hexdigest()
    print(f"{digest}  {artifact.name}")


if __name__ == "__main__":
    main()

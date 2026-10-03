#!/usr/bin/env python3
"""Build and validate local RustCraft release archives; never publishes them."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import zipfile


ROOT = Path(__file__).resolve().parent.parent
DIST = ROOT / "target" / "release-dist"
VERSION_RE = re.compile(
    r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)"
    r"(?:-([0-9A-Za-z.-]+))?(?:\+([0-9A-Za-z.-]+))?$"
)
PROHIBITED_ARCHIVE_PARTS = {
    ".git",
    ".cache",
    ".code-graph",
    ".content-cache",
    ".resource-cache",
    "atlas-debug",
    "benchmark-worlds",
    "content-cache",
    "local-worlds",
    "reference",
    "release-dist",
    "render-tests",
    "resource-cache",
    "saves",
    "target",
    "test-worlds",
}
PROHIBITED_ASSET_NAMES = {"minecraft.class", "mojang_c.dsa", "mojang_c.sf", "terrain.png"}


def run(args: list[str], *, capture: bool = False, input: str | None = None) -> str:
    result = subprocess.run(
        args,
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE if capture else None,
        input=input,
    )
    return result.stdout.strip() if capture else ""


def product_version() -> str:
    with (ROOT / "Cargo.toml").open("rb") as source:
        value = tomllib.load(source)["workspace"]["package"]["version"]
    if not VERSION_RE.fullmatch(value):
        raise RuntimeError(f"workspace version is not valid SemVer: {value!r}")
    return value


def host_target() -> str:
    output = run(["rustc", "-vV"], capture=True)
    for line in output.splitlines():
        if line.startswith("host: "):
            return line.partition(": ")[2]
    raise RuntimeError("rustc -vV did not report a host target triple")


def version() -> None:
    print(product_version())


def verify_tag(tag: str) -> None:
    expected = f"v{product_version()}"
    if tag != expected:
        raise RuntimeError(f"tag/version mismatch: received {tag!r}, expected {expected!r}")
    print(f"tag {tag} matches workspace SemVer {product_version()}")


def package_license_files(metadata: dict, stage: Path) -> tuple[list[str], list[str]]:
    workspace = set(metadata["workspace_members"])
    third_party = sorted(
        (package for package in metadata["packages"] if package["id"] not in workspace),
        key=lambda package: (package["name"], package["version"]),
    )
    missing_expressions = []
    copied_files = []
    license_root = stage / "THIRD_PARTY_LICENSES"
    for package in third_party:
        if not package.get("license"):
            missing_expressions.append(f"{package['name']}@{package['version']}")
        manifest = Path(package["manifest_path"])
        source = manifest.parent
        license_dir = license_root / f"{package['name']}-{package['version']}"
        for candidate in sorted(source.iterdir()):
            upper = candidate.name.upper()
            if not candidate.is_file() or candidate.is_symlink():
                continue
            if not (
                upper == "LICENSE"
                or upper.startswith("LICENSE-")
                or upper.startswith("LICENSE.")
                or upper.startswith("COPYING")
                or upper.startswith("NOTICE")
            ):
                continue
            license_dir.mkdir(parents=True, exist_ok=True)
            target = license_dir / candidate.name
            shutil.copyfile(candidate, target)
            copied_files.append(target.relative_to(stage).as_posix())

    lines = [
        "# Cargo dependency license inventory",
        "",
        "Generated from `cargo metadata --locked`. This is a package inventory, not legal advice.",
        "Upstream license and notice files found in each crate source are included under",
        "`THIRD_PARTY_LICENSES/<crate>-<version>/`.",
        "",
    ]
    for package in third_party:
        expression = package.get("license") or "MISSING LICENSE EXPRESSION"
        lines.append(f"- `{package['name']} {package['version']}` — `{expression}`")
    (stage / "THIRD_PARTY_NOTICES.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    return missing_expressions, copied_files


def build(target: str | None = None) -> Path:
    publication_check()
    target = target or host_target()
    version = product_version()
    print(f"Building RustCraft {version} for {target} (release profile)")
    run(
        [
            "cargo",
            "build",
            "--locked",
            "--release",
            "--target",
            target,
            "-p",
            "rustcraft-client",
            "-p",
            "rustcraft-server",
        ]
    )
    suffix = ".exe" if "windows" in target else ""
    binaries = ["rustcraft-client", "rustcraft-server"]
    binary_root = ROOT / "target" / target / "release"
    stage = DIST / target / f"rustcraft-{version}-{target}"
    if not stage.resolve().is_relative_to(DIST.resolve()):
        raise RuntimeError("refusing to stage outside target/release-dist")
    if stage.exists():
        shutil.rmtree(stage)
    stage.mkdir(parents=True)
    for binary in binaries:
        source = binary_root / f"{binary}{suffix}"
        if not source.is_file():
            raise RuntimeError(f"expected release executable missing: {source}")
        shutil.copy2(source, stage / f"{binary}{suffix}")
        os.chmod(stage / f"{binary}{suffix}", 0o755)

    for relative in ("scripts/dev/inspect_player.rhai", "scripts/commands/where.rhai", "scripts/scenarios/dx_smoke.rhai", "scripts/scenarios/dx_console.rhai", "scripts/scenarios/dx_responsive.rhai"):
        destination = stage / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / relative, destination)
    shutil.copyfile(ROOT / "docs" / "SCRIPTING.md", stage / "SCRIPTING.md")
    shutil.copyfile(ROOT / "README.md", stage / "README.md")
    shutil.copyfile(ROOT / "docs" / "RELEASE.md", stage / "RELEASE.md")
    shutil.copyfile(ROOT / "docs" / "RELEASE-RUNNING.md", stage / "RUNNING.md")
    for license_name in ("LICENSE", "LICENSE-MIT", "LICENSE-APACHE"):
        license_path = ROOT / license_name
        if not license_path.is_file():
            raise RuntimeError(f"required project license file is missing: {license_path}")
        shutil.copyfile(license_path, stage / license_name)
    metadata = json.loads(run(["cargo", "metadata", "--locked", "--format-version", "1"], capture=True))
    missing, copied = package_license_files(metadata, stage)
    (stage / "BUILD-INFO.txt").write_text(
        f"RustCraft {version}\nTarget: {target}\nProfile: release\n"
        f"Source commit: {run(['git', 'rev-parse', '--short=7', 'HEAD'], capture=True) if shutil.which('git') else 'unknown'}\n",
        encoding="utf-8",
    )
    if missing:
        raise RuntimeError("third-party packages without license expressions: " + ", ".join(missing))

    archive_base = DIST / f"rustcraft-{version}-{target}"
    archive_base.parent.mkdir(parents=True, exist_ok=True)
    if "windows" in target:
        archive = Path(f"{archive_base}.zip")
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as output:
            for file in sorted(path for path in stage.rglob("*") if path.is_file()):
                info = zipfile.ZipInfo(
                    f"{stage.name}/{file.relative_to(stage).as_posix()}",
                    date_time=(1980, 1, 1, 0, 0, 0),
                )
                info.compress_type = zipfile.ZIP_DEFLATED
                info.external_attr = 0o100755 << 16 if os.access(file, os.X_OK) else 0o100644 << 16
                output.writestr(info, file.read_bytes(), compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)
    else:
        archive = Path(f"{archive_base}.tar.gz")
        with archive.open("wb") as raw:
            with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
                with tarfile.open(fileobj=compressed, mode="w") as output:
                    for file in sorted(path for path in stage.rglob("*") if path.is_file()):
                        relative = PurePosixPath(stage.name, file.relative_to(stage).as_posix())
                        info = output.gettarinfo(str(file), arcname=str(relative))
                        info.uid = info.gid = 0
                        info.uname = info.gname = ""
                        info.mtime = 0
                        with file.open("rb") as content:
                            output.addfile(info, content)

    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    checksum = archive.with_suffix(archive.suffix + ".sha256")
    checksum.write_text(f"{digest}  {archive.name}\n", encoding="utf-8")
    print(f"Archive: {archive.relative_to(ROOT)}")
    print(f"SHA-256: {checksum.relative_to(ROOT)} ({digest})")
    print(f"Included dependency license/notice files: {len(copied)}")
    return archive


def safe_archive_names(archive: Path) -> list[str]:
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as source:
            names = source.namelist()
    else:
        with tarfile.open(archive, "r:gz") as source:
            names = source.getnames()
    for name in names:
        path = PurePosixPath(name)
        if path.is_absolute() or ".." in path.parts:
            raise RuntimeError(f"unsafe archive path: {name}")
        if any(part.casefold() in PROHIBITED_ARCHIVE_PARTS for part in path.parts):
            raise RuntimeError(f"prohibited local path in archive: {name}")
        if path.name.casefold() in PROHIBITED_ASSET_NAMES:
            raise RuntimeError(f"prohibited original game asset in archive: {name}")
    return names


def check(target: str | None = None) -> None:
    target = target or host_target()
    version = product_version()
    archive_base = DIST / f"rustcraft-{version}-{target}"
    archive = Path(f"{archive_base}{'.zip' if 'windows' in target else '.tar.gz'}")
    checksum = archive.with_suffix(archive.suffix + ".sha256")
    if not archive.is_file() or not checksum.is_file():
        raise RuntimeError("release archive/checksum missing; run `just release-build` first")
    expected = checksum.read_text(encoding="utf-8").split()[0]
    actual = hashlib.sha256(archive.read_bytes()).hexdigest()
    if expected != actual:
        raise RuntimeError(f"checksum mismatch for {archive.name}")
    names = safe_archive_names(archive)
    expected_suffix = ".exe" if "windows" in target else ""
    prefix = f"rustcraft-{version}-{target}/"
    required = {
        f"{prefix}rustcraft-client{expected_suffix}",
        f"{prefix}rustcraft-server{expected_suffix}",
        f"{prefix}README.md",
        f"{prefix}RELEASE.md",
        f"{prefix}RUNNING.md",
        f"{prefix}THIRD_PARTY_NOTICES.md",
        f"{prefix}BUILD-INFO.txt",
        f"{prefix}LICENSE",
        f"{prefix}LICENSE-MIT",
        f"{prefix}LICENSE-APACHE",
    }
    if not required.issubset(set(names)):
        raise RuntimeError("archive is missing required files: " + ", ".join(sorted(required - set(names))))
    if not any(name.startswith(f"{prefix}THIRD_PARTY_LICENSES/") for name in names):
        raise RuntimeError("archive contains no copied third-party license/notice texts")
    with tempfile.TemporaryDirectory(prefix="rustcraft-release-check-", dir=ROOT / "target") as temp:
        unpacked = Path(temp)
        if archive.suffix == ".zip":
            with zipfile.ZipFile(archive) as source:
                source.extractall(unpacked)
        else:
            with tarfile.open(archive, "r:gz") as source:
                source.extractall(unpacked, filter="data")
        package = unpacked / f"rustcraft-{version}-{target}"
        client = package / f"rustcraft-client{expected_suffix}"
        server = package / f"rustcraft-server{expected_suffix}"
        client_version = subprocess.check_output([str(client), "--version"], text=True).strip()
        server_version = subprocess.check_output([str(server), "--version"], text=True).strip()
        if f"RustCraft {version} (" not in client_version or client_version != server_version:
            raise RuntimeError(f"unexpected binary identities: {client_version!r}, {server_version!r}")
        subprocess.run([str(server), "--smoke"], cwd=unpacked, check=True)
    print(f"Archive structure, extraction, executable identities and headless smoke passed: {archive}")


def publication_check() -> None:
    blockers = []
    if not all((ROOT / name).is_file() for name in ("LICENSE", "LICENSE-MIT", "LICENSE-APACHE")):
        blockers.append("the MIT OR Apache-2.0 project license files are incomplete")
    try:
        metadata = json.loads(run(["cargo", "metadata", "--locked", "--format-version", "1"], capture=True))
        packages = {package["id"]: package for package in metadata["packages"]}
        wrong_license = [
            packages[package_id]["name"]
            for package_id in metadata["workspace_members"]
            if packages[package_id].get("license") != "MIT OR Apache-2.0"
        ]
        if wrong_license:
            blockers.append("workspace Cargo package license metadata differs from MIT OR Apache-2.0: "
                            + ", ".join(sorted(wrong_license)))
    except (KeyError, json.JSONDecodeError, subprocess.CalledProcessError) as error:
        blockers.append(f"could not verify Cargo workspace licensing metadata: {error}")
    if not shutil.which("git"):
        blockers.append("Git is unavailable, so reachable asset history cannot be verified")
    else:
        files = run(["git", "ls-files", "reference/assets"], capture=True).splitlines()
        current_assets = [path for path in files if path != "reference/assets/README.md"]
        if current_assets:
            blockers.append(f"{len(current_assets)} original/reference asset files remain tracked")
        path_history = run(
            ["git", "log", "--all", "--format=", "--name-only", "--", "reference/assets"],
            capture=True,
        ).splitlines()
        path_history = [path for path in path_history if path and path != "reference/assets/README.md"]
        if path_history:
            blockers.append(f"Git path history still includes {len(path_history)} reference/assets entries")
        objects = run(["git", "rev-list", "--objects", "--all"], capture=True).splitlines()
        historical_objects = [
            (line.split(" ", 1)[0], line.split(" ", 1)[1])
            for line in objects
            if " " in line
            and line.split(" ", 1)[1].startswith("reference/assets/")
            and line.split(" ", 1)[1] != "reference/assets/README.md"
        ]
        object_types = run(
            ["git", "cat-file", "--batch-check=%(objecttype)"],
            input="".join(f"{oid}\n" for oid, _ in historical_objects),
            capture=True,
        ).splitlines()
        historical = [
            path for (_, path), kind in zip(historical_objects, object_types) if kind == "blob"
        ]
        if historical:
            blockers.append(f"{len(historical)} original/reference asset blobs remain in Git history")
    if blockers:
        print("Formal publication is blocked:")
        for blocker in blockers:
            print(f"- {blocker}")
        raise SystemExit(2)
    print("Publication prerequisites passed")


def main() -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("version")
    build_parser = subparsers.add_parser("build")
    build_parser.add_argument("--target")
    check_parser = subparsers.add_parser("check")
    check_parser.add_argument("--target")
    tag_parser = subparsers.add_parser("verify-tag")
    tag_parser.add_argument("tag")
    subparsers.add_parser("publication-check")
    args = parser.parse_args()
    try:
        if args.command == "version":
            version()
        elif args.command == "build":
            build(args.target)
        elif args.command == "check":
            check(args.target)
        elif args.command == "verify-tag":
            verify_tag(args.tag)
        else:
            publication_check()
    except (OSError, RuntimeError, subprocess.CalledProcessError, json.JSONDecodeError) as error:
        print(f"release tooling error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

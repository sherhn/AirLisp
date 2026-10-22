#!/usr/bin/env python3
"""Build a self-contained Air distribution that works right after unpacking:
the app, the CLI, a bundled SBCL (REPL, compiler, ASDF) and all licence notices.

Usage:
  python tools/package.py [--sbcl-dir DIR] [--no-zip] [--skip-build]

--sbcl-dir  folder with sbcl[.exe], sbcl.core and contrib/ (default: auto-detected installation).
Result: target/dist/Air-<version>-<os>-<arch>[.zip]
"""
import argparse
import json
import os
import pathlib
import platform
import re
import shutil
import subprocess
import sys
import zipfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
EXE = ".exe" if os.name == "nt" else ""


def version():
    m = re.search(r'^version = "([^"]+)"', (ROOT / "Cargo.toml").read_text(encoding="utf-8"), re.M)
    return m.group(1) if m else "0.0.0"


def find_sbcl_dir(explicit):
    cands = []
    if explicit:
        cands.append(pathlib.Path(explicit))
    exe = shutil.which("sbcl")
    if exe:
        cands.append(pathlib.Path(exe).resolve().parent)
    cands += [pathlib.Path(p) for p in (r"C:\Program Files\Steel Bank Common Lisp", "/usr/local/lib/sbcl", "/usr/lib/sbcl")]
    for c in cands:
        if (c / f"sbcl{EXE}").is_file() and (c / "sbcl.core").is_file() and (c / "contrib").is_dir():
            return c
    sys.exit("No relocatable SBCL found (need sbcl, sbcl.core and contrib/ in one folder). Pass --sbcl-dir.")


def crate_licenses():
    out = subprocess.run(["cargo", "metadata", "--format-version", "1", "--locked"], cwd=ROOT, capture_output=True, text=True, check=True).stdout
    meta = json.loads(out)
    members = set(meta["workspace_members"])
    rows = sorted({(p["name"], p["version"], p.get("license") or "see crate", p.get("repository") or "") for p in meta["packages"] if p["id"] not in members})
    return rows


def notices(sbcl_version):
    rows = crate_licenses()
    lines = [
        "# Third-party notices",
        "",
        "Air itself is dual licensed MIT OR Apache-2.0.",
        "",
        "## Bundled software",
        "",
        f"* **Steel Bank Common Lisp {sbcl_version}** (`lisp/`) - public domain with BSD-style parts (MIT, Symbolics, Xerox);",
        "  full text in `licenses/SBCL-COPYING.txt`. Source: https://www.sbcl.org / https://github.com/sbcl/sbcl",
        "* **JetBrains Mono** - SIL OFL 1.1, `licenses/JetBrainsMono-OFL.txt` (https://github.com/JetBrains/JetBrainsMono)",
        "* **Inter** - SIL OFL 1.1, `licenses/Inter-OFL.txt` (https://github.com/rsms/inter)",
    ]
    for extra in sorted((ROOT / "assets" / "licenses").glob("*")):
        if extra.name not in ("SBCL-COPYING.txt",):
            lines.append(f"* see `licenses/{extra.name}`")
    lines += ["", "## Rust crates linked into the executables", "", "| crate | version | license |", "|---|---|---|"]
    lines += [f"| {n} | {v} | {l} |" for n, v, l, _ in rows]
    return "\n".join(lines) + "\n"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--sbcl-dir")
    ap.add_argument("--no-zip", action="store_true")
    ap.add_argument("--skip-build", action="store_true")
    a = ap.parse_args()

    if not a.skip_build:
        subprocess.run(["cargo", "build", "--release", "-p", "air", "-p", "air-cli"], cwd=ROOT, check=True)
    sbcl = find_sbcl_dir(a.sbcl_dir)
    ver = version()
    arch = platform.machine().lower().replace("amd64", "x64").replace("x86_64", "x64")
    name = f"Air-{ver}-{platform.system().lower()}-{arch}"
    dist = ROOT / "target" / "dist" / name
    if dist.exists():
        shutil.rmtree(dist)
    (dist / "lisp").mkdir(parents=True)
    (dist / "licenses").mkdir()

    for exe in ("air", "air-cli"):
        src = ROOT / "target" / "release" / f"{exe}{EXE}"
        if not src.is_file():
            sys.exit(f"missing {src}; build first")
        shutil.copy2(src, dist / src.name)

    shutil.copy2(sbcl / f"sbcl{EXE}", dist / "lisp")
    shutil.copy2(sbcl / "sbcl.core", dist / "lisp")
    shutil.copytree(sbcl / "contrib", dist / "lisp" / "contrib")
    sbcl_ver = subprocess.run([str(sbcl / f"sbcl{EXE}"), "--version"], capture_output=True, text=True).stdout.strip() or "unknown"

    for f in (ROOT / "assets" / "licenses").glob("*"):
        shutil.copy2(f, dist / "licenses" / f.name)
    for f in (ROOT / "assets" / "fonts").glob("*-OFL.txt"):
        shutil.copy2(f, dist / "licenses" / f.name)
    (dist / "THIRD_PARTY_NOTICES.md").write_text(notices(sbcl_ver), encoding="utf-8")
    shutil.copy2(ROOT / "README.md", dist / "README.md")
    (dist / "START_HERE.txt").write_text(
        "Air - an IDE for Lisp\n\n"
        f"Run {'air.exe' if os.name == 'nt' else './air'}. Nothing else has to be installed:\n"
        "the REPL, compiler and ASDF come from the bundled SBCL in the 'lisp' folder.\n"
        "Git features and 'Get from Version Control' use the git on your PATH (https://git-scm.com).\n\n"
        f"{sbcl_ver} is bundled; its licence is in licenses/SBCL-COPYING.txt.\n",
        encoding="utf-8",
    )

    size = sum(f.stat().st_size for f in dist.rglob("*") if f.is_file()) / 1e6
    print(f"built {dist}  ({size:.0f} MB, {sbcl_ver})")
    if not a.no_zip:
        zpath = dist.parent / f"{name}.zip"
        if zpath.exists():
            zpath.unlink()
        with zipfile.ZipFile(zpath, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
            for f in sorted(dist.rglob("*")):
                if f.is_file():
                    z.write(f, pathlib.Path(name) / f.relative_to(dist))
        print(f"zip: {zpath}  ({zpath.stat().st_size / 1e6:.0f} MB)")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""End-to-end tests: drive the real AirLisp window with JSON scripts (see apps/airlisp/src/script.rs).

Usage: python tests/run_e2e.py [--release] [name-substring ...]
Each tests/e2e/*.json script runs in a fresh fixture project with an isolated config directory.
Screenshots (taken by the app itself, never of the desktop) are written to target/e2e-shots/.
"""
import json
import os
import pathlib
import shutil
import subprocess
import sys
import stat
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent
PROFILE = "release" if "--release" in sys.argv else "debug"
EXE = pathlib.Path(os.environ["AIRLISP_E2E_EXE"]) if os.environ.get("AIRLISP_E2E_EXE") else ROOT / "target" / PROFILE / ("airlisp.exe" if os.name == "nt" else "airlisp")
CLI = ROOT / "target" / PROFILE / ("airlisp-cli.exe" if os.name == "nt" else "airlisp-cli")
SHOTS = ROOT / "target" / "e2e-shots"

FIXTURE_FILES = {
    "src/scratch.lisp": "",
    "src/hello.lisp": '(format t "Hi from run~%")\n',
    "src/warn.lisp": '(defun f (x)\n  (+ x "a"))\n\n(defun g ()\n  (missing-function 1))\n',
    "src/nav.lisp": "(defun alpha (x)\n  (* x 2))\n\n(defun beta (y)\n  (alpha y))\n",
    "src/messy.lisp": "(defun f (x)\n(let ((a 1)\n(b 2))\n(+ a b x)))\n",
    "src/bad.lisp": "(defun broken (x\n  (+ x 1)\n",
    "src/skip.fasl": "compiled",
    "build/out.txt": "generated",
}


def fwd(p):
    return str(p).replace("\\", "/")


def make_fixture(tmp):
    r = subprocess.run([str(CLI), "new-project", str(tmp), "demo"], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit(f"cannot create fixture: {r.stderr}")
    proj = tmp / "demo"
    for rel, content in FIXTURE_FILES.items():
        (proj / rel).parent.mkdir(parents=True, exist_ok=True)
        (proj / rel).write_text(content, encoding="utf-8")
    # every fixture is a git repository with one commit, so Git features have something to show
    def git(*args):
        subprocess.run(["git", "-c", "commit.gpgsign=false", *args], cwd=proj, capture_output=True, check=True)
    (proj / ".gitignore").write_text("*.fasl\n/build/\n", encoding="utf-8")
    git("init", "-q", "-b", "main")
    git("config", "user.email", "e2e@example.com")
    git("config", "user.name", "E2E")
    git("config", "core.autocrlf", "false")
    git("add", "-A")
    git("commit", "-q", "-m", "initial commit")
    return proj


def run_one(script_path):
    name = script_path.stem
    tmp = pathlib.Path(tempfile.mkdtemp(prefix=f"airlisp-e2e-{name}-"))
    try:
        proj = make_fixture(tmp)
        text = script_path.read_text(encoding="utf-8")
        text = text.replace("${PROJ}", fwd(proj)).replace("${SHOTS}", fwd(SHOTS)).replace("${NAME}", name)
        script = tmp / "script.json"
        script.write_text(text, encoding="utf-8")
        json.loads(text)  # validate
        if "@no_git" in text:
            def _unlock(func, path, exc):
                os.chmod(path, stat.S_IWRITE)
                func(path)

            shutil.rmtree(proj / ".git", onexc=_unlock)
        env = dict(os.environ, AIRLISP_CONFIG_DIR=str(tmp / "cfg"))
        if os.environ.get("AIRLISP_E2E_STRIP_SBCL"):
            # prove that the bundled SBCL is used: no system SBCL on PATH, no explicit override
            env.pop("AIRLISP_SBCL", None)
            sep = os.pathsep
            env["PATH"] = sep.join(p for p in env.get("PATH", "").split(sep) if "sbcl" not in p.lower() and "steel bank" not in p.lower())
        t0 = time.time()
        try:
            p = subprocess.run([str(EXE), "--script", str(script), "--size", "1280x800", "--no-restore"], capture_output=True, text=True, timeout=180, env=env)
        except subprocess.TimeoutExpired as e:
            return name, False, f"TIMEOUT after 180s\n{(e.stdout or b'').decode(errors='replace') if isinstance(e.stdout, bytes) else e.stdout}", time.time() - t0
        out = (p.stdout or "") + (p.stderr or "")
        ok = p.returncode == 0 and "SCRIPT OK" in out
        return name, ok, out, time.time() - t0
    finally:
        if os.environ.get("KEEP"):
            print(f"  kept fixture: {tmp}")
        else:
            shutil.rmtree(tmp, ignore_errors=True)


def main():
    if not EXE.exists() or not CLI.exists():
        raise SystemExit(f"build first: cargo build{' --release' if PROFILE == 'release' else ''} -p airlisp -p airlisp-cli")
    SHOTS.mkdir(parents=True, exist_ok=True)
    filters = [a for a in sys.argv[1:] if not a.startswith("--")]
    scripts = sorted((ROOT / "tests" / "e2e").glob("*.json"))
    if filters:
        scripts = [s for s in scripts if any(f in s.stem for f in filters)]
    failed = []
    for s in scripts:
        name, ok, out, dt = run_one(s)
        print(f"{'PASS' if ok else 'FAIL'}  {name:<22} {dt:5.1f}s")
        if not ok:
            failed.append(name)
            for line in out.strip().splitlines()[-25:]:
                print("      " + line)
    print(f"\n{len(scripts) - len(failed)}/{len(scripts)} e2e scripts passed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()

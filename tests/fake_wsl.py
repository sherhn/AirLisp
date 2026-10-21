#!/usr/bin/env python3
"""A stand-in for wsl.exe used by the UI tests (AIRLISP_FAKE_WSL points at this file).

It never touches the real WSL. Its state (is WSL "installed", which distributions exist, which have SBCL) lives in
the JSON file named by AIRLISP_FAKE_WSL_STATE; AIRLISP_FAKE_WSL_START=absent starts without WSL. Programs "inside"
a distribution run on Windows: /mnt/<drive>/... paths are mapped back, `sbcl` is the Windows SBCL
(AIRLISP_FAKE_WSL_SBCL or PATH) and `sh -lc LINE` becomes `cmd /c LINE`.
"""
import json
import os
import shutil
import subprocess
import sys
import time

STATE = os.environ.get("AIRLISP_FAKE_WSL_STATE") or os.path.join(os.environ.get("TEMP", "."), "airlisp-fake-wsl.json")
ONLINE = [("Debian", "Debian GNU/Linux"), ("Ubuntu", "Ubuntu"), ("Ubuntu-24.04", "Ubuntu 24.04 LTS"), ("Ubuntu-22.04", "Ubuntu 22.04 LTS"), ("archlinux", "Arch Linux")]


def load():
    try:
        with open(STATE, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return {"installed": os.environ.get("AIRLISP_FAKE_WSL_START") != "absent", "distros": {}, "default": None}


def save(s):
    with open(STATE, "w", encoding="utf-8") as f:
        json.dump(s, f)


def out(text):
    sys.stdout.write(text)
    sys.stdout.flush()


def progress(label):
    out(f"{label}\n")
    for p in (0, 25, 50, 75, 100):
        bar = "=" * (p // 5)
        out(f"\r[{bar:<20}] {p}.0%")
        time.sleep(0.12)
    out("\n")


def to_windows(arg):
    if arg.startswith("/mnt/") and len(arg) >= 6 and arg[5].isalpha() and (len(arg) == 6 or arg[6] == "/"):
        return arg[5].upper() + ":/" + arg[7:]
    return arg


def no_distro(name):
    out(f"There is no distribution with the supplied name: {name}\n")
    return 255


def run_inside(s, name, cwd, prog, args):
    d = s["distros"].get(name)
    if d is None:
        return no_distro(name)
    cwd = to_windows(cwd) if cwd else None
    if prog == "wslpath":
        out("/mnt/c/\n")
        return 0
    if prog == "sh" and args[:1] == ["-c"] and "pgrep" in (args[1] if len(args) > 1 else ""):
        # the cleanup of a tagged process: end the Windows programs carrying the marker
        marker = (args[3] if len(args) > 3 else "").replace("[", "").replace("]", "")
        if marker:
            ps = ("Get-CimInstance Win32_Process | Where-Object { ($_.Name -eq 'sbcl.exe' -or $_.Name -eq 'cmd.exe') -and $_.CommandLine -like '*" + marker + "*' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }")
            subprocess.run(["powershell", "-NoProfile", "-Command", ps], capture_output=True)
        return 0
    if prog == "sh" and args[:1] == ["-c"]:
        script = args[1] if len(args) > 1 else ""
        if "apt-get install -y sbcl" in script:
            for line in ["Hit:1 http://archive.ubuntu.com/ubuntu noble InRelease", "Reading package lists... Done", "Setting up sbcl (2:2.2.9-1) ...", "SBCL 2.2.9.debian"]:
                out(line + "\n")
                time.sleep(0.05)
            d["sbcl"] = True
            save(s)
            return 0
        out("fake wsl: unsupported script\n")
        return 1
    if prog == "sh" and args[:1] == ["-lc"] and "AIRLISP_SBCL=" in (args[1] if len(args) > 1 else ""):
        out("AIRLISP_SBCL=" + ("/usr/bin/sbcl" if d.get("sbcl") else "") + "\n")
        if d.get("sbcl"):
            out("AIRLISP_VERSION=SBCL 2.2.9.debian\n")
        out("AIRLISP_MOUNT=/mnt/c/\n")
        return 0
    env = dict(os.environ)
    if prog == "env":
        while args and "=" in args[0] and not args[0].startswith("-"):
            k, v = args.pop(0).split("=", 1)
            env[k] = v
        prog = args.pop(0) if args else ""
    if prog == "sh" and args[:1] == ["-lc"]:
        line = args[1] if len(args) > 1 else ""
        marker = args[2] if len(args) > 2 else ""
        return subprocess.call(f"cmd /d /c {line} & rem {marker}", cwd=cwd, env=env)
    if prog.endswith("sbcl"):
        if not d.get("sbcl"):
            out(f"execvpe({prog}) failed: No such file or directory\n")
            return 1
        sbcl = os.environ.get("AIRLISP_FAKE_WSL_SBCL") or shutil.which("sbcl")
        if not sbcl:
            out("fake wsl: no Windows SBCL to stand in for Linux SBCL\n")
            return 1
        return subprocess.call([sbcl] + [to_windows(a) for a in args], cwd=cwd, env=env)
    out(f"fake wsl: cannot run {prog}\n")
    return 1


def main(argv):
    s = load()
    if not argv:
        return 0
    if argv[0] == "--version":
        if not s["installed"]:
            out("Usage: wsl.exe [Argument] [Options...] [CommandLine]\n")
            return 1
        out("WSL version: 2.6.1.0\nKernel version: 6.6.87.2-1\n")
        return 0
    if argv[0] == "--status":
        if not s["installed"]:
            out("The Windows Subsystem for Linux is not installed.\n")
            return 1
        out("Default Version: 2\n")
        return 0
    if argv[0] in ("--list", "-l"):
        rest = argv[1:]
        if "--online" in rest or "-o" in rest:
            out("The following is a list of valid distributions that can be installed.\n\nNAME            FRIENDLY NAME\n")
            for n, f in ONLINE:
                out(f"{n:<16}{f}\n")
            return 0
        if not s["distros"]:
            out("Windows Subsystem for Linux has no installed distributions.\n")
            return 255
        out("  NAME            STATE           VERSION\n")
        for n in s["distros"]:
            star = "*" if n == s.get("default") else " "
            out(f"{star} {n:<15} Stopped         2\n")
        return 0
    if argv[0] == "--install":
        rest = argv[1:]
        if "--no-distribution" in rest:
            progress("Installing: Windows Subsystem for Linux")
            s["installed"] = True
            save(s)
            out("The requested operation is successful.\n")
            return 0
        if not s["installed"]:
            out("WSL is not installed.\n")
            return 1
        name = None
        if "-d" in rest:
            name = rest[rest.index("-d") + 1]
        if "--name" in rest:
            name = rest[rest.index("--name") + 1]
        if not name:
            out("fake wsl: no distribution given\n")
            return 1
        if name in s["distros"]:
            out(f"A distribution with the supplied name already exists: {name}\n")
            return 1
        progress(f"Downloading: {name}")
        out(f"Installing: {name}\nDistribution successfully installed.\n")
        s["distros"][name] = {"sbcl": False}
        s["default"] = s.get("default") or name
        save(s)
        return 0
    if argv[0] == "--import":
        name = argv[1]
        s["distros"][name] = {"sbcl": False}
        s["default"] = s.get("default") or name
        save(s)
        out("The operation completed successfully.\n")
        return 0
    # -d NAME [-u USER] [--cd DIR] -e PROGRAM ARGS...
    name, cwd, i = s.get("default"), None, 0
    while i < len(argv):
        a = argv[i]
        if a in ("-d", "--distribution"):
            name = argv[i + 1]
            i += 2
        elif a in ("-u", "--user"):
            i += 2
        elif a == "--cd":
            cwd = argv[i + 1]
            i += 2
        elif a in ("-e", "--exec"):
            return run_inside(s, name, cwd, argv[i + 1], list(argv[i + 2:]))
        else:
            out(f"fake wsl: unexpected argument {a}\n")
            return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

#!/usr/bin/env python3
"""Holds TCS's names to what they say: production code is built only from
production code.

Production code is what runs on a vehicle or in the MOC. A TCS name says
which it is: find the prefix, ``tcslog`` if the name starts with it and
``tcs`` otherwise, and look at what follows. A letter, or nothing, is
production -- tcspecial, tcsmoc, tcsprotocol, tcslog, tcsxtce -- and a
hyphen or an underscore is not -- tcs-gui, tcs-verify, tcs_xtce,
tcslog-tools. A name that does not start with ``tcs`` is not TCS's, and
nothing is said of it. This is the one statement of that rule; CLAUDE.md
and the documents point here.

What is checked:

* Every production crate of the workspace is built from no crate with a
  non-production name, by its normal dependencies.
* tcspecial as install-build builds it -- in one cargo run with every crate
  that is installed, whose features cargo pools -- likewise. Should that
  build have the visual window, it is said, as a warning and not an error:
  the window and tcs-diagram, which draws it, are for the desk, and a
  tcspecial with them is not one to fly.
* Every Python package python/pyproject.toml installs, which is what
  tcspecial-moc ships to ground stations, has a production name and
  imports no package with a non-production name.

A dependency in EXCEPTIONS is said as a warning rather than refused, with
why, until it can go.

usage: tests/production-names.py [--features FEATURES] [INSTALL_CRATE...]
"""

import argparse
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path

TOP = Path(__file__).resolve().parent.parent

#: Non-production names production code may still be built from, each with
#: why. Each is a warning on every run, so that it is not forgotten.
EXCEPTIONS = {
    "tcs-xtce": "tcsprotocol packs and unpacks with tcs-xtce's packet "
                "runtime, which is becoming the production crate "
                "tcsxtce; switch to tcsxtce once it is published",
}

#: One line of `cargo tree --prefix depth --format '{p}|{f}'`: the depth,
#: the crate's name, and its features.
TREE_LINE = re.compile(r"^(\d+)(\S+) v\S+.*?\|(.*)$")

#: An import in Python, by the name of the top package imported.
IMPORT = re.compile(r"^\s*(?:from|import)\s+([A-Za-z_]\w*)", re.M)


def prefix_of(name):
    """Gives the TCS prefix a name starts with.

    Args:
        name: A crate's or a package's name.

    Returns:
        ``tcslog`` or ``tcs``, the longer one where both fit; or None for
        a name that is not TCS's.
    """
    for prefix in ("tcslog", "tcs"):
        if name.startswith(prefix):
            return prefix
    return None


def is_production(name):
    """Says whether a TCS name is production code's.

    Args:
        name: A TCS name: one that prefix_of finds a prefix in.

    Returns:
        True where a letter, or nothing, follows the prefix; False where
        a hyphen or an underscore does.
    """
    rest = name[len(prefix_of(name)):]
    return rest == "" or rest[0].isalpha()


def is_tool(name):
    """Says whether a name is TCS's and not production code's.

    Args:
        name: Any crate's or package's name.

    Returns:
        True for a TCS name with a hyphen or underscore after its prefix.
    """
    return prefix_of(name) is not None and not is_production(name)


def workspace_crates():
    """Lists the workspace's crates.

    Returns:
        The name of each member of the workspace.
    """
    out = subprocess.run(["cargo", "metadata", "--no-deps", "--offline",
                          "--format-version", "1"],
                         cwd=TOP, check=True, capture_output=True,
                         text=True).stdout
    return [package["name"] for package in json.loads(out)["packages"]]


def trees(crates, features):
    """Gives the normal-dependency trees of crates built in one cargo run.

    Args:
        crates:   The crates, given to cargo together, as a build of them
                  all would be, so that their features are pooled.
        features: Features to build with, as cargo's --features takes
                  them, or None.

    Returns:
        A dict from each of crates to a list of (name, features) for it
        and every crate it is built from, its own first.
    """
    command = ["cargo", "tree", "--offline", "--no-dedupe", "-e", "normal",
               "--prefix", "depth", "--format", "{p}|{f}"]
    for crate in crates:
        command += ["-p", crate]
    if features:
        command += ["--features", features]
    out = subprocess.run(command, cwd=TOP, check=True,
                         capture_output=True, text=True).stdout
    found, root = {}, None
    for line in out.splitlines():
        m = TREE_LINE.match(line)
        if not m:
            continue
        depth, name, feats = int(m.group(1)), m.group(2), m.group(3)
        if depth == 0:
            root = name
            found.setdefault(root, [])
        if root is not None:
            found[root].append((name, set(filter(None, feats.split(",")))))
    return found


def check_crate(crate, built_from, problems, excepted, allowed=()):
    """Says what a production crate is built from that it should not be.

    Args:
        crate:      The crate.
        built_from: (name, features) for it and every crate it is built
                    from, as trees gives them.
        problems:   The list each error is added to.
        excepted:   A dict from each name in EXCEPTIONS to the crates
                    found built from it, which this adds to.
        allowed:    Non-production names to pass over here, which the
                    caller has warned of itself.
    """
    for name in sorted({name for name, _ in built_from[1:]}):
        if not is_tool(name) or name in allowed:
            continue
        if name in EXCEPTIONS:
            excepted.setdefault(name, []).append(crate)
        else:
            problems.append(f"{crate}, production code, is built from "
                            f"{name}, which has a tool's name: take the "
                            f"dependency out, or give {name} a production "
                            "name if it runs on a vehicle or in the MOC")


def check_python(problems):
    """Says what is wrong with the names of the Python packages installed.

    Args:
        problems: The list each error is added to.
    """
    project = TOP / "python" / "pyproject.toml"
    with open(project, "rb") as f:
        packages = tomllib.load(f)["tool"]["setuptools"]["packages"]
    for package in packages:
        if is_tool(package.split(".")[0]):
            problems.append(f"{project.relative_to(TOP)}: {package} has a "
                            "tool's name, and tcspecial-moc ships it to "
                            "ground stations: give it a production name, "
                            "or leave it out")
        here = TOP / "python" / package.replace(".", "/")
        for source in sorted(here.glob("*.py")):
            for imported in IMPORT.findall(source.read_text()):
                if is_tool(imported):
                    problems.append(f"{source.relative_to(TOP)}: imports "
                                    f"{imported}, which has a tool's "
                                    "name, into a package ground "
                                    "stations are built on")


def main():
    """Runs every check, and says what it found.

    Returns:
        0 if nothing is wrong, warnings or not; 1 otherwise.
    """
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--features",
                        help="features to check the installed build with, "
                             "as cargo takes them")
    parser.add_argument("install", nargs="*",
                        help="the crates install-build builds together")
    args = parser.parse_args()
    problems, warnings, excepted = [], [], {}

    crates = [crate for crate in workspace_crates()
              if prefix_of(crate) and is_production(crate)]
    built = trees(crates, None)
    for crate in crates:
        check_crate(crate, built[crate], problems, excepted)

    tcspecial = trees(args.install, args.features).get("tcspecial") \
        if args.install else None
    if tcspecial and "visual" in tcspecial[0][1]:
        warnings.append("the tcspecial built for installing has the visual "
                        "window, and tcs-diagram with it: that is a desk "
                        "build, never one to fly. Build the one that flies "
                        "without --features visual, and with no installed "
                        "crate asking for it")
        check_crate("tcspecial", tcspecial, problems, excepted,
                    allowed=("tcs-diagram",))
    elif tcspecial:
        check_crate("tcspecial", tcspecial, problems, excepted)

    check_python(problems)

    for name, crates_of in excepted.items():
        warnings.append(f"{name} has a tool's name and production code is "
                        f"built from it ({', '.join(dict.fromkeys(crates_of))}"
                        f"): {EXCEPTIONS[name]}")
    for warning in dict.fromkeys(warnings):
        print(f"production-names: warning: {warning}", file=sys.stderr)
    for problem in dict.fromkeys(problems):
        print(f"production-names: error: {problem}", file=sys.stderr)
    if problems:
        return 1
    print(f"production-names: {len(crates)} production crates, the "
          "installed tcspecial and the Python packages are built only from "
          "production code")
    return 0


if __name__ == "__main__":
    sys.exit(main())

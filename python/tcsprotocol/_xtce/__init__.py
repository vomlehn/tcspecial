"""The part of tcs-xtce this package uses: tcsxtce, its runtime, which the
fixed commands', the beacon's and the ping's generated code is built on, and
which computes the interface's digest.

tcs-xtce's Python packages are not published, so this tree takes tcsxtce
from a checkout of tcs-xtce beside it, ``../tcs-xtce``, at the release it is
pinned to in tcsprotocol/Cargo.toml. The five modules here are links to that
checkout's ``python/tcsxtce`` -- ``packet.py``, ``_buffer.py``, ``value.py``,
``digest.py`` and ``_error.py`` -- so there is one copy of them; but they
are inside this package, so that installing the package installs them and
an installed ground station needs nothing from the source tree. Nothing else
of tcs-xtce is anything a ground station reads.

A git worktree of the tree lies deeper than the main checkout -- under
``.claude/worktrees`` -- so there the links, being relative, point at a
tcs-xtce beside the worktree, where there is none. Where they point at
nothing, the modules are looked for in the tcs-xtce beside the main checkout
instead, which is where the Makefiles look too (``beside.mk``). Only then:
an installed package and the main checkout read the links as they are.

When tcsxtce's Python package is published, this goes, and the distribution
depends on it instead.
"""

import os as _os


def _main_checkout(tree):
    """Finds the main checkout of a git worktree, as git does: the
    worktree's ``.git`` is a file naming its directory in the main
    checkout's ``.git``, whose ``commondir`` leads back to that ``.git``.

    Args:
        tree: The root of the tree this package is in.

    Returns:
        The main checkout's root, or None where ``tree`` is not a worktree.
    """
    try:
        with open(_os.path.join(tree, ".git"), encoding="utf-8") as f:
            said = f.read().strip()
        if not said.startswith("gitdir: "):
            return None
        gitdir = said[len("gitdir: "):]
        with open(_os.path.join(gitdir, "commondir"), encoding="utf-8") as f:
            common = _os.path.join(gitdir, f.read().strip())
    except OSError:
        # A .git that is a directory -- the main checkout -- or none at all,
        # an installed package: no worktree, and nothing to look for.
        return None
    return _os.path.dirname(_os.path.normpath(common))


def _beside_main_checkout():
    """Gives where the runtime is in the tcs-xtce beside the main checkout,
    where this package's links point at nothing.

    Returns:
        The directory, or None where the links are good or this is not a
        worktree.
    """
    here = _os.path.dirname(_os.path.abspath(__file__))
    if _os.path.exists(_os.path.join(here, "packet.py")):
        return None
    # python/tcsprotocol/_xtce, three below the tree's root.
    tree = _os.path.dirname(_os.path.dirname(_os.path.dirname(here)))
    main = _main_checkout(tree)
    if main is None:
        return None
    runtime = _os.path.join(
        _os.path.dirname(main), "tcs-xtce", "python", "tcsxtce"
    )
    return runtime if _os.path.isdir(runtime) else None


_runtime = _beside_main_checkout()
if _runtime is not None:
    # After this directory, whose links are skipped as files that are not
    # there, so the modules are found in the main checkout's tcs-xtce.
    __path__.append(_runtime)

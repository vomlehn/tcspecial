"""The distribution's version is the workspace's.

python/pyproject.toml states the version pip installs, and the workspace's
Cargo.toml the one every crate and system package carries. A version is
stated once wherever it can be, but pyproject.toml has to state its own, so
this holds the two together instead.
"""

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def stated(path, section):
    """The ``version`` a TOML file states in one section.

    Read with a pattern rather than a TOML parser, which the standard
    library has only from Python 3.11.

    Args:
        path:    The file.
        section: The section's header, such as ``[project]``.

    Returns:
        The version, or ``None`` where the section states none.
    """
    text = path.read_text(encoding="utf-8")
    start = text.find(section + "\n")
    if start < 0:
        return None
    body = text[start + len(section):]
    end = re.search(r"^\[", body, re.MULTILINE)
    body = body[:end.start()] if end else body
    found = re.search(r'^version\s*=\s*"([^"]+)"', body, re.MULTILINE)
    return found.group(1) if found else None


class VersionTest(unittest.TestCase):
    def test_the_distribution_is_the_workspaces_version(self):
        workspace = stated(ROOT / "Cargo.toml", "[workspace.package]")
        distribution = stated(ROOT / "python" / "pyproject.toml", "[project]")
        self.assertIsNotNone(workspace)
        self.assertEqual(distribution, workspace)


if __name__ == "__main__":
    unittest.main()

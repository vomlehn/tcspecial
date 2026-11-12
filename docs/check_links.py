"""Checks that the relative links in the pages named on the command line
lead somewhere: to a file that exists and, when the link names an
anchor, to an element in it with that id.

A link into another TCS project, a checkout beside this one such as
../../tcslog, is checked only when that checkout is there, since the
links say where its documents are in one and not every tree has every
project beside it.

Run as `python3 check_links.py PAGE...`; it prints each link that leads
nowhere and exits 1 if there is one.
"""

import html
import re
import sys
from pathlib import Path

# Where this repository's checkout is: links that leave it go to the
# other TCS projects' checkouts beside it.
TOP = Path(__file__).resolve().parent.parent


def ids(path):
	"""Reads the ids of a page's elements.

	Args:
		path: The page.

	Returns:
		The set of its elements' id attributes.
	"""
	text = path.read_text(encoding="utf-8", errors="replace")
	return set(re.findall(r'\bid="([^"]+)"', text))


def broken(page):
	"""Finds a page's relative links that lead nowhere.

	Args:
		page: The page.

	Returns:
		The href of each of its relative links whose file is
		missing, or that names an anchor its file has no element
		for, other than those into a project not checked out.
	"""
	text = page.read_text(encoding="utf-8")
	bad = []
	for href in re.findall(r'href="([^"]+)"', text):
		href = html.unescape(href)
		if re.match(r"[a-z]+:", href) or href.startswith("#"):
			continue
		name, _, anchor = href.partition("#")
		target = (page.parent / name).resolve()
		if not target.is_relative_to(TOP):
			project = target.relative_to(TOP.parent).parts[0]
			if not (TOP.parent / project).is_dir():
				continue
		if not target.is_file() or (anchor
		                            and anchor not in ids(target)):
			bad.append(href)
	return bad


def main():
	"""Checks each page named on the command line."""
	failed = False
	for name in sys.argv[1:]:
		for href in broken(Path(name)):
			print(f"{name}: {href} leads nowhere", file=sys.stderr)
			failed = True
	sys.exit(1 if failed else 0)


if __name__ == "__main__":
	main()

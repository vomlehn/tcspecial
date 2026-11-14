"""Counts the non-test source statements of the tcs projects checked out
beside tcspecial, in ../tcs*: `make ntss` in tcspecial.

A *source statement* here is a line that carries code: not blank, and not
only a comment, a Python docstring, or the rest of a comment that began on
an earlier line. It is the usual stand-in for a count of statements, and
the same rule for every language, so that the projects can be compared.
A statement written over several lines counts once for each line.

*Non-test* leaves out:

- anything under a directory named tests, test, testing, benches or fuzz;
- files named as tests: test_*.py, *_test.*, *_tests.*, test-*, conftest.py;
- in Rust, every item marked #[cfg(test)] or #[cfg(all(test, ...))], such
  as a file's `mod tests`;
- generated code, whose first lines say @generated, since nobody writes it.

Code written for another project is left out too: anything under a
project's top-level directory named for one, such as tcspecial's cfs and
fprime, which hold a cFS mission's definitions and an F' component built
with that project's own tools. See OTHER_PROJECTS.

Only files git tracks are read, so build output is never counted, and
symbolic links are not followed, so a file one project links from another
is counted once, where it lives.

A project is a ../tcs* git checkout whose name is letters, digits and
hyphens (so a renamed ../tcslog-sample.DELETEME is not one), tcspecial
included. Its subprojects are its Rust crates, Python packages and CMake
projects, each the deepest directory below the project's top with a
Cargo.toml holding [package], a pyproject.toml, a setup.py or a
CMakeLists.txt; a file in none of them goes under the first directory of
its path, or "(top)" for one at the top.

Run as `python3 tools/ntss.py`, which is what `make ntss` does.
"""

import re
import subprocess
import sys
import tokenize
from io import StringIO
from pathlib import Path

# Where this repository is -- tools/ is one below its top -- and the
# directory the projects are checked out in.
HERE = Path(__file__).resolve().parent.parent
TOP = HERE.parent

# The columns, in order, and which files are in each.
LANGUAGES = ["Rust", "C/C++", "Python", "Shell", "Make", "Slint"]
EXTENSIONS = {
    ".rs": "Rust",
    ".c": "C/C++",
    ".h": "C/C++",
    ".cc": "C/C++",
    ".cpp": "C/C++",
    ".hpp": "C/C++",
    ".py": "Python",
    ".sh": "Shell",
    ".mk": "Make",
    ".slint": "Slint",
}

# Directories whose contents are tests, wherever they are in a path.
TEST_DIRECTORIES = {"tests", "test", "testing", "benches", "fuzz"}

# Top-level directories of a project that hold code written for another
# project, in its framework and built with its tools: cFS and F'.
OTHER_PROJECTS = {"cfs", "fprime"}

# File names that say a file is a test.
TEST_FILE = re.compile(r"^(test_.*\.py|.*_tests?\..*|test-.*|conftest\.py)$")

# What a subproject's directory holds.
MANIFESTS = ["Cargo.toml", "pyproject.toml", "setup.py", "CMakeLists.txt"]

# A character literal, which a ' that does not begin one, a Rust lifetime
# or label such as 'a, is told from.
CHAR = re.compile(r"'(\\(u\{[0-9a-fA-F]+\}|x..|.)|[^\\'\n])'")

# The attributes that make a Rust item test-only.
CFG_TEST = re.compile(r"#\s*\[\s*cfg\s*\(\s*(test\s*\)|all\s*\(\s*test\b)")


def language(path):
    """Says which column a file is counted in.

    Args:
        path: The file.

    Returns:
        One of LANGUAGES, or None for a file that is not source.
    """
    if path.name == "Makefile" or path.name == "GNUmakefile":
        return "Make"
    found = EXTENSIONS.get(path.suffix)
    if found is not None:
        return found
    if path.suffix == "":
        try:
            with path.open("rb") as f:
                first = f.readline(128)
        except OSError:
            return None
        if re.match(rb"#!\s*\S*(/|env\s+)(ba|da)?sh\b", first):
            return "Shell"
    return None


def is_other_project(relative):
    """Says whether a file is code written for another project.

    Args:
        relative: The file's path within its project.

    Returns:
        Whether its first directory is one of OTHER_PROJECTS.
    """
    return len(relative.parts) > 1 and relative.parts[0] in OTHER_PROJECTS


def is_test(relative):
    """Says whether a file is a test by where it is and what it is called.

    Args:
        relative: The file's path within its project.

    Returns:
        Whether it is a test.
    """
    if TEST_DIRECTORIES & set(relative.parts[:-1]):
        return True
    return TEST_FILE.match(relative.name) is not None


def is_generated(text):
    """Says whether a file is generated, as its first lines say.

    Args:
        text: What the file says.

    Returns:
        Whether its first five lines say @generated.
    """
    return "@generated" in "\n".join(text.split("\n", 5)[:5])


def c_like(text, rust):
    """Blanks the comments of a C, C++, Rust or Slint file.

    String and character literals are kept as code, with their contents
    replaced, so that a quote or a brace inside one is not taken for the
    start of a comment or the end of a block.

    Args:
        text: What the file says.
        rust: Whether it is Rust, whose block comments nest, which has
            raw strings, and whose ' may begin a lifetime.

    Returns:
        The text with every comment character a space and every
        literal's contents an S, its newlines where they were.
    """
    out = []
    i = 0
    n = len(text)

    def blank(s):
        return "".join(c if c == "\n" else " " for c in s)

    def literal(s):
        return "".join(c if c == "\n" else "S" for c in s)

    while i < n:
        c = text[i]
        if text.startswith("//", i):
            end = text.find("\n", i)
            end = n if end < 0 else end
            out.append(blank(text[i:end]))
            i = end
        elif text.startswith("/*", i):
            depth = 0
            j = i
            while j < n:
                if text.startswith("/*", j):
                    depth += 1
                    j += 2
                elif text.startswith("*/", j):
                    depth -= 1
                    j += 2
                    if depth == 0 or not rust:
                        break
                else:
                    j += 1
            out.append(blank(text[i:j]))
            i = j
        elif rust and re.match(r'b?r#*"', text[i:i + 260]) and not (
            i > 0 and (text[i - 1].isalnum() or text[i - 1] == "_")
        ):
            m = re.match(r'b?r(#*)"', text[i:])
            close = '"' + m.group(1)
            end = text.find(close, i + m.end())
            end = n if end < 0 else end
            out.append(text[i:i + m.end()])
            out.append(literal(text[i + m.end():end]))
            out.append(text[end:end + len(close)])
            i = end + len(close)
        elif c == '"':
            j = i + 1
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            out.append('"' + literal(text[i + 1:j]) + '"')
            i = j + 1
        elif c == "'":
            m = CHAR.match(text, i)
            if m is not None:
                inside = literal(m.group(0)[1:-1])
                out.append("'" + inside + "'")
                i = m.end()
            else:
                # A Rust lifetime or label, such as 'a: code.
                out.append(c)
                i += 1
        else:
            out.append(c)
            i += 1
    return "".join(out)


def without_test_items(code):
    """Blanks every Rust item marked test-only.

    Args:
        code: A Rust file with its comments blanked and its literals'
            contents replaced, by c_like.

    Returns:
        The code with each #[cfg(test)] item, its attribute
        included, made spaces.
    """
    chars = list(code)
    for m in CFG_TEST.finditer(code):
        start = m.start()
        i = code.find("]", m.end())
        if i < 0:
            continue
        i += 1
        depth = 0
        end = len(code)
        while i < len(code):
            c = code[i]
            if c in "([":
                depth += 1
            elif c in ")]":
                depth -= 1
            elif c == ";" and depth == 0:
                end = i + 1
                break
            elif c == "{" and depth == 0:
                braces = 0
                while i < len(code):
                    if code[i] == "{":
                        braces += 1
                    elif code[i] == "}":
                        braces -= 1
                        if braces == 0:
                            break
                    i += 1
                end = i + 1
                break
            i += 1
        for k in range(start, min(end, len(chars))):
            if chars[k] != "\n":
                chars[k] = " "
    return "".join(chars)


def python_lines(text):
    """Counts the lines of a Python file that carry code.

    Args:
        text: What the file says.

    Returns:
        How many lines hold a token of a statement that is not only
        a string, which is a docstring.
    """
    lines = set()
    statement = []
    skipped = {
        tokenize.COMMENT,
        tokenize.NL,
        tokenize.INDENT,
        tokenize.DEDENT,
        tokenize.ENCODING,
        tokenize.ENDMARKER,
    }
    try:
        for token in tokenize.generate_tokens(StringIO(text).readline):
            if token.type == tokenize.NEWLINE:
                if not all(t.type == tokenize.STRING
                       for t in statement):
                    spanned(statement, lines)
                statement = []
            elif token.type not in skipped:
                statement.append(token)
    except (tokenize.TokenError, SyntaxError):
        # Not Python this tokenizer can read: counted by lines instead.
        return hash_comment_lines(text)
    spanned(statement, lines)
    return len(lines)


def spanned(tokens, lines):
    """Adds the lines some tokens are on to a set of them.

    Args:
        tokens: The tokens.
        lines: The set of line numbers to add to.
    """
    for t in tokens:
        lines.update(range(t.start[0], t.end[0] + 1))


def hash_comment_lines(text):
    """Counts the lines of a shell script or makefile that carry code.

    Args:
        text: What the file says.

    Returns:
        How many lines are neither blank nor a # comment.
    """
    return sum(1 for line in text.splitlines()
           if line.strip() and not line.lstrip().startswith("#"))


def statements(path, kind):
    """Counts a file's non-test source statements.

    Args:
        path: The file.
        kind: Its language, one of LANGUAGES.

    Returns:
        How many, or 0 for a generated file.
    """
    text = path.read_text(encoding="utf-8", errors="replace")
    if is_generated(text):
        return 0
    if kind == "Python":
        return python_lines(text)
    if kind in ("Shell", "Make"):
        return hash_comment_lines(text)
    code = c_like(text, kind == "Rust")
    if kind == "Rust":
        code = without_test_items(code)
    return sum(1 for line in code.splitlines() if line.strip())


def projects():
    """Finds the projects.

    Returns:
        Each ../tcs* git checkout, by name.
    """
    return sorted(p for p in TOP.glob("tcs*")
              if re.fullmatch(r"tcs[a-z0-9-]*", p.name)
              and (p / ".git").exists())


def subprojects(project, files):
    """Finds a project's subprojects.

    Args:
        project: The project's directory.
        files: The paths git tracks, relative to it.

    Returns:
        The directories, relative to the project, that are
        subprojects, deepest first.
    """
    found = set()
    for f in files:
        if f.name not in MANIFESTS or f.parent == Path("."):
            continue
        if f.name == "Cargo.toml":
            text = (project / f).read_text(errors="replace")
            if not re.search(r"^\[package\]", text, re.M):
                continue
        found.add(f.parent)
    return sorted(found, key=lambda d: -len(d.parts))


def subproject_of(relative, dirs):
    """Says which subproject a file is counted under.

    Args:
        relative: The file's path within its project.
        dirs: The project's subprojects, deepest first.

    Returns:
        The subproject's name.
    """
    for d in dirs:
        if d in relative.parents:
            return str(d)
    if len(relative.parts) == 1:
        return "(top)"
    return relative.parts[0]


def count(project):
    """Counts a project.

    Args:
        project: The project's directory.

    Returns:
        {subproject: {language: statements}}.
    """
    listed = subprocess.run(
        ["git", "-C", str(project), "ls-files", "-z"],
        capture_output=True, check=True,
    ).stdout.decode("utf-8", errors="replace").split("\0")
    files = [Path(f) for f in listed if f]
    dirs = subprojects(project, files)
    counts = {}
    for relative in files:
        path = project / relative
        if path.is_symlink() or not path.is_file() or is_test(relative):
            continue
        if is_other_project(relative):
            continue
        kind = language(path)
        if kind is None:
            continue
        n = statements(path, kind)
        if n == 0:
            continue
        row = counts.setdefault(subproject_of(relative, dirs), {})
        row[kind] = row.get(kind, 0) + n
    return counts


def table(rows, width):
    """Lays out rows of counts.

    Args:
        rows: (name, {language: statements}) for each row.
        width: How wide the name column is.

    Returns:
        The lines.
    """
    lines = []
    for name, row in rows:
        cells = [row.get(k, 0) for k in LANGUAGES]
        lines.append(f"  {name:<{width}}"
                 + "".join(f"{c or '':>7}" for c in cells)
                 + f"{sum(cells):>8}")
    return lines


def main():
    """Prints the counts, project by project, with their totals."""
    results = [(p.name, count(p)) for p in projects()]
    names = [s for _, c in results for s in c] + ["total"]
    width = max(len(n) for n in names)
    header = (f"  {'':<{width}}" + "".join(f"{k:>7}" for k in LANGUAGES)
          + f"{'Total':>8}")
    grand = {}
    print("Non-test source statements: lines of code, less tests and "
          "generated code")
    for name, counts in results:
        print()
        print(name)
        print(header)
        total = {}
        for row in counts.values():
            for k, v in row.items():
                total[k] = total.get(k, 0) + v
                grand[k] = grand.get(k, 0) + v
        for line in table(sorted(counts.items()) + [("total", total)],
                  width):
            print(line)
    print()
    print("All projects")
    print(header)
    for line in table([("total", grand)], width):
        print(line)
    return 0


if __name__ == "__main__":
    sys.exit(main())

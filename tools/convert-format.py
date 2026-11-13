#!/usr/bin/env python3
"""Rewrite tcspecial configurations, simulator files and physics models from
the old shape into the new one.

The old shape wrote each node as an entry of its own, its name and its type
beside what it is told, in sections called ``gateway``, ``vehicle`` and
``payloads``::

    payloads:
      - name: temp_1
        type: i2c
        address: "0x48"

The new shape groups each section's nodes by type, and names each node by the
word for its section -- ``gateway``, ``compute`` or ``payload``::

    payloads:
      - type i2c:
          - payload: temp_1
            address: "0x48"

In XML the sections are one element each, ``<gateways>``, ``<vehicle>`` and
``<payloads>``, holding ``<type name="...">`` elements of ``<gateway>``,
``<compute>`` or ``<payload>`` elements named by a ``name`` attribute. The
editor's section names nodes the same way: ``- { node: temp_1, x: 34, y: 34 }``
and ``<node name="temp_1" x="34" y="34"/>``.

Everything is written in the one order a configuration has, which is by name:
the types of a section, the nodes of a type, a node's attributes, then its
links and their destinations, then the sinks it says a line arrives at.

A link is written as lines under the data source it leaves by, each the node
it reaches and the name it arrives there by, and the node it reaches lists
that name under the sink the line arrives at::

    payloads:
      - type i2c:
          - payload: temp_1
            world-out:
              - vote.in0
    vehicle:
      - type compute:
          - compute: vote
            function: vote
            world-in: [in0, in1, in2]

The name is the input an old link named, ``input: in0``, and otherwise the
name of the node the line leaves. The old shape's sinks, ``world-in: true``,
said only that a line arrives; they are written as the names that do. A
vote's ``inputs`` are the names under its world-in now, and are written there.
The names a node lists are worked out from the links in the one file, so a
line from a file this one includes, or that includes it, is not counted: add
its name by hand, which loading the configuration says to do.

The comments are kept. A comment above a node moves with the node, and one
above an attribute or a link with that attribute or link. The comment above
the first node of a section that is in paragraphs -- an empty comment line
between them -- is taken to be about the section in all but its last
paragraph, which stays with the node, and the section keeps the rest at its
top. A comment that cannot be kept where it was is said on standard error,
with the file and what became of it.

A simulator file is rewritten the same way. Its simulated payloads are grouped
by type, each named by the word ``payload``, and its groups of settings are
named by the word ``group`` and say no type, a payload that took its type from
its group being written under it::

    simulated_payload_groups:
      - group: zone_temps
        packet_interval: 100 ms

    simulated_payloads:
      - type network:
          - payload: temp_1
            group: zone_temps

In XML, ``<simulated_payloads>`` holds ``<type name="...">`` elements of
``<payload name="...">`` elements, and ``<simulated_payload_groups>`` holds
``<group name="...">`` elements. A physics model's components are grouped by
type, each named by the word ``component``::

    physics:
      - type heater:
          - component: heater_1
            power: "60 W"

Which kind of file it is shows in what it holds: a ``simulated_payloads`` or
``simulated_payload_groups`` section is a simulator file, a ``physics`` list a
physics model, and an XML root ``<simulator>`` a simulator file. Each entry's
settings are written in name order.

A file already in the new shape is left as it is, so running this twice
changes nothing the second time. Every section, and everything else in the
file, is left where it was.

Usage::

    tools/convert-format.py FILE...
    tools/convert-format.py --yaml - < old.ytcs > new.ytcs
    tools/convert-format.py --xml - < old.xtcs > new.xtcs

Each FILE is rewritten in place, YAML or XML by its extension (``.ytcs`` or
``.xtcs`` for a configuration, ``.yaml``, ``.yml`` or ``.xml`` for a simulator
file or a physics model). ``-`` reads standard input and writes standard
output, in the language ``--yaml`` or ``--xml`` names.
"""

import re
import sys
import textwrap
import xml.etree.ElementTree as ElementTree
import xml.parsers.expat
from xml.sax.saxutils import escape, quoteattr

import yaml

#: Each section of nodes as the old shape named it, the name it has now and
#: the word for one of its nodes.
SECTIONS = {"gateway": ("gateways", "gateway"),
            "vehicle": ("vehicle", "compute"),
            "payloads": ("payloads", "payload")}

#: A node's links, by the data source they leave.
LINKS = ("moc-out", "world-out")

#: A node's sinks, each listing the names a line arrives there by.
SINKS = ("moc-in", "world-in")

#: The sink a line leaving by each data source arrives at.
ARRIVES = {"moc-out": "moc-in", "world-out": "world-in"}

#: The attributes a node writes as a list, whose entries are a set:
#: ``tcslib_protocol::graph::LISTED``.
LISTED = ("terminators",)

#: The indentation of each level of the new shape in YAML: the section's
#: entries, a type's nodes, and a node's keys.
YAML_TYPE = 2
YAML_NODE = 6
YAML_KEY = 8

#: The same in XML: the section, its types, their nodes, a node's elements.
XML_SECTION = 4
XML_TYPE = 8
XML_NODE = 12
XML_CHILD = 16

#: The widest a line of a comment is written: a comment moved deeper than it
#: was is filled again where a line of it would be wider.
WIDTH = 80


class Report:
    """What could not be kept where it was, for one file."""

    def __init__(self, name):
        """Begin a report.

        Args:
            name: The file's name, to begin each line with.
        """
        self.name = name
        self.lines = []

    def say(self, what):
        """Note one thing.

        Args:
            what: What became of it.
        """
        self.lines.append(f"{self.name}: {what}")


def kind(key):
    """The place a key of a node has in the order it is written in.

    Args:
        key: The key.

    Returns:
        A tuple that sorts attributes first, by name, then links, then sinks.
    """
    if key in LINKS:
        return (1, key)
    if key in SINKS:
        return (2, key)
    return (0, key)


def comment_text(line):
    """The words of a YAML comment line, without its indentation.

    Args:
        line: The line.

    Returns:
        The line from its ``#`` on.
    """
    return line.strip()


def is_comment(line):
    """Whether a YAML line is a comment and nothing else.

    Args:
        line: The line.

    Returns:
        Whether it is.
    """
    return line.lstrip().startswith("#")


def indentation(line):
    """How far a line is indented.

    Args:
        line: The line.

    Returns:
        The number of spaces before its first other character.
    """
    return len(line) - len(line.lstrip(" "))


def filled(paragraphs, first, rest, end, report):
    """Fill paragraphs of words into lines no wider than :data:`WIDTH`.

    A paragraph whose lines are indented within the comment, as a list or an
    example is, is kept as it is, and said if a line of it is too wide.

    Args:
        paragraphs: Each paragraph, a list of its lines' words, each line's
                    words with their indentation within the comment.
        first:      What the first line of the comment begins with.
        rest:       What every other line begins with.
        end:        What the last line ends with.
        report:     Where to say what cannot be filled.

    Returns:
        The lines, ``None`` standing between paragraphs.
    """
    out = []
    for at, lines in enumerate(paragraphs):
        if at:
            out.append(None)
        kept = any(line.startswith((" ", "-", "*")) or not line
                   for line in lines)
        if kept:
            out.extend(lines)
            if any(len(rest) + len(line) + len(end) > WIDTH for line in lines):
                report.say("a comment set out as a list or an example is "
                           f"wider than {WIDTH} columns where it is now: "
                           f"{lines[0].strip()}")
            continue
        out.extend(textwrap.wrap(" ".join(line.strip() for line in lines),
                                 width=WIDTH - len(rest) - len(end),
                                 break_long_words=False,
                                 break_on_hyphens=False))
    return out


def yaml_comment(lines, indent, report):
    """Write a YAML comment at an indentation, filled again where a line of
    it would be wider than :data:`WIDTH`.

    Args:
        lines:  The comment's lines, each from its ``#`` on.
        indent: How far to indent it.
        report: Where to say what cannot be filled.

    Returns:
        Its lines, indented.
    """
    pad = " " * indent
    if all(len(pad) + len(line) <= WIDTH for line in lines):
        return [pad + line for line in lines]
    paragraphs = [[]]
    for line in lines:
        words = line[1:]
        if not words.strip():
            paragraphs.append([])
            continue
        paragraphs[-1].append(words[1:] if words.startswith(" ") else words)
    paragraphs = [p for p in paragraphs if p]
    out = []
    for line in filled(paragraphs, pad + "# ", pad + "# ", "", report):
        out.append(pad + "#" if line is None else pad + "# " + line)
    return out


def section_split(comments, empty):
    """Split the comment above a section's first node into the part about the
    section and the part about the node.

    Args:
        comments: The comment, one item for each line or piece.
        empty:    Whether one of them is an empty one, which separates
                  paragraphs.

    Returns:
        The part about the section and the part about the node; the first
        empty when the comment is one paragraph.
    """
    breaks = [at for at, one in enumerate(comments) if empty(one)]
    if not breaks:
        return [], comments
    last = breaks[-1]
    return comments[:last], comments[last + 1:]


# ---------------------------------------------------------------- YAML


def yaml_is_old(document):
    """Whether a YAML configuration, as parsed, is in the old shape anywhere.

    Args:
        document: The parsed file.

    Returns:
        Which of its parts are old: some of ``gateway``, ``vehicle``,
        ``payloads`` and ``tcs-gui``.
    """
    old = set()
    if not isinstance(document, dict):
        return old
    for key, value in document.items():
        if key == "gateway":
            old.add(key)
        elif key in SECTIONS and isinstance(value, list):
            if any(isinstance(entry, dict)
                   and ("name" in entry or "type" in entry) for entry in value):
                old.add(key)
        elif key == "tcs-gui" and isinstance(value, dict):
            placed = value.get("nodes") or []
            if (any(isinstance(entry, dict) and "name" in entry
                    for entry in placed)):
                old.add(key)
    return old


def top_blocks(lines, blank_inside=False):
    """Split a YAML file into its top-level keys and what lies between them.

    Args:
        lines:        The file's lines, without their ends.
        blank_inside: Whether an empty line followed by more of a key's
                      indented lines is the key's, as it is in a simulator
                      file or a physics model, whose entries are set apart
                      by empty lines.

    Returns:
        A list of items, each ``("loose", [lines])`` for lines that belong to
        no key, or ``("key", name, [comment lines above it], [its lines])``.
    """
    items = []
    loose = []
    current = None
    held = []
    key = re.compile(r'^(["\']?)([A-Za-z_][\w-]*)\1\s*:')
    for line in lines:
        if blank_inside and current is not None and not line.strip():
            held.append(line)
            continue
        if held:
            if (line.startswith((" ", "-", "\t"))
                    or (is_comment(line) and indentation(line) > 0)):
                current[3].extend(held)
            else:
                items.append(current)
                current = None
                loose.extend(held)
            held = []
        found = key.match(line)
        if found:
            # The comment lines right above the key, with no blank line
            # between, are about it.
            above = []
            while (loose and is_comment(loose[-1])
                   and indentation(loose[-1]) == 0):
                above.insert(0, loose.pop())
            if current is not None:
                items.append(current)
                current = None
            if loose:
                items.append(("loose", loose))
                loose = []
            current = ("key", found.group(2), above, [line])
            continue
        if (current is not None
                and (line.startswith((" ", "-", "\t"))
                     or (is_comment(line) and indentation(line) > 0))):
            current[3].append(line)
            continue
        if current is not None:
            items.append(current)
            current = None
        loose.append(line)
    if current is not None:
        items.append(current)
    loose.extend(held)
    if loose:
        items.append(("loose", loose))
    return items


def split_flow(text):
    """Split the inside of a YAML flow mapping at its top-level commas.

    Args:
        text: What is between the braces.

    Returns:
        Each ``key: value`` piece, trimmed.
    """
    pieces = []
    depth = 0
    quote = None
    start = 0
    for at, char in enumerate(text):
        if quote:
            if char == quote:
                quote = None
            continue
        if char in "\"'":
            quote = char
        elif char in "[{":
            depth += 1
        elif char in "]}":
            depth -= 1
        elif char == "," and depth == 0:
            pieces.append(text[start:at].strip())
            start = at + 1
    pieces.append(text[start:].strip())
    return [piece for piece in pieces if piece]


def key_of(line):
    """The key a YAML line of a mapping begins with.

    Args:
        line: The line, without indentation.

    Returns:
        The key, unquoted, or ``None`` for a line that is not ``key: ...``.
    """
    found = re.match(r'^(["\']?)([^"\':#][^:]*?)\1\s*:(\s|$)', line)
    return found.group(2).strip() if found else None


def destinations_of(value, report, where):
    """The destinations a link says, however it was written.

    Args:
        value:  The link's value, as parsed: a name, a list of names and
                entries, or an entry.
        report: Where to say what cannot be read.
        where:  The node and link, for the report.

    Returns:
        Each destination as ``(target, input)``, ``input`` ``None`` where
        none is named.
    """
    if value is None:
        return []
    if not isinstance(value, list):
        value = [value]
    out = []
    for one in value:
        if isinstance(one, dict):
            out.append((str(one.get("target", "")),
                        None if one.get("input") is None
                        else str(one["input"])))
        else:
            text = str(one)
            node, dot, entry = text.partition(".")
            if dot and node and entry:
                out.append((node, entry))
            else:
                out.append((text, None))
    return sorted(out, key=lambda d: (d[0], d[1] or ""))


def vote_inputs(said):
    """The inputs an old vote's ``inputs`` attribute names.

    Args:
        said: The attribute's text, or ``None``.

    Returns:
        Each name, in the order given.
    """
    if said is None:
        return []
    return [one for one in re.split(r"[,\s]+", str(said)) if one]


def line_to(target, entry, sender):
    """A link's destination as a line is written: the node it reaches and the
    name it arrives by.

    Args:
        target: The node it reaches.
        entry:  The input the old link named, or ``None``.
        sender: The node it leaves, whose name it arrives by otherwise.

    Returns:
        ``target.name``.
    """
    return f"{target}.{entry if entry is not None else sender}"


class Arrivals:
    """The names each node lists under its sinks, from the lines to it."""

    def __init__(self):
        """Nothing arriving anywhere yet."""
        self.names = {}

    def add(self, node, sink, name):
        """Note that a line arrives at a node's sink by a name.

        Args:
            node: The node it reaches.
            sink: ``world-in`` or ``moc-in``.
            name: The name it arrives by.
        """
        theirs = self.names.setdefault(node, {}).setdefault(sink, [])
        if name not in theirs:
            theirs.append(name)

    def link(self, sender, key, destinations):
        """Note the lines one link sends.

        Args:
            sender:       The node it leaves.
            key:          The data source it leaves by.
            destinations: Each as ``(target, input)``.
        """
        for target, entry in destinations:
            self.add(target, ARRIVES[key],
                     entry if entry is not None else sender)

    def of(self, node, report):
        """The names a node lists, under each sink.

        Args:
            node:   The node.
            report: Where to say a name that would be listed twice.

        Returns:
            ``(sink, names)`` for each sink that has any, in the order they
            are written, the names in the order of their text.
        """
        theirs = self.names.get(node, {})
        out = []
        for sink in sorted(SINKS):
            names = sorted(theirs.get(sink, []))
            if names:
                out.append((sink, names))
        both = set(theirs.get("world-in", [])) & set(theirs.get("moc-in", []))
        for name in sorted(both):
            report.say(f"\"{node}\" is reached by \"{name}\" under both its "
                       "sinks, and a name is listed once: rename one, in "
                       "the line and in the list")
        return out


def yaml_arrivals(document):
    """The names each node of a YAML configuration lists, from its links.

    Read from the whole file before any of it is written, since the names a
    node lists are said by the nodes that send to it, wherever they are.

    Args:
        document: The parsed file, in the old shape, the new, or both.

    Returns:
        The :class:`Arrivals`.
    """
    arrivals = Arrivals()
    if not isinstance(document, dict):
        return arrivals
    for section, (now, word) in SECTIONS.items():
        for key in sorted({section, now}):
            value = document.get(key)
            if isinstance(value, dict):
                value = [value]
            if not isinstance(value, list):
                continue
            for entry in value:
                for name, said in yaml_said(entry, word):
                    for link in LINKS:
                        arrivals.link(name, link,
                                      destinations_of(said.get(link), None,
                                                      link))
                    for one in vote_inputs(said.get("inputs")):
                        arrivals.add(name, "world-in", one)
    return arrivals


def yaml_said(entry, word):
    """The nodes one entry of a section says, in either shape.

    Args:
        entry: The entry: an old node, ``{name: ..., type: ...}``, or a type of
               new ones, ``{"type i2c": [{payload: ...}]}``.
        word:  The word the section names a node by in the new shape.

    Returns:
        Each node as ``(name, what it says)``.
    """
    if not isinstance(entry, dict):
        return []
    if "name" in entry:
        return [(str(entry["name"]), entry)]
    out = []
    for key, inside in entry.items():
        if not str(key).startswith("type ") or not isinstance(inside, list):
            continue
        out.extend((str(one[word]), one) for one in inside
                   if isinstance(one, dict) and word in one)
    return out


class YamlNode:
    """One node of the old shape, taken apart."""

    def __init__(self):
        """An empty node."""
        self.name = None
        self.name_text = None
        self.node_type = None
        self.above = []
        #: Each key, the comment lines above it, and its lines, the first
        #: without indentation and the rest indented relative to that.
        self.keys = []


def yaml_entries(body, report, section, need_type=True):
    """Take a section's entries apart.

    Args:
        body:      The section's lines under its key.
        report:    Where to say what cannot be kept.
        section:   The section's old name, for the report.
        need_type: Whether each entry must say its type: see
                   :func:`yaml_node`.

    Returns:
        The nodes, and the comment lines left at the section's end.
    """
    dashes = [indentation(line) for line in body
              if line.lstrip().startswith("- ")]
    if not dashes:
        return [], [line for line in body if is_comment(line)]
    margin = min(dashes)
    # Each entry: its comment lines above, and its own lines.
    entries = []
    pending = []
    for line in body:
        if not line.strip():
            pending = [one for one in pending if is_comment(one)]
            continue
        if indentation(line) == margin and line.lstrip().startswith("- "):
            entries.append((pending, [line]))
            pending = []
            continue
        if is_comment(line):
            pending.append(line)
            continue
        if not entries:
            report.say(f"{section}: a line before its first entry is dropped: "
                       f"{line.strip()}")
            continue
        # A line of the entry: the comments read since its last line were
        # inside it, above a key of its own.
        entries[-1][1].extend(pending)
        pending = []
        entries[-1][1].append(line)
    trailing = pending
    nodes = []
    for above, own in entries:
        nodes.append(yaml_node(above, own, margin, report, section,
                               need_type))
    return nodes, trailing


def yaml_node(above, own, margin, report, section, need_type=True):
    """Take one entry of the old shape apart.

    Args:
        above:     The comment lines above it.
        own:       Its lines, the first beginning with its dash.
        margin:    The indentation of its dash.
        report:    Where to say what cannot be kept.
        section:   The section's old name, for the report.
        need_type: Whether it must say its type: a simulated payload may take
                   its from its group, and a group need say none.

    Returns:
        The :class:`YamlNode`.
    """
    node = YamlNode()
    node.above = [comment_text(line) for line in above]
    first = own[0][margin + 2:]
    column = margin + 2
    if first.lstrip().startswith("{"):
        # A flow mapping, perhaps over several lines: each piece becomes a
        # key of its own.
        text = " ".join([first] + [line.strip() for line in own[1:]])
        inside = text.strip()[1:-1]
        blocks = [([], [piece]) for piece in split_flow(inside)]
    else:
        blocks = []
        comments = []
        rest = [" " * column + first] + own[1:]
        for line in rest:
            if is_comment(line) and (not blocks or indentation(line) <= column):
                comments.append(comment_text(line))
                continue
            if indentation(line) == column and not is_comment(line):
                blocks.append((comments, [line[column:]]))
                comments = []
                continue
            if not blocks:
                report.say(f"{section}: a line of an entry is dropped: "
                           f"{line.strip()}")
                continue
            # A line under the key, a comment inside its value among them:
            # its value goes on, kept as it was relative to the key.
            blocks[-1][1].append(line[column:] if line.startswith(" " * column)
                                 else line.lstrip())
        if comments:
            report.say(f"{section}: the comment at the end of an entry is put "
                       "above it")
            node.above.extend(comments)
    text = "\n".join(line for _, chunk in blocks for line in chunk)
    try:
        parsed = yaml.safe_load(text) or {}
    except yaml.YAMLError as why:
        raise ValueError(f"{section}: an entry does not read: {why}")
    for comments, chunk in blocks:
        key = key_of(chunk[0])
        if key is None:
            raise ValueError(f"{section}: cannot read the key of {chunk[0]!r}")
        if key == "name":
            node.name = str(parsed.get("name"))
            node.name_text = chunk[0].split(":", 1)[1].strip()
            if "#" in node.name_text:
                report.say(f"{section}: the comment beside the name of "
                           f"\"{node.name}\" is put above it")
                words, _, said = node.name_text.partition("#")
                node.name_text = words.strip()
                node.above.append("#" + said)
            node.above.extend(comments)
            continue
        if key == "type":
            node.node_type = str(parsed.get("type"))
            node.above.extend(comments)
            if "#" in chunk[0].split(":", 1)[1]:
                report.say(f"{section}: the comment beside the type of "
                           f"\"{node.name}\" is put above the node")
                node.above.append("#"
                                  + chunk[0].split(":", 1)[1].partition("#")[2])
            continue
        node.keys.append((key, comments, chunk, parsed.get(key)))
    if node.name is None or (need_type and node.node_type is None):
        raise ValueError(f"{section}: an entry has no name or no type: "
                         f"{text!r}")
    return node


def yaml_key_lines(node, key, comments, chunk, value, report, at=YAML_KEY):
    """Write one key of a node in the new shape.

    Args:
        node:     The node.
        key:      The key.
        comments: The comment lines above it.
        chunk:    Its lines as the file had them, the first without
                  indentation.
        value:    Its value, as parsed.
        report:   Where to say what cannot be kept.
        at:       How far to indent it.

    Returns:
        Its lines, indented.
    """
    pad = " " * at
    out = yaml_comment(comments, at, report)
    if key in LINKS:
        inner = [line.strip() for line in chunk[1:] if is_comment(line)]
        if inner or "#" in chunk[0].split(":", 1)[1]:
            report.say(f"the comments inside the {key} of \"{node.name}\" are "
                       "put above it")
            out.extend(yaml_comment(inner, at, report))
        out.append(f"{pad}{key}:")
        for target, entry in destinations_of(value, report, key):
            out.append(f"{pad}  - {line_to(target, entry, node.name)}")
        return out
    out.append(pad + chunk[0])
    rest = chunk[1:]
    # A listed attribute's entries, one to a line, in the order of their
    # text, as the editor writes them: they are a set.
    if key in LISTED and all(line.lstrip().startswith("- ") for line in rest):
        rest = sorted(rest, key=lambda line: line.lstrip())
    out.extend(pad + line if line.strip() else "" for line in rest)
    return out


def yaml_sink_lines(node, arrivals, comments, report):
    """Write the names a node lists under its sinks.

    Args:
        node:     The node.
        arrivals: The names each node lists.
        comments: The comment lines above the sinks and inputs the old shape
                  wrote, which go above these.
        report:   Where to say what cannot be kept.

    Returns:
        Its lines.
    """
    out = []
    lists = arrivals.of(node.name, report)
    if comments:
        if lists:
            out.extend(yaml_comment(comments, YAML_KEY, report))
        else:
            report.say(f"the comments above the sinks of \"{node.name}\", "
                       "which nothing arrives at, are dropped with them")
    for sink, names in lists:
        out.append(f"{' ' * YAML_KEY}{sink}: "
                   f"[{', '.join(plain(name) for name in names)}]")
    return out


def yaml_section(name, above, body, report, arrivals):
    """Write a section of nodes in the new shape.

    Args:
        name:     The section's old name.
        above:    The comment lines above its key.
        body:     Its lines under its key.
        report:   Where to say what cannot be kept.
        arrivals: The names each node lists under its sinks.

    Returns:
        Its lines.
    """
    now, word = SECTIONS[name]
    nodes, trailing = yaml_entries(body, report, name)
    out = list(above)
    out.append(f"{now}:")
    if nodes:
        about, node_part = section_split(nodes[0].above,
                                         lambda c: c.strip() == "#")
        nodes[0].above = node_part
        if about:
            report.say(f"{name}: the first paragraphs of the comment above "
                       f"\"{nodes[0].name}\" are taken to be about the "
                       "section, and kept at its top")
        out.extend(yaml_comment(about, YAML_TYPE, report))
    by_type = {}
    for node in nodes:
        by_type.setdefault(node.node_type, []).append(node)
    for node_type in sorted(by_type):
        out.append(f"{' ' * YAML_TYPE}- type {node_type}:")
        for node in sorted(by_type[node_type], key=lambda n: n.name):
            out.extend(yaml_comment(node.above, YAML_NODE, report))
            out.append(f"{' ' * YAML_NODE}- {word}: {node.name_text}")
            # The old sinks and a vote's inputs are written again as the
            # names that arrive, after everything else.
            moved = [k for k in node.keys if k[0] in SINKS or k[0] == "inputs"]
            kept = [k for k in node.keys if k not in moved]
            for key, comments, chunk, value in sorted(kept,
                                                      key=lambda k: kind(k[0])):
                out.extend(yaml_key_lines(node, key, comments, chunk, value,
                                          report))
            out.extend(yaml_sink_lines(node, arrivals,
                                       [c for k in moved for c in k[1]],
                                       report))
    if trailing:
        report.say(f"{name}: the comment at the end of the section is kept at "
                   "its end")
        out.extend(yaml_comment([comment_text(c) for c in trailing], YAML_TYPE,
                                report))
    return out


def plain(text):
    """A name as YAML writes it in a flow mapping: bare where that reads back
    as the same text, quoted where it does not.

    Args:
        text: The name.

    Returns:
        The name, quoted if it must be.
    """
    if (re.fullmatch(r"[A-Za-z_][\w./-]*", text)
            and yaml.safe_load(text) == text):
        return text
    return '"' + text.replace("\\", "\\\\").replace('"', '\\"') + '"'


def yaml_tcs_gui(above, body, report):
    """Write the editor's section in the new shape.

    Args:
        above:  The comment lines above its key.
        body:   Its lines under its key.
        report: Where to say what cannot be kept.

    Returns:
        Its lines.
    """
    said = yaml.safe_load("\n".join(["tcs-gui:"] + body))["tcs-gui"] or {}
    if any(is_comment(line) for line in body):
        report.say("tcs-gui: the comments inside the editor's section are "
                   "dropped: the editor writes the section again from what it "
                   "holds")
    unknown = set(said) - {"nodes", "set-aside"}
    if unknown:
        raise ValueError(f"tcs-gui: does not know {sorted(unknown)}")
    out = list(above)
    out.append("tcs-gui:")
    placed = said.get("nodes") or []
    if placed:
        out.append("  nodes:")
        for entry in sorted(placed,
                            key=lambda e: str(e.get("name", e.get("node")))):
            name = entry.get("name", entry.get("node"))
            out.append(f"    - {{ node: {plain(str(name))}, x: {entry['x']}, "
                       f"y: {entry['y']} }}")
    aside = said.get("set-aside") or []
    if aside:
        out.append("  set-aside:")
        for entry in sorted(aside,
                            key=lambda e: (str(e["node"]), str(e["concern"]))):
            out.append(f"    - {{ node: {plain(str(entry['node']))}, concern: "
                       f"{plain(str(entry['concern']))} }}")
    return out


def convert_yaml(text, report):
    """Rewrite a YAML configuration in the new shape.

    Args:
        text:   The file's text.
        report: Where to say what cannot be kept.

    Returns:
        The new text; the same text for a file already in the new shape.
    """
    document = yaml.safe_load(text)
    old = yaml_is_old(document)
    if not old:
        return text
    arrivals = yaml_arrivals(document)
    lines = text.split("\n")
    out = []
    for item in top_blocks(lines):
        if item[0] == "loose":
            out.extend(item[1])
            continue
        _, key, above, own = item
        if key in old and key in SECTIONS:
            out.extend(yaml_section(key, above, own[1:], report,
                                    arrivals))
        elif key in old and key == "tcs-gui":
            # Its blank lines after are not its own.
            body = own[1:]
            tail = []
            while body and not body[-1].strip():
                tail.insert(0, body.pop())
            out.extend(yaml_tcs_gui(above, body, report))
            out.extend(tail)
        else:
            out.extend(above + own)
    new = "\n".join(out)
    return new


# ---------------------------------------------------------------- XML


class Top:
    """One child of an XML file's root element, and where it is."""

    def __init__(self, what, tag, start):
        """Begin one.

        Args:
            what:  ``element`` or ``comment``.
            tag:   The element's name, or ``None`` for a comment.
            start: Its offset in the text.
        """
        self.what = what
        self.tag = tag
        self.start = start
        self.end = None


def xml_tops(text):
    """Find the children of an XML file's root element.

    Args:
        text: The file's text.

    Returns:
        Each child, element or comment, with where it starts and ends.
    """
    data = text.encode("utf-8")
    parser = xml.parsers.expat.ParserCreate()
    tops = []
    depth = [0]

    def start(name, attributes):
        """Note an element beginning.

        Args:
            name:       Its name.
            attributes: Its attributes, which are not needed.
        """
        if depth[0] == 1:
            tops.append(Top("element", name, parser.CurrentByteIndex))
        depth[0] += 1

    def end(name):
        """Note an element ending.

        Args:
            name: Its name, which is not needed.
        """
        depth[0] -= 1
        if depth[0] == 1:
            at = parser.CurrentByteIndex
            close = data.index(b">", at) + 1
            # An empty element ends where it began.
            tops[-1].end = close

    def comment(said):
        """Note a comment.

        Args:
            said: Its text, which is not needed.
        """
        if depth[0] == 1:
            top = Top("comment", None, parser.CurrentByteIndex)
            top.end = data.index(b"-->", top.start) + 3
            tops.append(top)

    parser.StartElementHandler = start
    parser.EndElementHandler = end
    parser.CommentHandler = comment
    parser.Parse(data, True)
    # Offsets are of bytes; the text is sliced by them as bytes.
    return tops, data


def xml_children(fragment):
    """Parse one element, keeping its comments.

    Args:
        fragment: The element's text.

    Returns:
        The element.
    """
    builder = ElementTree.TreeBuilder(insert_comments=True)
    parser = ElementTree.XMLParser(target=builder)
    parser.feed(fragment)
    return parser.close()


def is_xml_comment(item):
    """Whether a parsed child is a comment.

    Args:
        item: The child.

    Returns:
        Whether it is.
    """
    return item.tag is ElementTree.Comment


def text_of(element):
    """An element's text, trimmed.

    Args:
        element: The element.

    Returns:
        Its text.
    """
    return (element.text or "").strip()


class XmlNode:
    """One node of the old shape, taken apart."""

    def __init__(self):
        """An empty node."""
        self.name = None
        self.node_type = None
        self.section = None
        #: The comments above it, each as the file wrote it with the
        #: column it began at.
        self.above = []
        #: Each child but its name and type: its key, its comments, and the
        #: element.
        self.children = []


def reindent(comment, column, to):
    """A comment moved from one column to another, its continuation lines
    moved with it.

    Args:
        comment: The comment's text, ``<!--`` to ``-->``.
        column:  The column it began at.
        to:      The column it is to begin at.

    Returns:
        Its lines, the first not yet indented.
    """
    lines = comment.split("\n")
    shift = to - column
    moved = [lines[0]]
    for line in lines[1:]:
        if shift >= 0:
            moved.append(" " * shift + line)
        else:
            cut = min(-shift, indentation(line))
            moved.append(line[cut:])
    return moved


def comment_lines(comment, column, to, report):
    """Write a comment at a column, filled again where a line of it would be
    wider than :data:`WIDTH`.

    Args:
        comment: The comment's text, ``<!--`` to ``-->``.
        column:  The column it began at.
        to:      The column to write it at.
        report:  Where to say what cannot be filled.

    Returns:
        Its lines, indented.
    """
    lines = reindent(comment, column, to)
    lines = [" " * to + lines[0]] + lines[1:]
    if all(len(line) <= WIDTH for line in lines):
        return lines
    inside = comment[4:-3]
    paragraphs = [[]]
    for line in inside.split("\n"):
        if not line.strip():
            paragraphs.append([])
            continue
        paragraphs[-1].append(line.strip())
    paragraphs = [p for p in paragraphs if p]
    pad = " " * to
    words = filled(paragraphs, pad + "<!-- ", pad + "     ", " -->", report)
    out = []
    for at, line in enumerate(words):
        lead = pad + ("<!-- " if at == 0 else "     ")
        out.append("" if line is None else lead + line)
    out[-1] += " -->"
    return out


def comments_at(comments, to, report):
    """Write the comments above something at a column.

    A run of comments of one line each, as a writer that breaks a comment
    into lines writes them, is one comment set out in lines, and is filled
    again as one where a line of it would be wider than :data:`WIDTH`; an
    empty one is a break between paragraphs.

    Args:
        comments: Each comment, ``(text, column)``.
        to:       The column to write them at.
        report:   Where to say what cannot be filled.

    Returns:
        Their lines, indented.
    """
    pad = " " * to
    if comments and all("\n" not in text for text, _ in comments):
        lines = [pad + text for text, _ in comments]
        if all(len(line) <= WIDTH for line in lines):
            return lines
        paragraphs = [[]]
        for text, _ in comments:
            words = text[4:-3].strip()
            if not words:
                paragraphs.append([])
                continue
            paragraphs[-1].append(words)
        paragraphs = [p for p in paragraphs if p]
        out = []
        for line in filled(paragraphs, pad + "<!-- ", pad + "<!-- ", " -->",
                           report):
            out.append(pad
                       + ("<!-- -->" if line is None else f"<!-- {line} -->"))
        return out
    out = []
    for text, column in comments:
        out.extend(comment_lines(text, column, to, report))
    return out


def xml_node(top_text, top_column, above, report, need_type=True):
    """Take one node element of the old shape apart.

    Args:
        top_text:   The element's text.
        top_column: The column it began at.
        above:      The comments above it, each ``(text, column)``.
        report:     Where to say what cannot be kept.
        need_type:  Whether it must say its type: see :func:`yaml_node`.

    Returns:
        The :class:`XmlNode`.
    """
    element = xml_children(top_text)
    node = XmlNode()
    node.section = element.tag
    node.above = list(above)
    pending = []
    for child in element:
        if is_xml_comment(child):
            pending.append((f"<!--{child.text}-->", top_column + 4))
            continue
        if child.tag == "name":
            node.name = text_of(child)
            node.above.extend(pending)
        elif child.tag == "type":
            node.node_type = text_of(child)
            node.above.extend(pending)
        else:
            node.children.append((child.tag, pending, child))
        pending = []
    if pending:
        report.say(f"the comment at the end of \"{node.name}\" is put at the "
                   "end of it")
        node.children.append((None, pending, None))
    if node.name is None or (need_type and node.node_type is None):
        raise ValueError(f"a <{node.section}> has no <name> or no <type>")
    return node


def xml_destination(child):
    """The destination a link element says.

    Args:
        child: The element: ``<world-out><target>k_1</target>
               </world-out>``, perhaps with an ``<input>``, or
               ``<world-out>k_1</world-out>``.

    Returns:
        ``(target, input)``.
    """
    target = child.find("target")
    if target is None:
        text = text_of(child)
        node, dot, entry = text.partition(".")
        if dot and node and entry:
            return (node, entry)
        return (text, None)
    entry = child.find("input")
    return (text_of(target), None if entry is None else text_of(entry))


def xml_arrivals(nodes):
    """The names each node of an XML configuration lists, from its links.

    Args:
        nodes: Every node of the old shape the file has.

    Returns:
        The :class:`Arrivals`.
    """
    arrivals = Arrivals()
    for node in nodes:
        for key, _, child in node.children:
            if key in LINKS:
                arrivals.link(node.name, key, [xml_destination(child)])
            elif key == "inputs":
                for one in vote_inputs(text_of(child)):
                    arrivals.add(node.name, "world-in", one)
    return arrivals


def xml_node_lines(node, word, report, at=XML_NODE, arrivals=None):
    """Write one node in the new shape.

    Args:
        node:     The node.
        word:     The element its section writes a node as.
        report:   Where to say what cannot be kept.
        at:       The column it is written at, its children four further in.
        arrivals: The names each node lists under its sinks, for a node of a
                  configuration; ``None`` for one of a simulator file, which
                  has no links.

    Returns:
        Its lines.
    """
    child_at = at + 4
    out = comments_at(node.above, at, report)
    pad = " " * child_at
    keyed = [c for c in node.children if c[0] is not None]
    loose = [c for c in node.children if c[0] is None]
    # The old sinks and a vote's inputs are written again as the names that
    # arrive, after everything else.
    moved = []
    lists = []
    if arrivals is not None:
        moved = [c for c in keyed if c[0] in SINKS or c[0] == "inputs"]
        keyed = [c for c in keyed if c not in moved]
        lists = arrivals.of(node.name, report)
    if not keyed and not loose and not lists:
        out.append(f"{' ' * at}<{word} name={quoteattr(node.name)}/>")
        return out
    out.append(f"{' ' * at}<{word} name={quoteattr(node.name)}>")
    # Each attribute in name order; then the links, each destination in
    # order; then the sinks.
    links = {}
    # A listed attribute's repeats in the order of their text, as the editor
    # writes them: they are a set.
    for key, comments, child in sorted(keyed,
                                       key=lambda c: kind(c[0])
                                       + ((text_of(c[2]),) if c[0] in LISTED
                                          else ())):
        if key in LINKS:
            entry = links.setdefault(key, ([], []))
            entry[0].extend(comments)
            entry[1].append(xml_destination(child))
            continue
        if key in SINKS and links:
            for link in sorted(links):
                out.extend(xml_link_lines(link, *links[link], node.name,
                                          report))
            links = {}
        out.extend(comments_at(comments, child_at, report))
        if len(child):
            raise ValueError(f"\"{node.name}\": <{key}> holds elements, which "
                             "an attribute does not")
        out.append(f"{pad}<{key}>{escape(text_of(child))}</{key}>")
    for link in sorted(links):
        out.extend(xml_link_lines(link, *links[link], node.name, report))
    above = [c for _, comments, _ in moved for c in comments]
    if above and not lists:
        report.say(f"the comments above the sinks of \"{node.name}\", which "
                   "nothing arrives at, are dropped with them")
    if lists:
        out.extend(comments_at(above, child_at, report))
    for sink, names in lists:
        out.extend(f"{pad}<{sink}>{escape(name)}</{sink}>" for name in names)
    for _, comments, _ in loose:
        out.extend(comments_at(comments, child_at, report))
    out.append(f"{' ' * at}</{word}>")
    return out


def xml_link_lines(key, comments, destinations, sender, report):
    """Write one link of a node, each line an element of its own.

    Args:
        key:          The data source it leaves by.
        comments:     The comments above it.
        destinations: Each destination as ``(target, input)``.
        sender:       The node it leaves.
        report:       Where to say what cannot be filled.

    Returns:
        Its lines.
    """
    out = comments_at(comments, XML_CHILD, report)
    for target, entry in sorted(destinations, key=lambda d: (d[0], d[1] or "")):
        said = escape(line_to(target, entry, sender))
        out.append(f"{' ' * XML_CHILD}<{key}>{said}</{key}>")
    return out


def xml_section(name, nodes, about, report, arrivals):
    """Write a section of nodes in the new shape.

    Args:
        name:     The section's old name.
        nodes:    Its nodes.
        about:    The comments at its top, each ``(text, column)``.
        report:   Where to say what cannot be kept.
        arrivals: The names each node lists under its sinks.

    Returns:
        Its lines.
    """
    now, word = SECTIONS[name]
    out = [f"{' ' * XML_SECTION}<{now}>"]
    out.extend(comments_at(about, XML_TYPE, report))
    by_type = {}
    for node in nodes:
        by_type.setdefault(node.node_type, []).append(node)
    for node_type in sorted(by_type):
        out.append(f"{' ' * XML_TYPE}<type name={quoteattr(node_type)}>")
        for node in sorted(by_type[node_type], key=lambda n: n.name):
            out.extend(xml_node_lines(node, word, report,
                                      arrivals=arrivals))
        out.append(f"{' ' * XML_TYPE}</type>")
    out.append(f"{' ' * XML_SECTION}</{now}>")
    return out


def xml_tcs_gui(fragment, report):
    """Write the editor's section in the new shape.

    Args:
        fragment: The section's text.
        report:   Where to say what cannot be kept.

    Returns:
        Its lines, or ``None`` for a section already in the new shape.
    """
    element = xml_children(fragment)
    placed = []
    aside = []
    old = False
    for child in element:
        if is_xml_comment(child):
            report.say("tcs-gui: a comment inside the editor's section is "
                       "dropped: the editor writes the section again from what "
                       "it holds")
            continue
        if child.tag == "nodes" and child.find("name") is not None:
            old = True
            placed.append((text_of(child.find("name")),
                           text_of(child.find("x")), text_of(child.find("y"))))
        elif child.tag == "nodes":
            for one in child.findall("node"):
                placed.append((one.get("name"), one.get("x"), one.get("y")))
        elif child.tag == "set-aside" and child.find("concern") is not None:
            old = True
            aside.append((text_of(child.find("node")),
                          text_of(child.find("concern"))))
        elif child.tag == "set-aside":
            for one in child.findall("node"):
                aside.append((one.get("name"), one.get("concern")))
        else:
            raise ValueError(f"tcs-gui: does not know <{child.tag}>")
    if not old:
        return None
    pad = " " * XML_SECTION
    out = [f"{pad}<tcs-gui>"]
    if placed:
        out.append(f"{pad}    <nodes>")
        for name, x, y in sorted(placed):
            out.append(f"{pad}        <node name={quoteattr(name)} "
                       f"x={quoteattr(x)} y={quoteattr(y)}/>")
        out.append(f"{pad}    </nodes>")
    if aside:
        out.append(f"{pad}    <set-aside>")
        for name, concern in sorted(aside):
            out.append(f"{pad}        <node name={quoteattr(name)} "
                       f"concern={quoteattr(concern)}/>")
        out.append(f"{pad}    </set-aside>")
    out.append(f"{pad}</tcs-gui>")
    return out


def line_span(data, start, end):
    """Widen a span to the whole lines it is on, where nothing else is on
    them.

    Args:
        data:  The file, as bytes.
        start: Where the span starts.
        end:   Where it ends.

    Returns:
        The widened start and end.
    """
    begin = data.rfind(b"\n", 0, start) + 1
    if data[begin:start].strip() == b"":
        start = begin
    after = data.find(b"\n", end)
    if after == -1:
        after = len(data)
    if data[end:after].strip() == b"":
        end = min(after + 1, len(data))
    return start, end


def column_of(data, at):
    """The column a byte offset is at.

    Args:
        data: The file, as bytes.
        at:   The offset.

    Returns:
        Its column, from nought.
    """
    return at - (data.rfind(b"\n", 0, at) + 1)


def convert_xml(text, report):
    """Rewrite an XML configuration in the new shape.

    Args:
        text:   The file's text.
        report: Where to say what cannot be kept.

    Returns:
        The new text; the same text for a file already in the new shape.
    """
    tops, data = xml_tops(text)
    edits = []
    nodes = {name: [] for name in SECTIONS}
    first = {}
    for at, top in enumerate(tops):
        if top.what != "element":
            continue
        fragment = data[top.start:top.end].decode("utf-8")
        if top.tag == "tcs-gui":
            lines = xml_tcs_gui(fragment, report)
            if lines is not None:
                start, end = line_span(data, top.start, top.end)
                edits.append((start, end, "\n".join(lines) + "\n"))
            continue
        if top.tag not in SECTIONS:
            continue
        element = xml_children(fragment)
        if element.find("name") is None and top.tag != "gateway":
            continue
        # The comments right above it, with nothing but one line end between
        # each and the next, are about it.
        above = []
        begin = top.start
        back = at - 1
        while back >= 0 and tops[back].what == "comment":
            between = data[tops[back].end:begin]
            if between.strip() or between.count(b"\n") > 1:
                break
            above.insert(0, tops[back])
            begin = tops[back].start
            back -= 1
        said = [(data[c.start:c.end].decode("utf-8"), column_of(data, c.start))
                for c in above]
        node = xml_node(fragment, column_of(data, top.start), said, report)
        nodes[top.tag].append(node)
        start, end = line_span(data, begin, top.end)
        if top.tag not in first:
            first[top.tag] = start
            edits.append((start, end, None))
        else:
            edits.append((start, end, ""))
    if not edits:
        return text
    arrivals = xml_arrivals([node for theirs in nodes.values()
                             for node in theirs])
    sections = {}
    for name, theirs in nodes.items():
        if not theirs:
            continue
        # The first node of the section in the file: the part of its comment
        # before an empty comment is about the section.
        about, node_part = section_split(theirs[0].above,
                                         lambda c: c[0][4:-3].strip() == "")
        theirs[0].above = node_part
        if about:
            report.say(f"{name}: the first paragraphs of the comment above "
                       f"\"{theirs[0].name}\" are taken to be about the "
                       "section, and kept at its top")
        sections[name] = ("\n".join(xml_section(name, theirs, about, report,
                                                 arrivals))
                          + "\n")
    out = bytearray()
    done = 0
    for start, end, replacement in sorted(edits):
        out += data[done:start]
        if replacement is None:
            replacement = "".join(sections[name] for name in SECTIONS
                                  if first.get(name) == start)
        out += replacement.encode("utf-8")
        done = end
    out += data[done:]
    return out.decode("utf-8")


# ------------------------------------------------- simulator and physics


#: The sections of a simulator file whose entries are grouped by type now,
#: and the word for one of their entries.
SIM_PAYLOADS = ("simulated_payloads", "payload")
SIM_GROUPS = ("simulated_payload_groups", "group")

#: The section of a physics model, and the word for one of its components.
PHYSICS = ("physics", "component")

#: The indentation of a group's keys in YAML, under ``- group: <name>``.
YAML_GROUP_KEY = 4


def kind_of_file(document):
    """Which kind of file a YAML document is, by what it holds.

    Args:
        document: The parsed file.

    Returns:
        ``simulator``, ``physics`` or ``configuration``.
    """
    if not isinstance(document, dict):
        return "configuration"
    if SIM_PAYLOADS[0] in document or SIM_GROUPS[0] in document:
        return "simulator"
    if isinstance(document.get(PHYSICS[0]), list):
        return "physics"
    return "configuration"


def old_entries(document, section):
    """Whether a section of a simulator file or physics model, as parsed,
    holds an entry of the old shape.

    Args:
        document: The parsed file.
        section:  The section's key.

    Returns:
        Whether one of its entries has a ``name`` or a ``type`` of its own.
    """
    value = document.get(section) if isinstance(document, dict) else None
    return (isinstance(value, list)
            and any(isinstance(entry, dict)
                    and ("name" in entry or "type" in entry)
                    for entry in value))


def group_types(document):
    """The type each group of an old simulator file says, if it says one.

    Args:
        document: The parsed file.

    Returns:
        Each group's type, by its name.
    """
    out = {}
    for entry in document.get(SIM_GROUPS[0]) or []:
        if isinstance(entry, dict) and "name" in entry and "type" in entry:
            out[str(entry["name"])] = str(entry["type"])
    return out


def yaml_typed(section, word, above, body, report, inherited=None):
    """Write a section of a simulator file or physics model grouped by type.

    Args:
        section:   The section's key.
        word:      The word an entry is named by.
        above:     The comment lines above the section's key.
        body:      Its lines under its key.
        report:    Where to say what cannot be kept.
        inherited: For simulated payloads, each group's type by its name, for
                   a payload that took its type from its group.

    Returns:
        Its lines.
    """
    nodes, trailing = yaml_entries(body, report, section,
                                   need_type=inherited is None)
    for node in nodes:
        if node.node_type is not None:
            continue
        group = next((value for key, _, _, value in node.keys
                      if key == "group"), None)
        node.node_type = (inherited or {}).get(str(group))
        if node.node_type is None:
            raise ValueError(f"{section}: \"{node.name}\" says no type, and "
                             "nor does a group it names")
    out = list(above)
    out.append(f"{section}:")
    if nodes:
        about, node_part = section_split(nodes[0].above,
                                         lambda c: c.strip() == "#")
        nodes[0].above = node_part
        if about:
            report.say(f"{section}: the first paragraphs of the comment above "
                       f"\"{nodes[0].name}\" are taken to be about the "
                       "section, and kept at its top")
        out.extend(yaml_comment(about, YAML_TYPE, report))
    # Its entries set apart by empty lines, where the file set them apart.
    spaced = any(not line.strip() for line in body)
    by_type = {}
    for node in nodes:
        by_type.setdefault(node.node_type, []).append(node)
    for count, node_type in enumerate(sorted(by_type)):
        if spaced and count:
            out.append("")
        out.append(f"{' ' * YAML_TYPE}- type {node_type}:")
        for at, node in enumerate(sorted(by_type[node_type],
                                         key=lambda n: n.name)):
            if spaced and at:
                out.append("")
            out.extend(yaml_comment(node.above, YAML_NODE, report))
            out.append(f"{' ' * YAML_NODE}- {word}: {node.name_text}")
            for key, comments, chunk, value in sorted(node.keys,
                                                      key=lambda k: k[0]):
                out.extend(yaml_key_lines(node, key, comments, chunk, value,
                                          report))
    if trailing:
        report.say(f"{section}: the comment at the end of the section is kept "
                   "at its end")
        out.extend(yaml_comment([comment_text(c) for c in trailing], YAML_TYPE,
                                report))
    return out


def yaml_groups(above, body, report):
    """Write a simulator file's groups, each named by the word ``group`` and
    saying no type.

    Args:
        above:  The comment lines above the section's key.
        body:   Its lines under its key.
        report: Where to say what cannot be kept.

    Returns:
        Its lines.
    """
    section, word = SIM_GROUPS
    nodes, trailing = yaml_entries(body, report, section, need_type=False)
    out = list(above)
    out.append(f"{section}:")
    if nodes:
        about, node_part = section_split(nodes[0].above,
                                         lambda c: c.strip() == "#")
        nodes[0].above = node_part
        out.extend(yaml_comment(about, YAML_TYPE, report))
        if about:
            out.append(f"{' ' * YAML_TYPE}#")
    spaced = any(not line.strip() for line in body)
    for at, node in enumerate(sorted(nodes, key=lambda n: n.name)):
        if spaced and at:
            out.append("")
        if node.node_type is not None:
            report.say(f"{section}: the type of \"{node.name}\" is dropped: "
                       "each payload in it is written under that type")
        out.extend(yaml_comment(node.above, YAML_TYPE, report))
        out.append(f"{' ' * YAML_TYPE}- {word}: {node.name_text}")
        for key, comments, chunk, value in sorted(node.keys,
                                                  key=lambda k: k[0]):
            out.extend(yaml_key_lines(node, key, comments, chunk, value,
                                      report, at=YAML_GROUP_KEY))
    if trailing:
        out.extend(yaml_comment([comment_text(c) for c in trailing], YAML_TYPE,
                                report))
    return out


def convert_sim_yaml(text, document, report):
    """Rewrite a YAML simulator file or physics model in the new shape.

    Args:
        text:     The file's text.
        document: The file, as parsed.
        report:   Where to say what cannot be kept.

    Returns:
        The new text; the same text for a file already in the new shape.
    """
    kind = kind_of_file(document)
    sections = [SIM_PAYLOADS[0], SIM_GROUPS[0]] if kind == "simulator" \
        else [PHYSICS[0]]
    old = [s for s in sections if old_entries(document, s)]
    if not old:
        return text
    inherited = group_types(document)
    out = []
    for item in top_blocks(text.split("\n"), blank_inside=True):
        if item[0] == "loose":
            out.extend(item[1])
            continue
        _, key, above, own = item
        # Its blank lines after are not its own.
        body = own[1:]
        tail = []
        while body and not body[-1].strip():
            tail.insert(0, body.pop())
        if key not in old:
            out.extend(above + own)
            continue
        if key == SIM_GROUPS[0]:
            out.extend(yaml_groups(above, body, report))
        elif key == SIM_PAYLOADS[0]:
            out.extend(yaml_typed(key, SIM_PAYLOADS[1], above, body, report,
                                  inherited))
        else:
            out.extend(yaml_typed(key, PHYSICS[1], above, body, report))
        out.extend(tail)
    return "\n".join(out)


def convert_sim_xml(text, report):
    """Rewrite an XML simulator file in the new shape.

    Each old ``<simulated_payloads>`` element, one for each payload, becomes a
    ``<payload name="...">`` under its ``<type name="...">`` in one
    ``<simulated_payloads>``, and each old ``<simulated_payload_groups>`` a
    ``<group name="...">`` in one ``<simulated_payload_groups>``. Each takes
    the place of the first of its kind in the file.

    Args:
        text:   The file's text.
        report: Where to say what cannot be kept.

    Returns:
        The new text; the same text for a file already in the new shape.
    """
    tops, data = xml_tops(text)
    edits = []
    found = {SIM_PAYLOADS[0]: [], SIM_GROUPS[0]: []}
    first = {}
    for at, top in enumerate(tops):
        if top.what != "element" or top.tag not in found:
            continue
        fragment = data[top.start:top.end].decode("utf-8")
        if xml_children(fragment).find("name") is None:
            continue
        above = []
        begin = top.start
        back = at - 1
        while back >= 0 and tops[back].what == "comment":
            between = data[tops[back].end:begin]
            if between.strip() or between.count(b"\n") > 1:
                break
            above.insert(0, tops[back])
            begin = tops[back].start
            back -= 1
        said = [(data[c.start:c.end].decode("utf-8"), column_of(data, c.start))
                for c in above]
        node = xml_node(fragment, column_of(data, top.start), said, report,
                        need_type=False)
        found[top.tag].append(node)
        start, end = line_span(data, begin, top.end)
        if top.tag not in first:
            first[top.tag] = start
            edits.append((start, end, None))
        else:
            edits.append((start, end, ""))
    if not edits:
        return text
    inherited = {n.name: n.node_type for n in found[SIM_GROUPS[0]]
                 if n.node_type is not None}
    sections = {}
    groups = sorted(found[SIM_GROUPS[0]], key=lambda n: n.name)
    if groups:
        out = [f"{' ' * XML_SECTION}<{SIM_GROUPS[0]}>"]
        for node in groups:
            if node.node_type is not None:
                report.say(f"{SIM_GROUPS[0]}: the type of \"{node.name}\" is "
                           "dropped: each payload in it is written under that "
                           "type")
            out.extend(xml_node_lines(node, SIM_GROUPS[1], report,
                                      at=XML_TYPE))
        out.append(f"{' ' * XML_SECTION}</{SIM_GROUPS[0]}>")
        sections[SIM_GROUPS[0]] = "\n".join(out) + "\n"
    payloads = found[SIM_PAYLOADS[0]]
    if payloads:
        by_type = {}
        for node in payloads:
            if node.node_type is None:
                group = next((text_of(child) for key, _, child
                              in node.children if key == "group"), None)
                node.node_type = inherited.get(group)
            if node.node_type is None:
                raise ValueError(f"{SIM_PAYLOADS[0]}: \"{node.name}\" says no "
                                 "type, and nor does a group it names")
            by_type.setdefault(node.node_type, []).append(node)
        out = [f"{' ' * XML_SECTION}<{SIM_PAYLOADS[0]}>"]
        for node_type in sorted(by_type):
            out.append(f"{' ' * XML_TYPE}<type name={quoteattr(node_type)}>")
            for node in sorted(by_type[node_type], key=lambda n: n.name):
                out.extend(xml_node_lines(node, SIM_PAYLOADS[1], report))
            out.append(f"{' ' * XML_TYPE}</type>")
        out.append(f"{' ' * XML_SECTION}</{SIM_PAYLOADS[0]}>")
        sections[SIM_PAYLOADS[0]] = "\n".join(out) + "\n"
    out = bytearray()
    done = 0
    for start, end, replacement in sorted(edits):
        out += data[done:start]
        if replacement is None:
            replacement = "".join(sections[name] for name in sections
                                  if first.get(name) == start)
        out += replacement.encode("utf-8")
        done = end
    out += data[done:]
    return out.decode("utf-8")


def xml_root(text):
    """The name of an XML file's root element.

    Args:
        text: The file's text.

    Returns:
        The name.
    """
    return ElementTree.fromstring(text.encode("utf-8")).tag


# ---------------------------------------------------------------- main


def convert(text, language, name):
    """Rewrite a configuration in the new shape.

    Args:
        text:     The file's text.
        language: ``yaml`` or ``xml``.
        name:     The file's name, for the report.

    Returns:
        The new text, and the report.
    """
    report = Report(name)
    if language == "yaml":
        document = yaml.safe_load(text)
        if kind_of_file(document) != "configuration":
            return convert_sim_yaml(text, document, report), report
        return convert_yaml(text, report), report
    if xml_root(text) == "simulator":
        return convert_sim_xml(text, report), report
    return convert_xml(text, report), report


def main(argv):
    """Rewrite each file named, or standard input.

    Args:
        argv: The arguments, without the program's name.

    Returns:
        The exit status.
    """
    language = None
    files = []
    for arg in argv:
        if arg in ("--yaml", "--xml"):
            language = arg[2:]
        elif arg in ("-h", "--help"):
            print(__doc__)
            return 0
        else:
            files.append(arg)
    if not files:
        print("usage: convert-format.py FILE... | --yaml - | --xml -",
              file=sys.stderr)
        return 2
    status = 0
    for path in files:
        if path == "-":
            if language is None:
                print("convert-format.py: - needs --yaml or --xml",
                      file=sys.stderr)
                return 2
            new, report = convert(sys.stdin.read(), language, "-")
            sys.stdout.write(new)
        else:
            if path.endswith((".ytcs", ".yaml", ".yml")):
                said = "yaml"
            elif path.endswith((".xtcs", ".xml")):
                said = "xml"
            else:
                print(f"{path}: a configuration is named .ytcs or .xtcs, and "
                      "a simulator file or physics model .yaml, .yml or .xml",
                      file=sys.stderr)
                status = 1
                continue
            with open(path, encoding="utf-8") as text:
                old = text.read()
            try:
                new, report = convert(old, said, path)
            except (ValueError, yaml.YAMLError,
                    xml.parsers.expat.ExpatError) as why:
                print(f"{path}: {why}", file=sys.stderr)
                status = 1
                continue
            if new != old:
                with open(path, "w", encoding="utf-8") as text:
                    text.write(new)
        for line in report.lines:
            print(line, file=sys.stderr)
    return status


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

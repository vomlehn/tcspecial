"""Draws the message-flow diagrams of `In-flight data across a restart`.

Each diagram is a list of messages and notes, written below, from which an
SVG is made, so that a change to what happens is a change to a list rather
than to coordinates. Run from ``docs/``: ``python3 restart_flows.py``, which
``make`` does.

The diagrams must read without colour. A message's configuration is said
three ways at once: a tag in its label (``[A]``, ``[B]``, ``[any]``), a line
style, and a colour from the Okabe-Ito palette, which stays distinct for
the common kinds of colour blindness. What becomes of a message is written
in words at the end it reaches; the marker there only repeats it.
"""

from html import escape
import textwrap

# What each configuration tag looks like: colour, dash pattern and the words
# the legend gives it. ``any`` is the beacon and the three commands whose
# layout never changes, so they read the same under every configuration.
STYLES = {
    "A": ("#0072B2", "", "made with configuration A: solid line"),
    "B": ("#D55E00", "10 5", "made with configuration B: dashed line"),
    "C": ("#CC79A7", "10 4 2 4", "made with configuration C: dash-dot line"),
    "any": ("#555555", "2 4", "layout never changes (beacon, CONNECT, "
            "RESTART_ARM, RESTART, their answers): dotted line"),
}

WIDTH = 940
VEHICLE_X = 300
MOC_X = 640
ROW = 30
TOP = 150
FONT = "font-family: Helvetica, Arial, sans-serif"
OUTCOME_CHARS = 36
# Where along a message its label goes, from the sender: short of the
# middle, so that two messages crossing there keep their labels apart.
LABEL_AT = 0.32


def msg(direction, tag, label, sent, arrives, outcome, taken=True):
    """A message: `direction` "up" (MOC to vehicle) or "down", sent at row
    `sent` and arriving at row `arrives`; `outcome` is what the receiver
    does with it, and `taken` whether that is to act on it or read it."""
    return ("msg", direction, tag, label, sent, arrives, outcome, taken)


def note(side, row, text):
    """Something one end does, written beside its lifeline at `row`."""
    return ("note", side, row, text)


def wrapped(x, y, text, anchor, style, chars=OUTCOME_CHARS):
    """`text` as lines of at most `chars` characters, from `y` down."""
    lines = textwrap.wrap(text, chars)
    spans = "".join(
        f'<tspan x="{x}" dy="{0 if i == 0 else 14}">{escape(line)}</tspan>'
        for i, line in enumerate(lines))
    return (f'<text x="{x}" y="{y}" text-anchor="{anchor}" '
            f'style="{FONT}; font-size: 12px; {style}">{spans}</text>')


def legend(tags):
    """The key: one sample line per tag used, and the two outcomes."""
    out = []
    y = 46
    for tag in tags:
        colour, dash, words = STYLES[tag]
        out.append(
            f'<line x1="40" y1="{y}" x2="110" y2="{y}" stroke="{colour}" '
            f'stroke-width="2.5" stroke-dasharray="{dash}"/>')
        out.append(
            f'<text x="120" y="{y + 4}" style="{FONT}; font-size: 12px">'
            f'[{tag}] {escape(words)}</text>')
        y += 18
    out.append(
        f'<text x="40" y="{y + 4}" style="{FONT}; font-size: 12px">'
        'Arrowhead: read or acted on. Cross: dropped and counted, or not '
        'acted on. The words at each end say which.</text>')
    return out


def arrowhead(x, y, toward_right, colour):
    """A filled arrowhead pointing along the message."""
    d = -1 if toward_right else 1
    return (f'<polygon points="{x},{y} {x + 10 * d},{y - 5} '
            f'{x + 10 * d},{y + 5}" fill="{colour}"/>')


def cross(x, y, colour):
    """A cross where a message is dropped."""
    return (f'<path d="M{x - 6},{y - 6} L{x + 6},{y + 6} M{x - 6},{y + 6} '
            f'L{x + 6},{y - 6}" stroke="{colour}" stroke-width="2.5"/>')


def draw(name, title, desc, steps):
    """Writes `name`.svg from `steps`."""
    tags = []
    for step in steps:
        if step[0] == "msg" and step[2] not in tags:
            tags.append(step[2])
    tags.sort(key=list(STYLES).index)
    last = max(step[5] if step[0] == "msg" else step[2] for step in steps)
    height = TOP + int((last + 2) * ROW)
    row_y = lambda row: TOP + int(row * ROW)

    out = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" '
        f'height="{height}" viewBox="0 0 {WIDTH} {height}" role="img" '
        f'aria-labelledby="title desc">',
        f'<title id="title">{escape(title)}</title>',
        f'<desc id="desc">{escape(desc)}</desc>',
        f'<rect width="{WIDTH}" height="{height}" fill="#ffffff"/>',
        f'<text x="40" y="24" style="{FONT}; font-size: 15px; '
        f'font-weight: bold">{escape(title)}</text>',
    ]
    out += legend(tags)
    for x, who in ((VEHICLE_X, "Vehicle (tcspecial)"),
                   (MOC_X, "MOC (ground station)")):
        out.append(
            f'<text x="{x}" y="{TOP - 22}" text-anchor="middle" '
            f'style="{FONT}; font-size: 13px; font-weight: bold">{who}</text>')
        out.append(
            f'<line x1="{x}" y1="{TOP - 12}" x2="{x}" y2="{height - 10}" '
            f'stroke="#000000" stroke-width="1.5"/>')

    for step in steps:
        if step[0] == "note":
            _, side, row, text = step
            x = VEHICLE_X - 12 if side == "vehicle" else MOC_X + 12
            anchor = "end" if side == "vehicle" else "start"
            out.append(wrapped(x, row_y(row) + 4, text, anchor,
                               "font-style: italic; fill: #000000"))
            continue
        _, direction, tag, label, sent, arrives, outcome, taken = step
        colour, dash, _ = STYLES[tag]
        down = direction == "down"
        x1, x2 = (VEHICLE_X, MOC_X) if down else (MOC_X, VEHICLE_X)
        y1, y2 = row_y(sent), row_y(arrives)
        end = x2 - 14 if down else x2 + 14
        out.append(
            f'<line x1="{x1}" y1="{y1}" x2="{end}" y2="{y2}" '
            f'stroke="{colour}" stroke-width="2.5" '
            f'stroke-dasharray="{dash}"/>')
        if taken:
            out.append(arrowhead(x2, y2, down, colour))
        else:
            out.append(cross(end, y2, colour))
        mid_x = x1 + (x2 - x1) * LABEL_AT
        mid_y = y1 + (y2 - y1) * LABEL_AT - 6
        out.append(
            f'<text x="{mid_x}" y="{mid_y}" text-anchor="middle" '
            f'style="{FONT}; font-size: 12px; fill: #000000; '
            f'paint-order: stroke; stroke: #ffffff; stroke-width: 4px">'
            f'[{escape(tag)}] {escape(label)}</text>')
        x = MOC_X + 12 if down else VEHICLE_X - 12
        anchor = "start" if down else "end"
        out.append(wrapped(x, y2 + 4, outcome, anchor, "fill: #000000"))

    out.append("</svg>")
    with open(f"{name}.svg", "w") as f:
        f.write("\n".join(out) + "\n")


COMMANDED = [
    note("vehicle", 0, "Running A, restart flag clear"),
    note("moc", 0, "Using A, matched"),
    msg("down", "A", "datum", 1, 2, "read with A"),
    msg("up", "any", "RESTART_ARM to B", 3, 4, "armed"),
    msg("down", "any", "answer: armed", 5, 6, "taken"),
    msg("down", "A", "datum", 7, 10, "read with A: sent before the flagged "
        "beacon"),
    msg("up", "any", "RESTART to B", 7.5, 8.5, "taken: flag set; from now "
        "on only beacons and answers go down"),
    msg("down", "any", "beacon A, restart", 9, 11.5, "no longer matched; "
        "unsent [A] commands thrown away"),
    msg("down", "any", "answer to RESTART", 9.6, 12.8, "taken"),
    msg("up", "A", "START_NODE", 11, 12.3, "not acted on; answered with a "
        "refusal"),
    msg("down", "A", "refusal of START_NODE", 13, 14.5, "dropped and "
        "counted: not matched", taken=False),
    note("vehicle", 15, "Cleans up, runs again, loads B; restart flag set"),
    msg("down", "any", "beacon B, restart", 16.5, 17.5, "beacon's B is not "
        "this station's A: not matched; says the vehicle runs B"),
    note("moc", 19, "Operator retrieves B and loads it"),
    msg("up", "any", "CONNECT with B", 20, 21, "checksums match: flag "
        "cleared; queued commands dropped; downlink opens"),
    msg("down", "any", "beacon B, normal", 22.5, 23.5, "matched: queued data "
        "dropped, then reads on"),
    msg("down", "any", "answer: running B", 23, 24.7, "matched for "
        "commanding"),
    msg("down", "B", "datum", 26, 27, "read with B"),
    msg("up", "B", "START_NODE", 28, 29, "acted on"),
]

CRASH = [
    note("vehicle", 0, "Running A, restart flag clear"),
    note("moc", 0, "Using A, matched"),
    msg("down", "A", "datum", 1, 2, "read with A"),
    msg("down", "A", "datum", 3, 5, "read with A: sent before the new "
        "start's first beacon"),
    note("vehicle", 4, "Crashes; the supervisor starts it again; it loads "
         "A; restart flag set"),
    msg("down", "any", "beacon A, restart", 6, 7.5, "no longer matched; "
        "unsent [A] commands thrown away"),
    msg("up", "A", "START_NODE", 5.8, 7, "not acted on: not matched; "
        "answered with a refusal"),
    msg("down", "A", "refusal of START_NODE", 8, 9.5, "dropped and counted: "
        "not matched", taken=False),
    msg("up", "any", "CONNECT with A", 10.5, 11.5, "checksums match: flag "
        "cleared; downlink opens"),
    msg("down", "any", "beacon A, normal", 12.5, 13.5, "matched again: "
        "queued data dropped, then reads on"),
    msg("down", "any", "answer: running A", 13, 14.7, "matched for "
        "commanding"),
    msg("down", "A", "datum", 16, 17, "read with A"),
]

GROUND = [
    note("vehicle", 0, "Running A, restart flag clear: another station "
         "matched it"),
    note("moc", 0, "Restarted, using C"),
    msg("down", "A", "datum", 1, 2, "dropped and counted: not matched",
        taken=False),
    msg("down", "any", "beacon A, normal", 3, 4, "beacon's A is not this "
        "station's C: not matched; says the vehicle runs A"),
    msg("up", "any", "CONNECT with C", 5.5, 6.5, "checksums differ: "
        "changes nothing"),
    msg("down", "any", "answer: running A", 7, 8, "not matched: the "
        "answer's checksum is not C"),
    note("moc", 10, "Operator retrieves A and loads it"),
    msg("down", "A", "datum", 11, 12, "dropped and counted: no beacon "
        "heard since loading A", taken=False),
    msg("down", "any", "beacon A, normal", 13, 14, "matched for listening: "
        "queued data dropped, then reads on"),
    msg("down", "A", "datum", 15, 16, "read with A"),
    msg("up", "any", "CONNECT with A", 17, 18, "checksums match; the flag "
        "was already clear"),
    msg("down", "any", "answer: running A", 18.5, 19.5, "matched for "
        "commanding"),
]

if __name__ == "__main__":
    draw("restart-commanded", "A commanded restart from A to B",
         "The vehicle runs configuration A and the station is matched with "
         "it. The station arms and initiates a restart to B. A datum the "
         "vehicle sent before taking the restart arrives before the beacon "
         "that carries the restart flag, so it is read with A. A START_NODE "
         "the station sent before hearing that beacon reaches a vehicle "
         "that has taken the restart, so it is not acted on, and its "
         "refusal is dropped and counted on the ground. The flagged beacon "
         "ends the match, and the station throws away commands it built "
         "with A. The new start loads B and beacons B with the flag set. "
         "The operator loads B and sends CONNECT with B, which matches, so "
         "the vehicle clears the flag and opens the downlink. The station "
         "drops anything queued from before, becomes matched, and reads "
         "and commands with B.",
         COMMANDED)
    draw("restart-crash", "A crash and restart on the same configuration",
         "The vehicle runs A and crashes. A datum sent before the crash "
         "arrives before the new start's first beacon and is read with A. "
         "A command the station sent before hearing of the restart reaches "
         "the new start, which is not matched, so it is not acted on, and "
         "its refusal is dropped and counted. The flagged beacon ends the "
         "match. CONNECT with A matches again; the station drops anything "
         "queued and reads and commands with A.",
         CRASH)
    draw("restart-ground", "A ground station that restarts with another "
         "configuration",
         "The vehicle runs A, matched with another station. This station "
         "restarts using C. It drops and counts the vehicle's data, since "
         "the beacon says A. Its CONNECT with C does not match and changes "
         "nothing on the vehicle. The operator loads A; data is still "
         "dropped until a beacon with A and the flag clear is heard, then "
         "the station is matched for listening and reads with A. Its own "
         "CONNECT with A matches it for commanding.",
         GROUND)

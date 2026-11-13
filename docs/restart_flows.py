"""Draws the message-flow diagrams of `In-flight data across a restart`, and
those of the tcs-moc manual, which show what goes up and comes down as an
operator connects, commands a node and pings.

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
    """Describes one message for a diagram.

    Args:
        direction: "up", from the MOC to the vehicle, or "down".
        tag:       The configuration that made it: a key of ``STYLES``.
        label:     What the message is, drawn along its line.
        sent:      The row it is sent at.
        arrives:   The row it arrives at.
        outcome:   What the receiver does with it, in words.
        taken:     Whether the receiver reads or acts on it, rather than
                   dropping it.

    Returns:
        The message, as a step of a diagram.
    """
    return ("msg", direction, tag, label, sent, arrives, outcome, taken)


def note(side, row, text):
    """Describes something one end does, written beside its lifeline.

    Args:
        side: "vehicle" or "moc".
        row:  The row it is written at.
        text: What it does.

    Returns:
        The note, as a step of a diagram.
    """
    return ("note", side, row, text)


def wrapped(x, y, text, anchor, style, chars=OUTCOME_CHARS):
    """Writes text as lines of at most a given width, from a point down.

    Args:
        x:      Where the lines are anchored across.
        y:      Where the first line's baseline is.
        text:   The text.
        anchor: The SVG text-anchor: "start" or "end".
        style:  More CSS for the text.
        chars:  The most characters on a line.

    Returns:
        The SVG text element.
    """
    lines = textwrap.wrap(text, chars)
    spans = "".join(
        f'<tspan x="{x}" dy="{0 if i == 0 else 14}">{escape(line)}</tspan>'
        for i, line in enumerate(lines))
    return (f'<text x="{x}" y="{y}" text-anchor="{anchor}" '
            f'style="{FONT}; font-size: 12px; {style}">{spans}</text>')


def legend(tags):
    """Draws the key: one sample line per tag used, and the two outcomes.

    Args:
        tags: The configuration tags the diagram uses, in the order shown.

    Returns:
        The SVG elements of the key.
    """
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
    """Draws a filled arrowhead where a message is taken.

    Args:
        x:            Where its point is across.
        y:            Where its point is down.
        toward_right: Whether the message goes from left to right.
        colour:       Its fill.

    Returns:
        The SVG polygon.
    """
    d = -1 if toward_right else 1
    return (f'<polygon points="{x},{y} {x + 10 * d},{y - 5} '
            f'{x + 10 * d},{y + 5}" fill="{colour}"/>')


def cross(x, y, colour):
    """Draws a cross where a message is dropped.

    Args:
        x:      Its centre across.
        y:      Its centre down.
        colour: Its stroke.

    Returns:
        The SVG path.
    """
    return (f'<path d="M{x - 6},{y - 6} L{x + 6},{y + 6} M{x - 6},{y + 6} '
            f'L{x + 6},{y - 6}" stroke="{colour}" stroke-width="2.5"/>')


def draw(name, title, desc, steps):
    """Writes one diagram as an SVG file.

    Args:
        name:  The file's name, without ``.svg``.
        title: The diagram's title, drawn and given as the SVG's title.
        desc:  The SVG's description, for a reader who cannot see it.
        steps: Its messages and notes, from ``msg`` and ``note``.
    """
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

# The figures of `The restart and the beacon, in time`: the beacons are
# numbered, because what the ground judges a start by is the number going
# back, and the flag is what ends a match.

BEACONS = [
    note("vehicle", 0, "Running A, matched; a beacon every "
         "default_interval"),
    note("moc", 0, "Using A, matched"),
    msg("down", "any", "beacon #41, normal", 1, 2, "matched"),
    msg("up", "any", "RESTART_ARM to A", 3, 4, "armed"),
    msg("down", "any", "answer: armed", 4.5, 5.5, "taken"),
    msg("up", "any", "RESTART to A", 6.5, 7.5, "taken: state file written; "
        "flag set; nothing but the three acted on from now"),
    msg("down", "any", "answer: Success", 9, 10, "taken"),
    msg("down", "any", "beacon #42, restart", 10.6, 12, "flag set: the "
        "match ends; the station sends nothing but the three"),
    note("vehicle", 13, "Cleans up; a beacon every reset_interval"),
    msg("down", "any", "beacon #43, restart", 13.5, 14.5, "number goes up: "
        "the same start, ending"),
    note("vehicle", 15.5, "exec: the new start loads A; its first beacon "
         "goes at once"),
    msg("down", "any", "beacon #1, restart", 17, 18, "number went back: a "
        "new start; tcs-moc takes the link down: press Connect"),
    msg("up", "any", "CONNECT with A", 20, 21, "checksums match: flag "
        "cleared; downlink opens; a beacon at once"),
    msg("down", "any", "answer: running A", 21.5, 22.5, "CONNECT answered "
        "with this station's checksum"),
    msg("down", "any", "beacon #2, normal", 22.6, 24, "matched for "
        "commanding"),
    note("vehicle", 25.5, "A beacon every default_interval"),
    msg("down", "any", "beacon #3, normal", 26.5, 27.5, "matched"),
]

LOST_UP = [
    note("vehicle", 0, "Running A, matched"),
    note("moc", 0, "Using A, matched"),
    msg("up", "any", "RESTART_ARM to B", 1, 2, "armed: the arm stands for "
        "restart_arm_valid"),
    msg("down", "any", "answer: armed", 2.5, 3.5, "taken"),
    msg("up", "any", "RESTART to B", 4.5, 6, "never arrives", taken=False),
    note("moc", 7.5, "No answer: the client times out. Not sent again on "
         "that alone"),
    msg("down", "any", "beacon #42, normal", 9, 10, "number up, flag clear, "
        "at default_interval: the RESTART was not taken"),
    msg("up", "A", "START_NODE", 11.5, 12.5, "acted on: still matched with "
        "A"),
    msg("down", "A", "answer: Success", 13, 14, "taken: nothing held it "
        "back"),
    msg("up", "any", "RESTART to B, same key", 15.5, 16.5, "taken: the arm "
        "still stands; flag set"),
    msg("down", "any", "answer: Success", 17.5, 18.5, "taken"),
    msg("down", "any", "beacon #43, restart", 19.1, 20.5, "flag set: the "
        "match ends; the restart is under way"),
]

LOST_ANSWER = [
    note("vehicle", 0, "Running A, matched, armed for B"),
    note("moc", 0, "Using A, matched"),
    msg("up", "any", "RESTART to B", 1, 2, "taken: flag set"),
    msg("down", "any", "answer: Success", 3, 4.5, "never arrives",
        taken=False),
    msg("down", "any", "beacon #42, restart", 4, 6, "flag set: the "
        "RESTART was taken, though no answer came; the match ends"),
    note("moc", 8, "The client times out on the answer"),
    msg("up", "any", "RESTART to B, again", 8.5, 9.5, "not acted on: the "
        "first used the arm up"),
    msg("down", "any", "answer: NotArmed", 10, 11, "taken: restarts "
        "nothing; the beacons have said the first was taken"),
    note("moc", 12.8, "A station that watches the beacons refuses a "
         "START_NODE unsent from #42; one that does not sends it"),
    msg("up", "A", "START_NODE", 14.5, 15.5, "not acted on: a RESTART was "
        "taken; answered Unmatched", taken=False),
    msg("down", "A", "answer: Unmatched", 16.5, 17.5, "tells it why "
        "nothing happened"),
    note("vehicle", 18.5, "exec: the new start loads B"),
    msg("down", "any", "beacon #1, restart", 19.5, 20.5, "number went back, "
        "with B's checksum: load B and Connect"),
]

LOST_BEACONS = [
    note("vehicle", 0, "Started a moment ago, running A, matched"),
    note("moc", 0, "Using A, matched"),
    msg("down", "any", "beacon #2, normal", 1, 2, "matched"),
    msg("up", "any", "RESTART to A", 3, 4, "taken: flag set"),
    msg("down", "any", "answer: Success", 5, 6, "taken"),
    msg("down", "any", "beacon #3, restart", 6.2, 7.6, "flag set: the "
        "match ends"),
    note("vehicle", 8.5, "exec: the new start loads A"),
    msg("down", "any", "beacons #1 to #3, restart", 10, 11, "all lost",
        taken=False),
    msg("down", "any", "beacon #4, restart", 12.5, 13.5, "number up from "
        "#3: not seen as a new start, so the link stays up; flag set: "
        "still not matched"),
    note("moc", 15.5, "A START_NODE pressed now is refused here, unsent: "
         "not matched"),
    note("moc", 18, "The operator presses Disconnect, then Connect"),
    msg("up", "any", "CONNECT with A", 19.5, 20.5, "checksums match: flag "
        "cleared; a beacon at once"),
    msg("down", "any", "answer: running A", 21, 22, "answered with this "
        "station's checksum"),
    msg("down", "any", "beacon #5, normal", 22.6, 24, "matched for "
        "commanding"),
]

# The figures of the tcs-moc manual: what goes up and what comes down behind
# what an operator presses, as its screenshots show it.

MOC_CONNECT = [
    note("vehicle", 0, "Starts, loading A; restart flag set; a beacon "
         "every reset_interval"),
    note("moc", 0, "Opened on A: link down, every panel Unknown"),
    msg("down", "any", "beacon #1, restart", 1, 2, "indicator green; "
        "Restarted\u2014connect; not matched, so data would be dropped"),
    note("moc", 4, "The operator presses Connect"),
    msg("up", "any", "CONNECT with A", 5, 6, "checksums match: flag "
        "cleared; downlink opens; a beacon at once"),
    msg("down", "any", "answer: running A", 6.5, 7.5, "Last Response: "
        "tcspecial agrees"),
    msg("down", "any", "beacon #2, normal", 7.6, 9, "matched for "
        "commanding; asks what each node is doing"),
    msg("up", "A", "QUERY_NODE, one per panel", 10.5, 11.5, "answered"),
    msg("down", "A", "answers: running", 12, 13, "each panel Active"),
    msg("down", "A", "datum, each node to its port", 14.5, 15.5, "read: "
        "Last recv, From payload, Recv, and the plot"),
]

MOC_COMMAND = [
    note("vehicle", 0, "Running A, matched"),
    note("moc", 0, "Using A, matched"),
    note("moc", 1.5, "Disable pressed on a panel"),
    msg("up", "A", "STOP_NODE, the panel's node", 2.5, 3.5, "stops the "
        "node and frees what it held"),
    msg("down", "A", "answer: Success", 4, 5, "panel Stopped; Last "
        "Response says so"),
    note("moc", 6.5, "Enable pressed on it"),
    msg("up", "A", "START_NODE, the same node", 7.5, 8.5, "does not "
        "start it"),
    msg("down", "A", "answer: Failure", 9, 10, "Last Response: refused; "
        "the panel keeps Stopped"),
    note("moc", 11.5, "Query chosen from the Command menu"),
    msg("up", "A", "QUERY_NODE, one per panel", 12.5, 13.5, "answered "
        "with each node's state and counts"),
    msg("down", "A", "answers", 14, 15, "each panel's status; the counts "
        "under its Configuration"),
]

MOC_PING = [
    note("vehicle", 0, "Running A, matched"),
    note("moc", 0, "Using A, matched for commanding"),
    note("moc", 1.5, "Send ping pressed in the ping window"),
    msg("up", "A", "ping #1, the MOC's time", 2.5, 3.5, "through the "
        "gateway node to the ping node, which adds its own time"),
    msg("down", "A", "ping #1, both times", 5, 6, "at ground_base plus "
        "the ping node's id: the round trip and each way shown"),
    note("moc", 7.5, "Not matched: nothing goes up, and a pop-up says "
         "why"),
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
    draw("restart-beacons", "A commanded restart, every beacon heard",
         "The vehicle runs A, matched, beaconing every default interval; "
         "beacon 41 says normal. The station arms and sends RESTART. The "
         "vehicle takes it, sets the restart flag, answers Success, and "
         "sends beacon 42 at once with the flag set, which ends the "
         "station's match. While it cleans up it beacons every reset "
         "interval, the number still going up. After the exec the new "
         "start's first beacon, number 1, goes at once with the flag set; "
         "the number going back says a new start, and tcs-moc takes its link "
         "down. The station sends CONNECT with A, which matches: the "
         "vehicle clears the flag, answers, and sends beacon 2, normal, at "
         "once, and from then beacons every default interval.",
         BEACONS)
    draw("restart-lost-up", "A RESTART that never arrives",
         "The station arms a restart to B and sends RESTART, which is lost. "
         "No answer comes and the client times out. The next beacon, 42, "
         "counts up at the default interval with the flag clear, which says "
         "the RESTART was not taken. Nothing on the ground is held back: a "
         "START_NODE goes and is acted on. The station sends RESTART again "
         "with the same key while the arm still stands; it is taken, and "
         "beacon 43 carries the flag.",
         LOST_UP)
    draw("restart-lost-answer", "A RESTART taken whose answer is lost",
         "The vehicle takes a RESTART to B; its answer is lost, but beacon "
         "42 carries the restart flag, which says the RESTART was taken and "
         "ends the station's match. A RESTART sent again is refused "
         "NotArmed, since the first used the arm up, and restarts nothing. "
         "A station that watches the beacons refuses a START_NODE unsent "
         "from then; one that does not sends it, and the vehicle answers it "
         "Unmatched and does nothing. The new start "
         "beacons number 1 with the flag set and B's checksum.",
         LOST_ANSWER)
    draw("restart-lost-beacons", "A new start whose first beacons are lost",
         "The vehicle started only a moment ago, so its beacon numbers are "
         "low. It takes a RESTART and beacon 3 carries the flag, ending the "
         "match. The new start's beacons 1 to 3 are all lost, so beacon 4 "
         "looks like the next of the old start and the link stays up; but "
         "its flag is set, so the station is still not matched and would "
         "refuse a START_NODE unsent. The operator presses Disconnect and "
         "Connect; "
         "the CONNECT matches, the vehicle clears the flag, and beacon 5, "
         "normal, makes the station matched for commanding. Nothing is "
         "stuck.",
         LOST_BEACONS)
    draw("tcs-moc-connect", "Coming up: the first beacon, Connect, the match",
         "The vehicle starts on configuration A with its restart flag set "
         "and sends beacon 1; tcs-moc shows the indicator green and "
         "Restarted, connect, and is not matched. The operator presses "
         "Connect, which sends CONNECT with A. The checksums match, so the "
         "vehicle clears the flag, opens the downlink, answers that it is "
         "running A and sends beacon 2, normal, at once. tcs-moc says the "
         "vehicle agrees, becomes matched for commanding with beacon 2, "
         "and sends one QUERY_NODE per panel; the answers make each panel "
         "Active. From then each node's data is read into its panel and "
         "the plot.",
         MOC_CONNECT)
    draw("tcs-moc-command", "Commanding a node from its panel",
         "With the two ends matched on A, Disable on a panel sends "
         "STOP_NODE for that panel's node; the vehicle stops it and "
         "answers Success, and the panel says Stopped. Enable sends "
         "START_NODE; a vehicle that does not start the node answers "
         "Failure, which Last Response shows, and the panel keeps Stopped. "
         "Query, from the Command menu, sends one QUERY_NODE per panel, "
         "and the answers set each panel's status and the counts its "
         "Configuration shows.",
         MOC_COMMAND)
    draw("tcs-moc-ping", "Pinging the ping node",
         "Matched for commanding, Send ping in the ping window sends ping "
         "1 with the MOC's time up through the gateway node to the ping "
         "node, which adds its own time and sends it back down to the "
         "ground's port for it; the window shows the round trip and each "
         "way. Not matched, nothing goes up and a pop-up says why.",
         MOC_PING)

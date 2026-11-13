==============
tcs-gui Design
==============

.. contents:: Contents
   :depth: 3
   :local:

How tcs-gui, the configuration editor, works and why.

tcs-gui draws a configuration as the graph it is -- nodes, and the links that
join them -- lets somebody change it by drawing, and writes the result back to
the files it came from. What it reads and writes is the configuration, in
either spelling -- XML in a ``.xtcs`` file, YAML in a ``.ytcs`` one -- with
every file the configuration includes, and, where it is given one, the
simulator's own file, a ``.yaml``. It writes nothing else: no code, and
nothing the vehicle runs that the configuration does not already say. It runs
on the ground and never flies.

What the configuration means -- its sections, its nodes, its links and the
rules a configuration is held to -- is in the `tcspecial design
<tcspecial-design.html>`__, and the rules are ``tcs-verify``'s and the flight
software's, reached by the editor rather than restated. This document is about
the editor: how it reads and writes those files, how it keeps its undo and
recovers from a crash, how it draws a configuration, and the windows a
configuration is edited in.

Reading and Writing a Configuration
===================================

The editor reads what it writes
-------------------------------

The editor reads what it writes. A configuration opened in tcs-gui comes up on
the canvas -- nodes where its ``tcs-gui`` section put them (see `What the file
keeps for the editor`_), links between them, groups and command bindings held
and written back -- and the comments come with it: a comment is written as a
comment rather than as a field, so the editor reads the run of comment lines
above an entry as that entry's.

An *entry* is a node, a link, a group or a command binding. The file's
structure carries comments too, and those are kept the same way, written back
where they were read:

* a comment above a node section's line, ``payloads:`` or ``<payloads>``,
  with no blank line between, is that section's;
* a comment under it, above the section's first ``- type`` line, is the
  section's as well -- the paragraph about the section as a whole that the
  examples have there. It is also where the first type group's own comment
  would be, and a file cannot tell the two apart, so the section has it;
* a comment above any later ``- type X:`` or ``<type name="X">`` line is that
  type group's;
* a comment above a line of the ``tcspecial`` section -- a setting, a
  subsection such as ``beacon:``, or a subsection's setting -- is that line's.

Each lives as long as what it is about. A section or type group the editor
empties is no longer written, and its comment goes with it; one made again
later starts without, and so does a ``tcspecial`` setting cleared and set
again. Undo does not bring such a comment back, as it does not bring back a
removed node's.

Four kinds of comment in a hand-written file are not held by the editor, and
are not in the text it writes of its own:

* prose attached to nothing: a heading at the top of a file, or a paragraph
  after the last entry, belongs to no entry.
* a comment above the ``tcspecial`` section's own line, or above any other
  top-level key that is not a node section, which is not an entry either.
* a comment above one *attribute* of an entry rather than above the entry
  itself -- the line explaining why a sensor is at ``0x49`` is held, and the
  same line written directly above the ``address`` attribute is not.

A comment above a link is held, and a link is now an attribute -- so this is
the one attribute whose comment the editor holds: the run of comment lines above
a ``world-out:`` or ``moc-out:`` is read as a note about the links that
attribute makes. Where the attribute names several destinations the note is
about all of them, one line carrying one comment.

A comment above a ``world-in:`` or ``moc-in:`` is not held, those being
a record of links rather than links: the note belongs above the link, at the
end it is written at, and a comment the editor held in a derived line would be
a comment it moved. It is the fourth kind in the list above.

Nor is the order of the attributes: the editor's own text gives a node's
attributes and the section's parameters in the order a dictionary gives them,
which is alphabetical, where a hand-written file usually groups them by what
they mean; and it gives the types of a section and the nodes of a type by
name.

None of that is lost by a save, because a save does not write the editor's own
text over the file: it merges the edit into the file as it was read, as the
requirement below says. A comment the editor does not hold is a line the edit
did not touch, and stays where its author put it; so do the order of the
attributes, the blank lines and the quoting of each value, and a save with no
edit, which the Save command makes on demand, leaves the file as it was. Two
things still lose them. Where the edit and the author's hand touch the same
lines, the edit's lines are the ones written, a line somebody edited being the
one that has to change. And where the merged file would not read back as what
the edit should have made it say, the editor's own text is written instead, in
its own order and without the comments it does not hold: a file that keeps its
author's hand and says the wrong thing would be worse than one that does not.

Requirement
    A save changes only the lines an edit changed, in every file it writes:
    both spellings, the included files and the simulator's file. Comments,
    blank lines, the order of keys and how each value is quoted stay as the
    file had them, and a save with no edit leaves every file byte for byte as
    it was. The editor writes its model out as it always did and merges the
    difference the edit made into the file as it was read; the merged file is
    then read again and held to what the edit should have made it say, and
    where it says anything else the editor's own text is written instead.
    See ``tcs_gui::faithful``. A comment is written as it was given, never
    broken again to a width; the one change is XML's, which cannot hold two
    hyphens in a row inside a comment.

A configuration in two spellings
--------------------------------

A configuration that exists in two spellings is written in both.
``vehicle.ytcs`` beside ``vehicle.xtcs`` is one configuration written twice, and
two files meant to say the same thing drift the moment only one of them is
written -- the shipped example is such a pair, and keeping it a pair by hand
meant a command to remember after every session. Only where the sibling is
already there: writing one that is not would be the editor deciding somebody
wants their configuration in two languages, which is not a thing to decide for
them.

Saving, and Autosave
--------------------

Every edit is written as it is made, so saving is not something to remember,
while the File menu's Autosave is on, as it is until somebody turns it off.
Off, an edit is written by Save, and by the way out of the program, and not
before: for somebody who wants to try a change and keep it only if it works.
Which it is is a setting, ``<autosave>`` in ``$HOME/.tcs-gui/preferences.xml``,
and on where the file does not say; the File menu shows it as a tick beside
**Autosave**. Turning it on writes at once whatever was waiting. Autosave on
or off, the undo history is written after every edit: see `Crash recovery`_.

There is a Save command all the same, in the File menu and as a floppy disk on
the bar. It writes every file of the session now, which with Autosave on covers
the three things the ordinary case does not: retrying a write that failed once
whatever stopped it has been put right, rewriting a set of files left half new
by a failure between two of them, and answering somebody who has been taught by
every other program to save their work. The way out of the program does the same
for anything the files do not have, and says on the error output whether it got
there -- unless that was given up by *Leave without saving*: see `Leaving`_.

The directory a configuration goes in is made if it is not there. A name the
editor is started on need not exist -- that is how a new configuration is begun
-- and a name in a directory that does not exist is the same intention one level
up. Without that, every write of the session failed: with Autosave on the editor
saves each edit as it is made, so somebody who named a path with a new directory
in it got a window that worked, an alert about the first edit, and nothing on
the disk. All the missing components are made, since a path somebody typed may
name several. A directory that cannot be made at all -- usually a file in the
way of one -- is its own kind of failure with its own sentence, because its
answer is different: the others are answered by freeing space or by waiting, and
this one by naming a directory that can exist.

Save as is for when the files cannot be written where they are -- a directory
made read-only, a device that is full -- and for a copy. It asks in the
desktop's own Save dialog for a new name for the top file, and writes every
file of the session there: both spellings where the configuration is kept as a
pair, each included file placed relative to the new top file as it was to the
old one, so the names the top file includes them by still find them, and the
simulator's file beside the new top file under its own name. The copy is the
same configuration, with the same digest, and each file is written as a save
writes it, changing only what was edited. Before it writes over a file that is
already there it asks, and it writes nothing until it has. The session carries
on in the new files, and its undo history goes with it, so undo reaches back
past the Save as and past a crash after it; the old files' history is thrown
away, because it describes edits the old files may not have.

Leaving
-------

Exit offers to write anything the files do not have before it goes, which is
normally nothing. Four answers rather than two: *Save and leave*, *Save as and
leave*, for files that cannot be written where they are, *Leave without
saving* -- because somebody who has just been told the device is full may want
to go and deal with it, and a program that insisted on writing first would be
one they cannot get out of -- and *Stay*, because the menu is next to
everything else and gets pressed by accident.

*Leave without saving* means what it says. The edits the files do not have are
let go before the editor stops (``Editor::abandon_unsaved``), so the write on
the way out, which writes anything unsaved, finds nothing to write; and the
exit being one that was asked for, the undo history is deleted with it, so the
next session on the configuration puts nothing back either. A choice to leave
the files as they are is not one the next start should undo.

*Save and leave* that fails says why and stays, the offer still open: the one
thing not to do is to quit quietly with the work unwritten. An exit with
nothing unsaved asks nothing.

Files that cannot be opened
---------------------------

The simulator's file is joined to the configuration by name, and is held to that
from both ends. Two entries of one name are refused outright -- a name is the
whole of how the two files are joined, so two of them are two sets of settings
for one payload and the file does not say which applies -- and so is an entry
naming a payload the configuration does not have. Both are refused rather than
half-read: the file parses, so adopting part of it would mean writing the rest
of it away at the next edit.

Any file the editor is given that is there and cannot be opened -- the
configuration or the simulator's, unparsable, unreadable, or one of the refusals
above -- ends the editor with a message saying which file and why, before any
window opens. Such a file used to be opened as an empty one, so that the editor
could be used to fix it, and that was the editor's most dangerous habit: the
session still wrote to the file, every edit being written as it was made, and
the first one replaced the whole configuration, both spellings of it, with
whatever had been drawn in an empty window. The file had never been read, so no
undo could bring it back. A file that does not read is put right in a text
editor, by the message. A file that is not there yet is not refused: that is how
a new configuration is begun.

A new configuration
-------------------

A configuration that does not exist yet opens with the ground station already
in it -- a ``gateway_radio`` node named ``gateway-1`` -- at the upper right, in
the gateway working pane. Every graph has
one -- a vehicle whose telemetry reaches nobody is a vehicle nobody is
operating -- so placing it is work the editor can do rather than work it can
wait for. It is placed through the history like any other edit, together with
the ``tcspecial`` section's questions, so one undo takes it back for somebody
drawing a page of a larger configuration.

Nothing is raised about a configuration that is one node or none. A new
configuration opens with the ground station already in it, and the moment that
node exists every rule of `Concerns`_ has something to say about it: nothing is
linked to it, because there is nothing to link it to. A count on the button
before anybody has drawn anything is a complaint about the editor's own first
act, and it cannot be acted on until there is a second node. The rules come back
as soon as there is a graph to apply them to.

A configuration that does not exist yet is given the section's attributes and,
with one exception, none of its values. The window then has a field apiece and
the configuration answers nothing, which is the honest state of it: an editor
that filled in a protocol and a beacon interval on the grounds that they are the
same on every vehicle until somebody has a reason would be making a
configuration that looks as answered as one somebody thought about. Nothing of
an unanswered section reaches the file -- a ``tcspecial:`` with nothing under it
would read back as no section at all.

The exception is the beacon's interval, which a new configuration starts at
1000 ms. It is an exception because it is a different kind of question. A
socket is a fact about this mission and there is no value that is right until
somebody says so; "how often does the vehicle say it is alive" has an answer
that is right until somebody has a reason, and once a second is that answer.
``parameters::default_of`` is where it is written, so the editor and anything
else that makes a new section agree about it.

Read-only sessions
------------------

Requirement
    ``-r``, ``--read-only`` or ``--readonly``, anywhere on the command line,
    starts a session that shows the configuration and changes nothing:
    ``tcs-gui --read-only thermostat.ytcs``. The window is the editor's, with
    the controls that would change something greyed rather than hidden -- Edit
    and Delete on a node and in the window about the file's problems,
    renaming, Undo, Redo, Save, arranging, placing a node, drawing a line,
    every field in every window -- and nodes stay where they are when dragged.
    What the file came with is still checked and still said as the editor
    starts, in the same windows, because a configuration somebody opened to
    look at is one they want to know the state of; and every window that
    shows something can still be opened. The title bar says ``(read-only)``.

    The greying is the window's half. The editor's is that a read-only session
    takes no edit and writes no file -- not the configuration, not the
    simulator's, not the undo history -- whatever asks, and deletes no
    session's history as it starts or ends. Any other word starting with
    a dash is refused, as a misspelled flag would otherwise be taken for the
    name of a configuration to create.

What the file keeps for the editor
----------------------------------

Where a node is drawn is kept in the configuration, unlike how the canvas is
divided, which is a preference of the person at the keyboard (see `The working
panes`_). A node's position is in the configuration because it is part of the
diagram -- two people reading one configuration should see one drawing -- in a
``tcs-gui`` section at the end of the file rather than on the node it is about:

.. code-block:: yaml

   tcs-gui:
     nodes:
       - { node: gateway, x: 784, y: 40 }
       - { node: payload, x: 40, y: 160 }

Last, and in a section of its own, because it is a different kind of thing.
The sections above say what the vehicle carries and what it is told; this says
where somebody put the boxes on a screen, which the flight software ignores
entirely. So a reader meets the vehicle first and can stop reading when the
vehicle stops, and a node entry is about the vehicle and nothing else. A node's
position written on the node is refused where the file is read, saying where it
has gone.

The same section holds the concerns somebody has set aside (see `Concerns`_),
for the same reason: a decision about the drawing is one the next reader of it
should not have to make again. The simulator's file keeps a ``tcs-gui`` section
of its own for where its physics components are drawn: see `The Physics
pane`_.

The Undo History
================

Undo reaches back to the start of the session, and past a crash: the history
is the list of the edits made, in order, with how far along it the document
stands, and it is written to a file after every edit (``tcs_gui::journal``).
With Autosave on every edit is on the disk already, and what the history buys
is the ability to undo past a crash. With Autosave off it buys the edits the
files do not have yet too, which the next start puts back: see `Crash
recovery`_. Either way it is a convenience for the person who was working
rather than a record anybody else reads, which is what decides where it lives
and how long for.

It lives in the user's own state directory: ``$XDG_STATE_HOME/tcs-gui``, or
``~/.local/state/tcs-gui`` where that is not set. The specification names this
exact case -- state that should persist between restarts but is not important
enough to be data, with history given as an example -- and it is deliberately
not the cache directory, a cache being something a program can regenerate.

It used to sit beside the configuration, as ``<name>.tcs-gui-history``, on the
argument that a configuration copied to another machine should take its history
with it. That argument was worth less than it cost: a configuration lives in a
directory under version control, so the file turned up in ``git status``, in
review and in release tarballs -- and nobody has ever wanted to undo an edit on
a machine they copied a file to.

Each history is named for the configuration's own name and a digest of its
absolute path -- ``graph-3f91c2d4e5f60718.yaml`` -- so that a person looking in
the directory can tell what belongs to what and two configurations called
``graph.ytcs`` in two directories are two histories. The file says which
configuration it is the history of, because a digest cannot be read backwards.
The digest is FNV-1a written out in the source rather than the standard
library's hasher, which does not promise the same answer across Rust releases:
a hash that changed with the toolchain would orphan every history on the disk
without saying so.

It is deleted on a clean exit, which is what every other editor does: the
history of a session that ended tidily is of no further use, and a directory
that only ever grows is one somebody has to tidy. It is **kept** when the
editor does not exit tidily -- a crash, a kill, a machine that went down --
because that is the undo somebody will want when they come back; and kept, too,
when the exit was asked for but the last save failed, which is the one case
where the history is the only copy of a session's work. A read-only session
writes none and deletes none.

And it is pruned at startup, two ways: a history older than a week, and one
whose configuration is no longer where it was. The first is the ordinary bound
-- a crash a fortnight ago is not an undo anybody is waiting for -- and the
second is tidiness nothing else would do, a configuration being free to be
deleted or renamed by anything at all. A file in that directory that tcs-gui
does not understand is left alone: it is not the prune's place to decide that
somebody else's file is rubbish.

Crash recovery
--------------

The undo history is written after every edit, Autosave on or off, to the
user's state directory (``tcs_gui::history``), and with it how far into it the
files go and an MD5 digest of each file of the session -- the configuration's
files and the simulator's -- as the editor last wrote or read it. That is crash
recovery. When the editor opens a configuration whose history is there --
which it is only when a session stopped without exiting, or exited after a
save that failed, the history being deleted on a clean exit -- it holds the
history to the files: the digest of each as it is now must be the digest the
history holds. One changed by anything else since is a file the history no
longer describes, so the history is set aside -- deleted -- and the window
says so; the file is as somebody left it, and what is lost is the undo. A
history that holds no digests at all cannot be held to the files, and is set
aside the same way. Held to them, the history is the session's again, so undo
reaches back past the crash, and every edit it holds that the files do not --
made with Autosave off, or whose write failed, before the editor stopped -- is
put back at once, without asking, and written where Autosave is on. The window
says how many, and whether they are saved. See ``Editor::recover``.

A clean exit, which saves what is unsaved first, deletes the history; one
whose last save failed keeps it, since it is then the only record of the work,
and says on the error output where it is. An exit by *Leave without saving*
deletes it too, so nothing is put back that was given up: see `Leaving`_.

Undo and redo on the bar
------------------------

Undo and redo are a drawn arrow apiece beside the name of the edit they would
take back or put again. The arrow is what the button does and the name is what
it will do it to, which is the half worth keeping: "Undo" on a button is a word
anybody could have guessed, where ``move "payload"`` is something only this
session knows. Save is a floppy disk and no word at all -- the mark every
program uses for it, where the other two need their names because what they
would undo is a thing only this session knows. The three icons are the
project's own SVG files beside the markup, ``undo.svg``, ``redo.svg`` and
``save.svg``, rather than an icon set's: compiled into the program, they are
nothing to fail to load. Each takes the colour of the button's text, so it
goes grey with the button when there is nothing to take back. Beside them the
bar says how many edits can be undone -- *1 edit*, *2 edits*, the noun
agreeing with the count.

The Canvas
==========

The working panes
-----------------

The canvas is divided in three, and each part says in its top left corner what
it is for. Left to right: the *payload working pane*, labelled ``Payloads``,
which holds everything the vehicle carries: a device on a bus, a line it
drives, a link to something aboard; the *vehicle working pane*, labelled
``Vehicle``, which holds the vehicle's own graph -- the computations, the log,
a page of its own; and the *gateway working pane*, labelled ``Gateway``, which
holds the ground link. It is the order the examples' diagrams in the manual are
drawn in, so a diagram there and the same configuration in the editor read
alike.

The labels are light grey and italic, which is this window's way of saying "not
part of the drawing": one in the colour and the face of a node would read as
something placed. They are drawn under the nodes, so a node may cover one. A
label is there to be found when a pane is empty and when somebody has lost
their bearings, and a pane that reserved a row for its own name would be a pane
with a row nothing can be put in.

What that buys is a drawing that can be compared with last mission's. The three
parts change on different schedules -- the ground is the other end of the link
and not the vehicle at all, the vehicle's own graph persists, and what it
carries changes between flights -- so a diagram that puts each in its own
column shows at a glance what is different. Links cross the dividers freely --
that is what a payload is for -- and only positions are confined.

Each divider is drawn, and each can be moved: it carries a handle halfway down
it, and dragging that handle moves the line and resizes the two panes either
side of it. The line and its handle are drawn over the sheet and not on it, as
tall as the window shows, so they stay in view however far down the sheet is
scrolled; drawn on it, they scrolled away with the top of the sheet. No pane may
be squeezed below a tenth of the working pane, so none can be dragged shut -- a
pane with nothing visible in it is a pane whose nodes cannot be reached, and the
drag that does it is one press away. With three panes that means a divider stops
a tenth short of its neighbour as well as of the edge. A divider also stops
against a node rather than passing over it: against the right edge of the
rightmost node of the pane on its left and the left edge of the leftmost node of
the pane on its right. The alternative was to let it go where it liked and shunt
the nodes it passed, which is a gesture that quietly rearranges a diagram -- and
one that would have to carry every node it moved through the history to be
undoable. A line that stops where it would orphan something says what it is
doing while it does it. A node already on the wrong side holds nothing, so one
such node cannot freeze the control.

Where the dividers sit is kept in ``$HOME/.tcs-gui/preferences.xml``, as the
percentages of the working pane the gateway and vehicle working panes take, the
payload pane taking what is left. Each is kept by the pane it is about rather
than by its place on the screen, so a file written when the gateway pane was the
leftmost still means the same widths now that it is the rightmost. The file is
read when the editor starts and is what places the dividers; it is written
whenever a line is moved or Autosave is turned on or off, and with the
defaults, whose comments say what may be put in it, when it is not there. The
directory is made automatically, and a
preferences file that is missing, unreadable or hand-edited into nonsense
leaves the defaults in place and says so on the error output: an editor that
refused to start because it could not remember a pane width would be an editor
traded for a preference. A file written before there was a gateway working pane
states only the vehicle's share, and the new pane's share is taken out of that
one rather than out of the payload column somebody had arranged. The file was
called ``config.xml`` in one build and is still read under that name when the
current one is absent, so the rename forgets nothing; the old file is left for
its owner to delete.

How the canvas is divided is a different question from where a node is drawn,
and the answer is the other way about: it is not part of the diagram at all but
a working preference of the person at the keyboard, and writing it into the
configuration would make resizing a pane a change to a file that flies. A
percentage rather than pixels for the same reason: a percentage is the thing the
person chose, where a pixel count is how this build happens to lay a canvas out.

Within a session the divider is one position in the document's own
coordinates, shared by every window, and not three quarters of whatever window
it is drawn in. Which pane a node is in says what it is allowed to be, so a
divider that moved with the window would make a payload into a vehicle node by
being looked at on a smaller screen.

Reading a configuration puts every node in the pane its type belongs to. A
payload the file draws in the vehicle working pane is a drawing that
contradicts the rule the editor enforces everywhere else, and a reader has to
be told which of the two to believe; the commonest source of one is a file
written before the canvas was divided at all. A node that has to move keeps its
height and takes the near edge of its own pane, which is the smallest move that
obeys the rule, and steps down the grid from there if that would land it on top
of another -- a file with its payloads in a row would otherwise come up as one
payload with the rest hidden behind it. A node off the grid is brought to the
nearest grid point in the same way. A node already in its pane and on the grid
moves too if it covers one before it in the order of their names: the first
keeps its place and the later one steps down the grid until it is clear, so a
file that puts two nodes in one place comes up with both in view. tcspecial's
visual window settles what it draws by the same rule, and writes nothing.

That is a change to the document, so it is worth saying what it is not. It is
not an edit: nothing goes into the history, because undoing it would put a node
back somewhere the editor refuses to put one. And it writes nothing, nor does
an edit of something else: a save writes only what was edited, so a settled
node's corrected position is written when that node is moved. Where a node
settles depends on the window's panes, which are not a fact about the file.
Opening a configuration to look at it is not editing it, and an editor that
rewrote a file under version control by being pointed at it would be an editor
nobody could point at anything.

The selection panes
-------------------

There is a selection pane for each, one above another in the same order the
working panes are in -- payloads at the top, then the vehicle's, then the
gateway's -- and which one a shape was pressed in is what decides where it
lands. A node stays in its pane: a drag that would carry a payload rightwards
across a divider stops at it.

A type is placed from its list in either of two ways. A click places it where
it fits, as high as it can be in view, and scrolls the sheet to it. A press
that moves carries the type's shape out of the list under the pointer, drawn
faint at the sheet's scale, and letting go over the sheet places it there,
centred on the pointer, on the grid and in its own pane -- a type let go in
another's pane stops at its own pane's edge, as a drag of a node does, so the
gesture always places something where it can be seen. Let go anywhere but
over the sheet, it places nothing. The two are told apart by how far the
press moved, four pixels, and not by how long it lasted, as a node's click and
drag are. Both are one edit, and one Undo takes the node away; and both go
through ``tcs_gui::add_node`` (``add_physics`` for a physics component), so
that a node clicked into being and one carried there are the same node.

Each of the three lists is as tall as its own share of the column, with a line
between each pair that can be dragged to change the two it is between -- the
same arrangement as the working panes, turned on its side, and kept in the same
settings file as percentages. A line moves the boundary between its own two
neighbours and leaves the third alone, which is what a person dragging it
expects: the list above grows by what the list below gives up. The upper line
is between the payload list and the vehicle's, the lower one between the
vehicle's and the gateway's. The arithmetic
is ``tcs_gui::list_shares_after``, in Rust where it can be tested, and the
markup reports a drag as how far it went and how tall the column is -- what a
share is depends on what it is a share of.

No list may be squeezed below a tenth of the column, so a line dragged to
the top or the bottom stops a tenth short of it. It is the rule the working
panes already have and the same constant: some of every list is always to be
seen, because a list with nothing showing is a list whose entries cannot be
reached, and a line dragged to the edge is easy to do by accident and hard to
understand afterwards. The floor was a twentieth once, which left a list half
its own heading.

And every list shows a node when the window opens, which a share of a column
cannot be trusted to do by itself: a twentieth of an 820-pixel window is
forty-one pixels, enough for the list's own heading and nothing under it. So the
default shares -- 20 for the physics list, then 25, 35 and 20 -- are checked
against what the lists actually have to draw at the height the window opens at,
and the test reads both the window's preferred height and the list's own padding
out of ``main.slint`` rather than restating them: a rule about what fits is a
rule about numbers in two files, and a test that copied them would pass while
the window showed nothing.

Each selection panel draws its shapes at three quarters of the size they are
on the canvas -- outline, terminals and name together -- so a list is a list of
the shapes rather than a column of the canvas, and more of it is in view at
once.

Each selection panel is in alphabetical order under each of its headings. A
palette is a thing people look in rather than read, and the order it had was
the order the type table happens to declare things in, which is no order at
all to somebody hunting for ``rpmsg``. Within a heading and not across the
panel: a heading is a statement about the entries under it, so a list sorted
past its headings would file a device under whichever heading its first letter
landed in. The headings keep the table's order, which is the one place that
order says something -- devices, then the things that are driven, then the ends
of the graph.

The Physics pane
----------------

Left of the three, where a simulator file is open, is a fourth: the *Physics
pane*, labelled ``Physics``, which holds the simulator file's world -- its
physics components -- and not anything of the configuration's. It is drawn
beside the working width rather than in it, at negative positions in the
coordinates the configuration holds, so that adding it moved nothing a file
had placed. A component is drawn where it was put, which the simulator file
keeps in a ``tcs-gui`` section of its own, last in the file as a
configuration's is:

.. code-block:: yaml

   tcs-gui:
     nodes:
       - { node: zone, x: -193, y: 280 }

The simulator file and not the configuration, because the world is that
file's: two configurations that share one simulator file, as the thermostat
examples do, share one drawing of its world. A component it does not place is
laid out down the pane by name, the one order a model has, so a model nobody
has arranged is drawn the same every time; one placed outside the pane is
brought inside it, and one placed on another steps down until it is clear,
as a node does. A position left behind by a component removed is kept, as a
configuration's is, and not written again. A component is the one shape no
node type has, a hexagon pointed at both ends with a dotted edge, narrower
than every node -- and narrower still in a pane too narrow for it and the
lanes its lines run down, so that it stays in its pane -- in the pane's own
reddish purple, with its type written along its top and its name in its
middle, as every node's are. It has at most two
terminals, both on its right, towards the payloads: an output at the top,
which every value it gives out leaves by, and an input under it, which
everything it takes in arrives at, each shown only where its type has a
terminal that way. A component's terminals are many and named, but which one
a line is joined to is written in the file, and a box for each would leave no
room for the name. A physics connection is drawn between the two components
it joins, and each simulated payload's ``connects`` between its component and
the payload node, each from or to a terminal, the arrow the way
the value goes: both dashed and in that purple, where a configuration's links
are solid. Neither is labelled, the lanes they run in being narrow; a pointer
resting on one is told the terminal it is joined to in a tip, which is what
tells a line into a thermal mass's ``heat`` from one into its ``heater_1``
when both enter its one input box. tcspecial's visual window draws the same
pane from the simulator file tcs-sim would read for the configuration, and only
draws it. See ``tcs_diagram::physics``.

The physics list is at the top of the selection column, its pane being the
leftmost, with a line under it that is dragged as the others are. Its entries
are the component types, under ``Thermal``, ``Power`` and ``Motion``.
Pressing one adds a component of that type to the simulator file's world, under
a name nothing has -- ``heater_3`` -- with nothing stated, and opens the
physics window on it, which shows its type's settings and, under them, what
the simulator's own reader says is wrong with it as it is typed: a figure
without its unit, one required and not given. A component the reader would
refuse is drawn red on the canvas.

A component is then handled as a node is. Dragged, it moves, on the grid and
inside its pane; a left click selects it, and a right click offers a menu of
Edit, which opens the physics window on it, and Delete. A line taken from its
output or its input terminal, as a node's is, and clicked on another component
is a connection between them, and over a payload, or the box of a subdiagram,
which is all of one the top page shows, it joins that simulated payload to the
component: its ``connects`` is set to the terminal, or, for a GPIO payload
watching lines, given the terminal after those it names already, one decision a
line. The line says only which two things, the component being drawn with one
output and one input whatever its type has, so the terminals come from the rules
a file is held to -- ``tcslib_protocol::node::physics::connectable`` for a
connection, an output to an input of one kind, and
``tcslib_protocol::sim_config::may_join`` for a simulated payload -- and each is
offered where the line was clicked, with Cancel, one join or several -- a
heater's heat, current and heating to a sensor -- rather than one guessed at: a
line to the wrong terminal is a simulator that reads something other than was
meant. A join already made is offered greyed, saying so. Moving, connecting and
joining, like placing, setting and removing, are edits like any other, each
taken back by one undo. Removing a component removes the connections that join
it and takes it out of every simulated payload's ``connects``, as removing a
node removes its links, and all of it comes back with one undo. Without a
simulator file the list is greyed and says to open one, and so does the Physics
pane, which is drawn all the same: some of every pane is always to be seen, and
a pane that came and went with a second file would be one nobody could find. The
world's ``ambient_temp`` is edited in the simulator file for now, and so is a
connection or a join that is to be taken away.

Where the Physics pane's line and the physics list's line are is kept in the
preferences file beside the others: the pane's width as a percentage of the
working width, and the list's share of the column.

How large the sheet is drawn
----------------------------

Some of every pane is in view whatever the window's width, too. A window
narrower than the sheet draws the sheet smaller, as much as it takes for all
four panes to fit beside the selection panes, down to half size; and the main
window cannot be made narrower than the sheet at half size and the selection
panes together, so the sheet never has to be scrolled sideways to find a
pane. Before, a narrowed window cut the panes off at its right edge and the
Gateway pane went first. Drawn smaller rather than with narrower panes,
because a node's place is kept in its file in the sheet's pixels: narrower
panes would leave nodes outside them that nobody had moved. Only the drawing
is scaled. A drag, a line drawn, a node placed, a divider moved and the
sheet scrolled are all still measured in the sheet's own pixels, so nothing
that reads a position knows how large it is drawn. Half size is the floor
because below it a node's type cannot be read.

That is the size a window starts at, and **View -> Fit the width** goes back
to it. The View menu's other entries choose a size outright: **Zoom to the
drawing** fits every node and component in the view, **Zoom to the
selection** the one node, line or component selected, and **Actual size**
draws the sheet at its own size, keeping what is in the middle of the view
there. A zoom chosen outright may be under a half -- somebody who asked to see
the whole drawing is shown it -- and is never under a quarter, at which a node
is a dot, nor over twice, at which one node fills a window. Zooming to
something scrolls it to the middle of the view. Zoom to the selection, with
nothing selected, is greyed rather than doing nothing.

The grid
--------

Every node is drawn the same width. Nodes of four widths in a column have no
edge to line up on, and a diagram built from them looks approximate however
carefully each node was placed.

And every node is drawn the same height, 64 pixels, for the reason that follows
from the terminals: they are drawn a fixed inset from the top and the bottom of
a node, so one height is what puts every node's two rows of terminals the same
distance apart. Two nodes level on the grid have their terminals level too, and
a line between them runs straight across rather than stepping by the
difference between two heights. Height used to be one of the channels that
told two types apart, running from 40 to 96 pixels; without it, and with every
payload a parallelogram, four edges by two weights could no longer keep the
twelve payload types two channels apart. So there are six edges -- one line,
two, dashed, dashed and doubled, dotted, and dotted and doubled -- each payload
has a line and a weight no other payload has and a typeface of its own, and the
other types that leaned on height were given an edge, a weight or a typeface
instead. The rule that no two types differ by colour alone is unchanged, and
the test that holds it is unchanged.

The canvas is measured in millimetres -- 96/25.4 of its pixels, which is what
the window toolkit means by a millimetre -- each one rounded to the nearest
whole pixel, so positions in the file stay whole numbers and the grid never
drifts. A node's terminals are on lines five millimetres apart, and a node
moves in steps of five millimetres to keep them there: its upper left corner is
a millimetre short of a line -- at 4 mm, 9 mm, 14 mm and so on -- which puts
its terminals' first line a millimetre inside it. A node at any millimetre
would have its terminals rounded up to five millimetres from where they belong,
and in a node seventeen millimetres tall the two rows would meet. A node
placed, a node dragged and a node read from a file all land on one of these
places; the dividers between the panes sit on the millimetre grid, and a pane's
edges are taken in to the places just inside them, so a node held against an
edge is where a node may be as well. The canvas draws a faint line every five
millimetres, which are the lines the terminals are on.

Nodes
-----

Names and types
~~~~~~~~~~~~~~~

A node's name is written in the middle of its shape, both ways, which is where
a reader looks first: the terminals hold the four corners, so the middle is the
name's alone. Its type is written along the inside of the top, centred between
the terminals in the two top corners -- or between the shape's own sides where a
corner has none, a leaning side followed at that row -- so that what a node is
can be read without remembering what its shape means. A compute node's line
names its function instead -- ``vote``, ``to_kelvin``, ``thermostat``,
``process`` -- because ``compute`` is one type and many things, its shape and
colour are the type's, and a page of computations each written "compute" is a
page whose reader has to open every node to find the thermostat. The function
stands alone rather than after "compute, ", the shape already saying that, and
one with no function yet says ``compute``
(``tcs_diagram::view::type_line``). There is less room
between two terminals than across the middle, so the type is set in the
diagram's own small size, nine pixels, rather than the window's: the one label
on the canvas that is, and the longest type, ``subdiagram`` between four
terminals, fits at it as drawn (``tcs_diagram::style::TYPE_SIZE`` and
``type_band``). A closed subdiagram is taller than the other nodes, 80 pixels
to their 64, so that what it holds -- how many nodes, and how many lines cross
its edge -- has room under its name, and the mark that opens it the middle of
its bottom edge; a line to its lower terminals steps where one to another
node's would run straight, which is the price of saying what is in it without
opening it.

Outlines and edges
~~~~~~~~~~~~~~~~~~

There are three outlines, one to a pane, so the outline says which pane a node
belongs in before anything else is read: a payload is a parallelogram, a part
of the vehicle a rectangle with rounded corners, and the ground the plain
rectangle. Within an outline the types are told apart by the line around them,
its weight and the way the name is set.

Every part of the vehicle is a **rectangle with rounded corners**, the process
of a data-flow diagram: a computation, a log, a null node, a tee, a page of its
own and the three apps aboard. They were five shapes once -- a log the
open-ended data store, a page and the apps the barred predefined process -- and
each of those said something about the node at the cost of saying it is part of
the vehicle. The corners are arcs of nine pixels drawn as a path, in eight
straight pieces each, rather than a corner radius on a rectangle: a radius
cannot be dashed or doubled, and seven types of one outline need the edges to be
told apart by. Nine pixels is tight enough that a terminal three pixels in from
the border is inside the curve, so a rounded node's terminals sit where every
other node's do.

A payload is a **parallelogram**, every one of them, the flowchart symbol for
input and output on an unspecified medium -- which is what a payload is: a bus,
a socket, a channel, a sample buffer, a file, lines driven, a program. The
outline alone then says which nodes are what the vehicle carries; ``gpio``, as
the discrete lines were one type then, was a keyed rectangle and the two
payloads that are programs wore the bars of a predefined process, and each of
those said something about the node at the cost of saying it is a payload. It is
the one symbol in the notation that means *data crossing a boundary* rather than
a thing or a step, and a payload is exactly that boundary. It leans right, as
the symbol is drawn, so the corners it gives away are the top left and the
bottom right.

The top left is where a payload's ``world-out`` terminal is, so this is the
first shape whose slant had to be paid for rather than dodged. Three things
move with it, all of them from ``style::room_at``, which says how far in the
shape is at the row a given terminal sits in: the terminal box itself, so the
arrow stays on the node; the point where a line joins the node, so the line
stops where the shape is instead of out in the notch beside it; and the column
the name is padded away from, so a name is not padded into a terminal that has
moved. The room is measured at the edge of the terminal's box nearest the top
or the bottom, not at the row's middle, because a leaning side is furthest in
at one end of the box and taking the middle would leave half of it hanging
out.

Nothing is drawn inside a leaning shape that could spill out of it. The second
line of a doubled edge is inset as a *smaller box* and so leans by its own
slant, and a test holds it inside the outer line.

The shapes are the ones the standard notations use, which is worth more than
being inventive: a reader who has seen a data-flow diagram already knows that a
parallelogram is data crossing a boundary, a rounded box is a process and a
plain rectangle is something outside the system.

The ground station has worn a pill and a five-sided point before this. Both
said "not like the others" by being unlike anything; the rectangle says it by
being the one shape a reader of these notations already knows the meaning of,
and it is free to say it now that the payloads wear the data symbol. It is the
same size as everything else, 152 by 64, and it is wide, solid, azure and
set in the largest italic, so it catches the eye without being the node shaped
oddly.

Every outline -- a side that leans, corners drawn in pieces, a plain box --
arrives as path data from ``style::shape_of`` and the markup draws them with a
``Path`` -- the same arrangement as the link routes, and for the same reason:
geometry that can be read and tested belongs in Rust rather than in a layout
language. A ``Path`` has no drop shadow, so the lift such a node gets while it
is being dragged is a second copy of the shape, three pixels lower and barely
visible, drawn under the first.

A terminal is drawn in the corner of a node's box, so a shape that gives a
corner away has to answer for the terminal that was in it. A test holds every
shape drawn from a path to that: each terminal of the type has to be inside
the path, and anything the markup draws inside the shape has to be inside it
too. It is checked as the rule rather than as the shape, because the reasoning
is about where the terminals are, and a shape kept while a terminal moved
would otherwise draw an arrow floating beside the node.

Nothing is drawn as a pattern inside a node. A body is its own colour and
nothing else: a pattern there competes with the name written over it and with
the colour the body carries, and it was the one channel that looked like noise
rather than like a statement. What it used to tell apart is now told apart by
the line around the outside, which may be **doubled**, **broken**, both or
neither -- two independent facts, so four values out of one channel.

Only four pairs of types ever leaned on the old pattern channel, all of them
payload kinds drawn at the same shape, width and height, and the new one
carries all four: a dashed serial against a plain I2C, and HDLC doubled,
Network dashed and IIO both, which separates the three of them from each
other.

A broken line is only available to a straight-sided outline. Dashes are walked
along the shape's own polygon -- ``style::polygon_of``, which is also where the
filled shape and the inner line come from -- and the rounded outline is a
corner radius with no polygon to walk. A test holds the two types drawn with a
radius to a single whole line, because the markup would otherwise draw a plain
rounded rectangle and say nothing about the dash it could not manage.

The dashes themselves are subpaths, one per dash: Slint's ``Path`` takes no
dash pattern, so a broken line is drawn as the pieces that are there, which is
what ``route.rs`` already does for the line being dragged. The pattern is
walked round the whole shape rather than restarted at each side, so it carries
on around a corner instead of putting a dash on all four of them. And the
shape is filled by one path and stroked by another, because filling a row of
short subpaths would fill each dash rather than the node.

Terminals
~~~~~~~~~

A node is drawn with its terminals on it: a rectangle of 36 by 14 pixels in
each corner it has one in, with an arrow in it pointing the way the data on it
travels across the canvas. Two rows, which are the two streams. Along the top,
left to right, the payload's own data on its way to the ground: it arrives at
``world-in`` and leaves at ``world-out``. Along the bottom, right to left,
what the ground sends on its way to a payload: it arrives at ``moc-in`` and
leaves at ``moc-out``. Each box's upper left corner is on one of the lines
five millimetres apart, so the terminals of nodes line up with each other and
with those lines, and a line between two level terminals runs along one: worked
out from where the node is, a millimetre not being a whole number of pixels,
and taken at the first line in from the border -- the last at the bottom and
the right -- so a box never leaves its node or crosses its slant. The two rows
are ten millimetres apart.

So the arrows along the top point right and the arrows along the bottom point
left, because that is the way the canvas itself is laid out -- the payloads
are the leftmost working pane and the ground the rightmost. A reader who has
worked that out once need not read the names again; and a reader who reads them
is told whose data it is and whether it arrives or leaves, which is the
question somebody about to draw a line is asking.

The boxes were 24 by 10 -- wide enough for an arrow and no wider -- and that is
a strip to aim at. A press has to land on a terminal exactly, the terminal being
what says which stream the line carries, where the next click only has to land
on a node; so the box is now twice the area and bigger in both directions, the
hand being short in whichever one it is short. What bounds them is a node's
size, 152 by 64, ``app_all_veh`` being the one type with all four: two rows and
their insets have to fit inside that height with daylight between them, and two
columns inside the width. A name never collides with them however they grow,
being padded away from whichever side carries terminals.

Only the terminals the type actually has. A payload carries its two on its
right, a gateway link its two on its left, an ``app_all_veh`` all four, a tee
its three, a null node its one, a log the one it receives on, and a computation
and a page of its own all four -- a computation may be on either stream in
either direction, and what is inside a page may have any of them, which nothing
outside it knows. A drawing that showed four on every node would say nothing
about what may be linked to what, which is the one thing about a graph that the
shapes cannot say. The table is the schema's own,
``tcslib_protocol::graph::terminals``, read rather than copied wherever it has
an answer, so a terminal added there appears on the canvas without the editor
being touched; the computation, the log and the page, whose terminals that table
leaves to them, are answered in ``tcs_diagram::style``.

The label is padded away from whichever side carries terminals rather than a
row being reserved at the top and the bottom: the name stays in the middle of
the node and stops short of the corner boxes, where reserving rows would leave
a few pixels for a name on the shortest node.

A node that is wrong
~~~~~~~~~~~~~~~~~~~~

What is wrong with a node is not drawn on it as a sentence. A node of a type
this build knows that is wrong -- a value a rule refuses, a name another node
has -- keeps its shape, its edge and its type line, and is painted in the
error red, the one red on the canvas (``error-ink`` in ``diagram.slint``); the
marks drawn on its body stay white, so the one thing that changed is the thing
that is wrong. A node of a type this build does not know is drawn differently
as well: as a small plain rectangle, 104 by 40 where every other node is 152
by 64, with a thin single edge and no terminals, in the same red
(``tcs_diagram::view::node_look``). Showing it beats refusing the file: a
configuration written for a later build is readable, and what cannot be
placed is visibly what cannot be placed.

So on the canvas the red is the whole of what marks a faulty node of a known
type, and colour must not carry a meaning alone. What does not depend on it is
said off the canvas, in words: the errors button on the bar says how many
errors there are -- *3 errors* -- and its menu has a line for each, naming the
node, which goes to the node, chooses it and opens its window (see `Errors`_);
the node's own window says what is wrong; and the alert says it the first time
it is wrong (see `Alerts`_). The sentence was once drawn on the canvas, inside
a box forty-eight pixels tall, which is a paragraph in the middle of a
diagram; a diagram is read for its shape.

Colour
------

Each working pane is painted, and every node type has its own colour inside its
pane's family. The gateway pane is a pale blue, the vehicle pane a pale green
and the payload pane a pale teal: all three light and neutral -- no chroma
above 14 -- so that a pane reads as tinted paper rather than as a coloured
panel. The nodes drawn on one are darker members of its family: a payload is
a dark teal, a vehicle node a green or an olive, the ground link a blue. The
colours are written down in ``tcs_diagram::look``, which tcspecial's visual
window paints with too.

So a node's colour says two things at once. The hue says which family it is in,
which makes a node drawn in the wrong pane visible as one; the step within the
family -- lightness mostly, with chroma and a little hue beside it -- says
which type it is. The selection panes carry their family's ground too, so
picking a shape out of a list on a teal ground and putting it on a teal pane
says where it goes twice.

The colours were searched for rather than chosen, and three things were asked
of the search at once: that every tint stay in the hue family of its own pane,
that it be dark enough against that pane to read as a shape -- a contrast of
three to one, which is the threshold for something that is not text, and at
least 25 CIE units darker -- and that, subject to those, the smallest perceived
difference between any two of the fifteen node tints be as large as it can be,
measured in CIE L\*a\*b\* under normal vision and under simulated protanopia,
deuteranopia and tritanopia at once.

Over the node tints alone it came to fourteen CIE units, where two or three is
"the same colour in two printings" and ten is "clearly two colours"; with the
panes and the lines held to it as well it is twelve, as below. So a pair
anybody can tell apart is a pair everybody can tell apart, which is what colour
blindness asks of a palette.

Fourteen and not the seventeen the earlier family-free palette managed, and the
three units are the price of the families: a palette free to put its colours
anywhere spreads them further than one whose reds must stay red. What the
families buy is a colour that says which pane a node belongs in, which no
amount of separation would have told anybody. The families are told apart by
more than hue as well, because hue is what a dichromacy takes: the payload
family is dark, the vehicle family light, and the ground link's one tint sits
where nothing else does. Lightness is the one channel every kind of colour
blindness keeps, the total ones included.

The simulation and all of these checks are in the editor's own tests, run
against the colours the window is actually given, because a tint nudged by hand
would be a near-copy of another under one of the dichromacies and nobody with
normal vision would see it. A failure there means the search has to be run
again rather than a value adjusted.

Colour remains the redundant channel for all that. Nothing is told apart by
colour alone: every type differs from every other in at least two of outline,
border width, the line around it, height and the way its label is set, and a
test fails if that stops being true. Red is reserved for things that are wrong.

The colours themselves are three tiers of one hue family, lightest first: a
**working pane** is the palest shade of it, a **link line** carrying that
family's data is darker, and the **node bodies** are darker still. A drawing
then reads as one picture -- a pale sheet, lines drawn on the sheet, solid
blocks sitting on the lines -- rather than as three unrelated colour choices,
and a line can never be mistaken for a node's own edge. Each step is at least
eight CIE units, which is "clearly lighter" rather than "possibly lighter".

The three panes are chosen **as a set**: a blue, a green and a teal, each a
wash, 14 CIE units apart from one another under four ways of seeing. That is
the thing that changed. They used to be chosen one at a time, and came out at
84, 91 and 95 of lightness with chromas of 6, 14 and 14 -- a pale blue, a
near-white green and a grey. Three pale colours are told apart mostly by
*lightness*, so that spread was their separation, and a canvas whose three
grounds have nothing in common is a canvas that reads as three unrelated
choices rather than one palette.

The trade was measured before it was made. Holding the three within four
points of lightness and three of chroma -- the most coherent the gamut allows
-- the best floor any trio reaches is 13.2; within six, 13.8; within eight,
14.6; and with no constraint at all, 17.1, which is the shape the canvas had.
These three sit at 90, 90 and 82 with chromas of 13.5, 13.5 and 8.6, and their
closest pair is 14.5 against the 15.2 of the three that came before. Half a
unit, for a canvas that holds together.

A pane no longer carries its family's hue; the payload pane and what it
holds happen to share one, teal, and the other two need not. Which pane a node
belongs in is said by the family's own hue -- ``family_hue`` -- and a pane's
job is to be a quiet ground that the other two are not. What a pane is still
held to is the rest of it: above 82 of lightness, under a chroma of 14, 8
clear of both link lines, 10 clear of the window's white chrome, and
light enough that its own family's lightest body keeps three to one against it
and sits 25 points below it. The contrasts across all fifteen bodies run 3.2
to 9.4 to one, and the scheme floor for everything a reader must tell apart is
where it was, at 12.2.

One rule had to change to say what it always meant. A node's colour says which
pane it belongs in, and that was checked against the *pane's* hue; a pane with
a chroma of 1.8 has no hue worth measuring against, its number being
arithmetic on a rounding error. Each family's hue is declared instead --
``family_hue``: the blue of the ground link at 265 degrees, the green of the
vehicle's own graph at 118, the teal of what it carries at 200 -- and the three
are held 60 degrees apart, because a family spans 40 either side of its centre
and two families that overlap say nothing about where a node belongs.

Node body colours are **derived**, not chosen. Each panel's nodes are a ramp: it
starts twelve CIE units below the line that carries that panel's data -- the
vehicle panel below both lines, having none of its own -- and steps three and a
half units darker for each node, taking the panel's nodes in alphabetical order.
The ground station takes one rung, the vehicle's four, and what the vehicle
carries ten, from 46.2 of lightness down to 15.0 -- the first twelve below the
payload line, which is the same family's hue, so that the lightest payload body
is a darker teal than the line and plainly not it. A reader finds a node in a
panel by how dark it is, and a panel reads as one family instead of as a set of
unrelated colours.

The first step is twelve rather than the eight the tiers ask, because the
ground station's line and its bodies are the same hue: at one hue a step in
lightness *is* the difference between two colours, and twelve is this scheme's
floor. The ramp stops at thirteen of lightness, which is where the three
families stop being told apart -- below it a dark blue, a dark green and a
dark teal converge under deuteranopia. A panel with more nodes than its
ramp has rungs starts again at its first colour, so two of its nodes share
one. The apps are the ones that wrap: a panel's other types are walked first
and the apps after them, so the two payload apps take the first two payload
colours again and the three vehicle apps the first three vehicle colours,
and no type that had a colour before the apps came lost it.

What this gives up is the twelve units between *neighbours* in one panel. Ten
steps of twelve would want a hundred and twenty units of lightness out of the
eighty-seven there are, so within a panel the steps are three to four units --
"a shade darker" -- while across panels the closest pair is 29.9. That is a
deliberate trade and the reason colour is the redundant channel here: every
pair of node types differs in two of outline, edge, border width, size and the
way its label is set, and a test holds them to it. What a body's colour says
is which panel it belongs to and how far down the panel it sits.

A body is painted in its own tint now rather than washed with a fifth of it,
and what is written on it -- the outline, the name, the terminals, the marks
in its corners -- is white. The pane's own shade was the first thing tried for
that and is not light enough: the payload pane is the darkest of the three at
84 of lightness, and its shade on its own lightest body is 3.4 to one, under
the floor for text somebody has to read. White clears 4.5 to one against every
body, and a test says so.

Each link line is a **darker shade of the pane its data comes from**. The
payload's own readings start in the payload pane and generally end at the
ground, so that line is a teal taken from the teal pane they leave; what the
ground sends starts in the gateway pane and ends at a payload, so that line is
a darker blue. A reader can then see where a line's data started without
following it back, and the hue is held within twenty degrees of its own pane's
by a test.

"A bit darker" turned out to be twenty-four and thirty-two points, and the
reason is worth writing down. A line has to stay eight points above the
lightest node body, which puts its floor at 57.4 of lightness; and it has to be
seen on a pane at 90, which puts the lightest usable shade at 58.2. The two
meet, with nothing in between: anything lighter reaches about 1.3 to one
against its own pane and vanishes, and nothing in the gamut reaches 2.5 to one
at all while staying clear of the bodies. So both lines sit at 58.2 -- as light
as a line can be here -- and the panes they are shades of are simply much
paler than they are.

A link line also has to be unmistakable from the fault red, which is the one
colour that means something on its own. The payload line was once a dusty red at
hue 23 -- the fault's own side of the colour space, 27 CIE units from it, which
is "another colour" to measure and "something is wrong with that line" to look
at. It is a teal now, 42 units away, and the ground's line is a blue at 80.
The floor is 35, asked under each of the four ways of seeing, and it is a floor
for lines only. A node body is not held to it, a faulty node being painted the
fault's red whole and named, in words, by the errors button's menu (see `A node
that is wrong`_); a line that is wrong is lit as well as listed (see
`Errors`_).

Getting the tiers cost two units of separation. Every colour of the scheme is
held apart from every other -- the twenty of them, panes and lines and bodies
together, under normal vision and each of the three dichromacies -- and the
floor is now 12 where the bodies alone managed 14. Six bodies moved down to
get under the lines, which took the top off the range they had; the closest
pair left is an olive and a gold at 12.2. Twelve is still above the ten that
means "clearly two colours" and well above the two or three of "the same
colour in two printings".

What was asked for and is not there is a rung per body: each body a step
darker than the last, down the panel. It does not exist. Under protanopia and
deuteranopia a single hue family is very nearly a lightness scale, so ten
payload bodies each far enough from the rest would want ten steps of fourteen
-- a hundred and forty units of lightness out of the eighty-seven there are.
Forcing a ladder drops the floor to about ten, and re-fitting the family to
win it back leaves each colour's name describing a colour it no longer is. The
bodies are a *range* of shades instead, spanning 33 units from the lightest to
the darkest, and what separates any two of them is the floor above.

A line's colour says which stream it carries, and a line is drawn twice as
wide as it was. Both halves of that are the same point: a line is the thing a
reader of a diagram follows, and a grey one two pixels wide left the two
streams to be told apart by which corner each line left -- which is reading a
diagram by its arithmetic.

So a line carrying the payload's own data is of the payload family and one
carrying what the ground sends is of the ground's, each lighter than every
node body of its family -- at least eight CIE units above the lightest, as the
tiers above say. Lighter so that a line is not taken for a node's outline, and
of the family so that the colour says the stream. A line whose stream cannot be
told, which is a line this build does not understand, is grey, and drawn all
the same (``tcs_diagram::look::line_ink``).
``the_palette_runs_from_the_pane_through_the_line_to_the_bodies`` holds both
lines to the panels themselves, so a palette change that made a node lighter
than its lines fails there, and ``no_two_colours_of_the_scheme_look_alike``
holds the two lines apart from each other and from everything else under
normal vision and the three dichromacies, at the floor the palette itself is
held to.

What it costs is contrast against the panes: a line at that lightness is
between two and three to one against the three grounds, where a node is held
to three. The width is what pays for it -- the rule a node is held to is for a
shape, and a five-pixel line is read far more easily than its contrast ratio
suggests -- but it is a trade and it is written down here rather than left to
be noticed.

Lines
-----

A link is drawn as an orthogonal route rather than a straight line -- out of
the source, across, into the target, corners rounded -- and it goes around
whatever is in the way. Out of the source and into the target, and which way
each of those is, is what the terminals decide: the groundward stream leaves a
node's right side and arrives at the next one's left side, the ground being the
rightmost working pane, and the vehicleward stream leaves a node's left side
and arrives at the next one's right side. So a line's first turn is to the left
of a node whose terminal is on its left, and the arrowhead at the end points
the way the line is actually travelling when it arrives. Before there were
terminals a link left every node by the middle of its right edge and arrived at
the middle of the next one's left edge, which made a command from the ground to
a payload drawn to its left run out to the right and all the way back. A line
that runs through a node says there is a connection where there is none, which
is worse than a line that is hard to follow: a reader tracing it arrives at the
wrong node and has no way of knowing. So a route is chosen rather than
computed. A handful of shapes are laid out -- straight through, a vertical run
down the middle or beside each node in the way, and a run along a clear row
above or below everything -- each is checked against every node on the page,
and the one that crosses fewest, turns least and runs shortest is drawn, in
that order of priority. Crossings first because a line through a node is a lie
about the graph; turns before length because two orthogonal routes between one
pair of points are usually the same length, and of two equal routes the one
with fewer corners is the one that can be followed. A line keeps six pixels off
any node it only passes, because a line along a node's edge reads as a line
that touches it. And a route's runs are on the millimetre grid, as the nodes
are -- every vertical run, and every horizontal one but the two level with its
own ends, which are where its terminals put it -- with the lanes a line may be
moved into two millimetres apart, so the lines of a drawing line up with each
other and with the boxes they run between. Where nothing misses everything -- a
target walled in on every side -- the best available is drawn anyway: a link
with no line is a connection that does not appear in the drawing at all.

Lines are also laid clear of each other. Two lines that *cross* are two lines:
the crossing is a point, and a reader follows each one through it. Two lines
that run along the same track are one line on the screen -- whichever was
drawn second is the only one anybody sees, and a link that is invisible reads
as a link that is not there. So each route is laid against the routes already
laid: every column and row is offered in lanes either side of itself, seven
pixels apart (a line is five wide), and a shared run costs more than a bend.
Two lines a lane apart are two lines, and are not counted as sharing a run.

Lines with the same two ends -- the same terminal of one node to the same
terminal of another, as a subdiagram's readings all go to the gateway -- are
a bundle, and run side by side as closely as they can without lying along each
other. Each after the first is offered the first one's route moved over a lane,
or two, up to four, either way, stepping across to it just past each end; and
it takes the nearest of those that crosses no node and shares no run, before
any route of its own however fewer bends that has. A bundle whose lines went
each its own way read as so many unrelated lines, spread over the page. The
order the router weighs things in is now: nodes crossed, then length shared
with another line, then whether a line of a bundle keeps beside the rest, then
bends, then distance.

A line's label is written at the middle of its longest run, and the lines of a
bundle have their longest runs side by side, so their labels would be written
over each other. A label that would cover one already written moves along its
own run instead, its length at a time, to the first place that is clear.

tcs-gui writes neither a line's label nor its comment on the sheet. Moved clear
of each other, labels still ran over the next bundle and over nodes, and a
comment a paragraph long, the thermostat's over its two switches, was drawn
across the log. A pointer resting on a line is shown a tip instead, over
everything and at the window's own size whatever the zoom: the node the line
leaves, the node it reaches and the input there where it names one -- which
is what the labels said, the far node of a line to another page included --
and under them the comment, whole, each paragraph run together to be wrapped
at the tip's width. The tip is each line's own, so every line of a list a
comment is written over says it, where the drawing wrote it on the first. The
line a tip is about is found as a click finds one, so it is the line a click
there would select. tcspecial's visual window, which nobody points at, still
writes the labels and comments beside the lines.

Two overlaps are not the router's to fix, and both are short. Two links
leaving one terminal leave from the same point, so their first segments run
together for the length of a stub whatever either does afterwards; the same
goes for two links arriving at one terminal, which the ground station invites
by merging. At that length it reads as one line dividing rather than as a line
gone missing, so a shared run shorter than a stub is not counted at all.

What is left is counted and budgeted. On the worked example the lanes take the
overlap from a hundred and seven lane-widths to twenty-two -- four fifths of it
-- and the remainder is columns with nodes either side and no lane free. The
test states the twenty-two as a budget rather than asserting nothing, so that
a change which brings wholesale overlap back fails, and so that the day the
router learns to clear the rest the number comes down rather than the rule
being quietly satisfied by a simpler drawing.

Working on the Canvas
=====================

Selecting, menus and deleting
-----------------------------

A right click on a node opens a menu of what there is to do with it: **Edit**,
which opens the window of what the node says, and **Delete**, which asks before
anything goes -- with **Open** above them on a subdiagram's box. A node is not
in the menu bar -- that is for the configuration as a whole -- and it has
nowhere else to put its own commands, so the node itself is where they are asked
for. Edit is first because it is what nearly every menu is opened for.

The right button opens it, as it opens a menu in every other program: a left
click opening one was a menu nobody expected, under a hand that meant to select
or to drag. A right click selects the node too, so the node the menu is about is
the node drawn as chosen while it is open. A left press selects the node and
starts a drag -- a node somebody has started to move is the node they are
interested in -- and a left click that moved nothing does no more than select:
it opens no window, a node's window being opened by the menu's Edit. The grip
that takes a drag covers the whole node and is above everything, so it takes
every press on it and Slint's ``ContextMenuArea`` underneath never sees one; the
grip shows the menu by name.

A click of either button selects: a node, a physics component or a line,
outlined in the dark grey a selected node always was, a line by a wider line
under it so that it still says which stream it carries. One thing is selected on
a page at a time, and the Delete key, or Backspace, takes it -- a line at once,
a node after asking, as its menu's Delete does. A right click on a line selects
it and opens a menu of its own, whose one item is Delete. A click of either
button anywhere else on the page selects nothing, and closes whatever menu was
open. A line is a ``Path``, and a press on a path is not reported, so the press
goes to the page underneath, which asks which line runs within a few pixels of
the point, by the line's points rather than its drawing: see
``tcs_diagram::route::distance_to``. Only the document's lines are found, not
the ones an edge draws to a node nothing joins outside yet, there being no line
to take out. A selected line is kept by what it joins rather than where it is in
the page's list, so an edit elsewhere that redraws the page keeps it selected,
and one that takes it away selects nothing.

"Stayed put" is within three pixels, not exactly nowhere. The test was that
the release landed on the very pixel of the press, which a hand on a mouse does
not manage: every click carried a fraction of a pixel, every click was read as
a drag, and the menu was unreachable by the gesture meant to open it. The same
slack keeps a trembling click from recording a one-pixel move as an edit.

Delete asks, with the node's name and what goes with it: "2 lines are attached
to it and go with it." Every line to it or from it goes, a node of a
subdiagram's as well as one of the top file's: a line left naming a node that is
not there would be a configuration refused when it is loaded. Undo would put all
of that back, a removal being one edit, but an undo somebody has to discover is
no answer to a node that has already gone and taken four lines with it -- and
the press that deleted it is one key away from the press that opens its
attributes. Cancel leaves the document exactly as it was, the history included:
a question answered no is not an edit.

Placing a node
--------------

A node placed from a palette is named after the entry it was placed from -- the
type, or for a described variant of one the description's own name, so a vote is
``vote-1`` and not ``compute-1`` -- a hyphen, and the first number that is free:
``i2c-1``, then ``i2c-2``, and ``i2c-1`` again if the first one is deleted --
the number says which of them this is, not how many have ever been placed.
Numbered from the first rather than from the second, which is what it used to
be: a bare ``i2c`` and then an ``i2c-2`` is two kinds of name for one kind of
thing, the first one's name becomes the odd one out the moment a second is
placed, and a diagram of type names reads as a diagram of types rather than of
nodes. A number on every one of them also says plainly that the name is the
editor's suggestion rather than a description. The ground station a new
configuration is given is named the same way, ``gateway-1``, by the same
function, because a name the editor chose should look the same whichever of its
hands chose it.

Nothing the editor places itself covers a pane's own name. The three names are
written across the top of the sheet -- "Gateway", "Vehicle", "Payloads", one per
pane at the same height -- and they are what a pane has to say when it is empty
or when somebody has lost their bearings. A node somebody *drags* over one is
their own business, the sheet being their drawing; a node the editor put there
would be the editor covering a name nobody asked it to cover. So the strip they
sit in is reserved against automatic placement: ``pane::NAMES``, four pixels
down and sixteen tall, across every pane rather than three strips, because a
name belongs to a pane and the panes' edges move with their dividers. Sixteen
for a ten-pixel face is the line box and a pixel, so a node's border does not
sit on a descender -- minimal in the sense asked for, encompassing the three
names and nothing else.

It is reserved for everything the editor places, not only for a node placed by
hand: arranging the whole canvas goes through the same placing, and the ground
station a new configuration is given is held clear of it by an assertion at
the constant. A test reads the names' own geometry out of ``main.slint`` and
holds the reserved strip against it, so a name moved down the sheet cannot
leave the strip reserved where nothing is written.

A new node goes as high as it will fit in its own pane without covering
anything, and in view: the highest free place in the part of the sheet the
window is showing, or failing that the lowest free place above it, or the
highest below -- and then the sheet is scrolled so the new node's top is at the
top of the view. A node placed where it cannot be seen is a press that did
nothing as far as the person who made it can tell.

Drawing a line
--------------

A link is drawn in three clicks: a terminal, which takes a line in hand; the
node it is to reach, anywhere on it, which offers what the line can be made
into there; and one of those offers. The terminal is where a line is taken
*from*, and it is the only handle there is: there were two round handles on
every node -- one on each side, meaning "a link leaves here" and "a link
arrives here" -- and they are gone, because a node has two places a link may
leave and two it may arrive at, and a handle in the middle of an edge could not
say which of them was meant. So the boxes a reader looks at to see what may be
linked to what are the boxes a person presses to link them, which is one thing
to learn instead of two.

Clicks, and not a drag held from the terminal to the node, which is how a line
was drawn before. A click leaves the hand free between the two: to scroll the
sheet to a node that is out of view, which a drag cannot do without a second
hand on the wheel, and to rest, which a drag across a tall
sheet does not allow. The line in hand is put down by Escape, by a right click,
by Cancel in the menu, or by a click on bare sheet, and none of them changes
anything. While a line is in hand, a box along the bottom of the sheet says so,
what to click next and how to put it down: a gesture that spans three clicks
has to say it is still going on, or a click on a node that asks about the line,
instead of opening the node's own menu, would look like a fault.

The node is clicked anywhere -- its name, its middle, a corner box, the wrong
corner box -- and the menu there, headed with the node's name, lists the
inputs of the stream the line carries: a stream has one name at each of its
ends, so a line taken from a ``world-out`` can only arrive at a
``world-in``. Into a node that declares its inputs, as a vote does, the menu
lists each of them, ``in0``, ``in1`` and so on, because which input a reading
arrives at is the whole of what such a line says: a choice, and not the next
free one guessed at. Taken from an input, the line goes the other way, and the
menu offers the node's output of that stream.

What cannot be made is in the menu too, greyed, and saying why in words, so
that the grey is never the only thing that says so: ``world-in (fed by
to_k1)``, an input something else feeds already; ``world-in (joined
already)``, a line the configuration has. A menu that left out the input
somebody was looking for would leave them wondering whether they had the right
node; one that names what is in the way answers the question they were about
to ask. A node with nothing at all for the line -- the ground is sent nothing,
so a line carrying what the ground sends has nothing to reach on a gateway
node -- offers no menu, and the line stays in hand with the box on the sheet
saying so, which is a kinder answer to a near miss than throwing away a line
that took a click to pick up. The rule is still
``tcslib_protocol::graph::terminals``; the menu only says it.

A link that is already there can be picked up by either of its ends. Click the
terminal it is attached to and the line comes off the drawing and into the
hand; the node clicked next offers the pressed end moved there, to its terminal
of the same name -- a ``world-in`` taken off one node goes on another node's
``world-in`` and nowhere else -- the far end staying where it is, in one
edit, so one undo puts it back. Beside that, the same menu offers the new line
the terminal could draw instead, because a terminal with one link on it is
ambiguous: the gateway node already feeding one payload, its ``moc-out``
clicked and then a second payload, is offered a second line to that payload,
the payload having no ``moc-out`` for the picked-up end to move to. Clicked
back on the node it was picked up from, the menu offers to take the line out,
and so does the Delete key while it is in hand. Those are the two ways to
remove a line by hand; letting go of it over empty canvas was the way when a
line was dragged, and a click on bare sheet now only puts the line down.

A terminal with one link on it hands that link over; one with none starts a new
one, and so does one with several, because nothing says which of several was
meant.

While a line is in hand, it follows the pointer: a dashed line from the end
that is staying put to wherever the pointer is, erased and drawn again on every
movement. Without it a person aims at nothing, and the line in hand is what
says the gesture has started at all. While a node's menu is asked, the line
stays where that node was clicked, so the hand can go to the menu without the
line following it there.

Dashed because it is not a connection yet, and straight where a link's own
route is orthogonal: this one is being aimed, and a route that turned corners
as the hand moved would jump about under it. The dashes are a run of short
subpaths, a path in this toolkit having no dash pattern of its own, and the
last of them ends under the pointer rather than wherever the pattern stopped:
the tip is what somebody is aiming with. Which end stays put is the terminal
the line was taken from when a line is being drawn, and the *far* end when a
line is being carried by one of its ends -- that end is in the hand, and the
other one is what it still hangs from.

Only the lines are redrawn, never the nodes. Rebuilding the nodes would
destroy the element holding the pointer and take the rest of the press with
it -- which is a fault this window has made once already, when the same press
that chose a node rebuilt the canvas under it.

A link may be drawn either way about: from the data source that feeds it or
from the data sink that wants feeding. It is the same link, and the file says
the same thing -- a person drawing a diagram works in whichever direction they
are reading it, and an editor that insisted on one would be an editor to be
remembered rather than used.

The gesture will not make a link that the configuration check would refuse:
nothing from a node to itself, nothing that carries one stream into the
other, nothing the configuration has already, and nothing into an input
something else feeds unless the node merges, as the ground's does. An editor
that cannot make a bad link is worth more than one that makes it and marks it
red -- and these refusals are the ways a person's hand slips, not ways of being
wrong about the vehicle.

While a line is in hand, every click on the sheet is about the line. A layer
over the whole sheet takes the clicks and the pointer's movement, and which
node is under a click is worked out in Rust from where the nodes are, with the
rest of the geometry, where it can be tested without a window. The keys that
put the line down or take it out are taken outside the sheet: taking the focus
inside a scrolled view scrolls it to whatever took it, and a sheet that jumped
when a line was picked up would throw the node somebody was aiming at out from
under the pointer.

A terminal is drawn inside its node and over the area a drag is taken from, so
a press on a corner takes a line and a press anywhere else moves the node. That
is the one thing the arrangement costs: a node can no longer be dragged by its
corners.

Subdiagrams
-----------

tcs-gui opens each included file as a subdiagram under its name, keeps its
path as the box's ``path`` attribute, and writes each node, group and command
binding back to the file it was read from. A subdiagram is renamed as a node
is, its box being its name: its nodes, the lines to them and its exports go
with the name, and its file stays where its path says.

A line that crosses into or out of a subdiagram is drawn to the subdiagram's
edge on both sides of it -- see `Configuration
<tcspecial-design.html#configuration>`__ for ``exports``, which says what of an
included file may be joined from outside it. On the page the box
is on, the line runs to the box's corner terminal of the same name as the
inner node's, and is labelled with the inner node; the box's four terminals
are its edge, and what is behind each is named on the lines. Inside, the
subdiagram's page has a margin either side of what is inside it, pale and
labelled *outside*, holding the four terminals again where the box has them:
``world-in`` and ``moc-out`` on the left, ``world-out`` and ``moc-in``
on the right. A line from another file runs to the edge terminal facing its
own end and is labelled with the node outside it reaches. A node the file
exports that nothing outside joins yet is drawn joined to the edge all the
same, its line labelled *not joined outside*, because the edge shows what may
be joined at it and a node shown there with no line would look like one that
is not. In the margins rather than over the panes, which are where nodes go:
an edge drawn over the payload pane's first column would be drawn over the
payloads in it. A line between two subdiagrams is drawn between their boxes on
the top page, and to the edge on each page.

The Physics pane is inside the edge, left of the payloads, as it is on the top
page: the left margin is at the page's far left, then the Physics pane, then the
working panes. It holds nothing of the subdiagram's file -- the physics model is
the simulator file's -- but it is drawn against the payloads it joins, and put
past the edge a line from a sensor to the zone it reads would cross the edge
without leaving the page. A line from outside runs from the left margin across
the Physics pane to its node, its end at the Physics pane's left edge
(``tcs_gui::inside_of``).

The same three clicks join across the edge. Clicking a subdiagram's box with a
line in hand offers a line to each node its file exports at that terminal, by
name, greyed as any offer is greyed where it cannot be made; and a line taken
from one of the box's terminals offers, at the node clicked next, a line from
each exported node there. A box that exports nothing at that terminal offers no
menu, and the window says to open it and join a node to its edge. Inside, an
edge terminal is clicked as a node's terminal is: then a node of the page,
or a node's terminal and then the edge terminal of the same name, offers to
show that node outside there, or, where it is shown, to hide it again. Hiding
is greyed while a line from another file joins the node there, saying which:
the line would be one the checking refuses. Showing and hiding are edits like
any other, each taken back by one undo, and written into the file's
``exports`` as they are made.

And a line to a node outside is drawn from inside too, which is how a gateway's
data and the MOC's commands reach a node of a subdiagram without leaving its
page. The same menu, at the edge terminal or at the node, offers a new line to
each node of another file that has the terminal answering the one in hand --
``New line to ground's world-in`` -- greyed as any offer is: every node of the
top file, and a node of another subdiagram where that file shows it at that
terminal. Choosing one makes the line, and shows the node outside at that
terminal in the same edit where it was not shown yet, so one undo takes back
both and no line is made that the checking would refuse.

A subdiagram is opened by **Open** in its box's menu, or by the mark in the
middle of its bottom edge, and not by a double press, whose first press would
already have opened the menu. Its page is drawn in a window that is the main
window -- the same component, furnished and wired the same way, the title
saying which subdiagram it draws -- so every page has the same panes, the same
selection lists, the Physics pane and its list among them, the same bar with
its counts and the same menus and edits: what can be done on one page can be
done on every page, because there is nothing a second kind of window could
leave out. The world is one, the simulator file's, and each of its components
is drawn on one page: the page it was placed from, which the simulator file's
``tcs-gui`` section records under ``pages`` -- ``thermostat-common: [zone]`` --
and the top page for one it names under none. A component belongs with the
payloads it stands beside, and a zone made for a subdiagram's sensors drawn on
every page would be a box on the top page that nothing there is joined to. A
connection or join to a component on another page is drawn on neither, as one
to a component the world has not; names are the file's, whatever the page, so
two components on two pages still cannot share one. Several pages may be open
at once and cannot disagree, there being one document behind all of
them, and each window is redrawn from it after every edit. Opening a page that
is open brings its window forward.

Renaming
--------

Renaming one of two nodes that share a name is done **by position**, which is
the one thing ``Edit::RenameNode`` cannot express: a name is how an edit says
which node it means, so an edit cannot mean "the second of the two". It is also
not recorded in the history, which is the decision worth stating. A duplicate
is the file being unreadable rather than somebody's drawing, and an undo would
put it back -- which, with Autosave on, would write a broken
configuration to the disk. Undo takes back what was drawn; this was not drawn.
The repair is written at once instead, Autosave on or off, so the document and
the file never disagree about a name.

Any node of a working pane, on any page, is renamed in its own window, whose
name is a field at the top: renamed when the name is done with, as a value is
taken -- on Enter, on leaving the field, on Close or when the window is closed
-- rather than at each letter, a name being what every line to the node is drawn
by. The node is drawn under its new name at once, and the window stays where it
is, the renamed node's from then on. A name any other node has is refused, in
any file of the configuration -- the reader allows no two nodes one name across
the top file and its includes, and a line names a node by its name alone -- and
so is a physics component's, the simulator file joining payloads to components
by name, and so is no name, or one with a dot or a slash in it, which a line and
a page read as separators. What is refused is said under the field and nothing
changes, and the window will not close on it until the name is put right or
cleared, a name cleared being the one it had. A rename is one edit, taken back
by one undo, and takes the node's lines, its simulator settings, the command
bindings that name it -- as the gateway a command arrives on or the node it sets
-- and its showing at its page's edge with it; a node of a subdiagram keeps its
page. A subdiagram's box is renamed the same way, which renames the subdiagram
-- its page, the lines to its nodes and its exports go with the name -- and
leaves its file where its ``path`` says.

A physics component is renamed as a node is, in the physics window, by a name
field over its settings, and held to the same rules: no name any node or other
component has, and no dot, which a simulated payload's ``connects`` reads as
separating a component's name from a terminal's. The rename is one edit, and
every connection, every simulated payload's and group's ``connects``, and its
place in the Physics pane say the new name.

The links stay with whoever keeps the name. A link names a name: while two
nodes shared one, every link naming it was ambiguous, and renaming one of them
does not make the link mean the other. When the last holder of a name is
renamed there is no ambiguity left and the links follow, as they do for any
other rename.

And renaming a payload node carries its simulated entry with it, so the editor
cannot be the thing that breaks the join.

Arranging the nodes
-------------------

**Arrange**, on the bar and as **Edit -> Arrange the nodes**, lays every node
out afresh, the way its data flows, each page on its own and each node in its
own pane. The payloads stand in a column down the left of theirs; the
vehicle's nodes in columns by how many steps of readings lie between each and a
payload, so what a conversion feeds stands to the right of the conversion; the
ground in a column of its own. A vehicle pane too narrow for every step holds
the later ones in its last column. Within a column the nodes are in the order
of what feeds them, so the lines between two columns cross as little as they
can, and each node is level with what feeds it where there is room -- a
sensor beside its own conversion -- so a line between them runs straight. All
of it from the top of the sheet, clear of the pane names and of each other, on
the node grid, and the view back at the top. The sheet is 960 by 2400 and a
window shows a few hundred pixels of it, so a configuration read out of a file
-- or one left spread about by a morning's dragging -- can have most of itself
outside the window with nothing to say so. This is the one press that brings it
back, and a second press moves nothing.

The button is emphasised, and says why in words -- "Arrange: 2 nodes are
outside their panes" -- while any node on the page lies wholly or partly outside
its own pane across: a node overhanging a divider somebody dragged, which is
how a node comes to be off the side of the window. Down the sheet nothing is
outside, the sheet scrolling, so nothing there is counted. On the bar and not
only in the menu because the time it is wanted is when nodes have gone out of
sight, and a command in a menu is one somebody has to know to look for.

It is one edit, not one per node, so one undo puts a hand-made layout back:
somebody who arranged a diagram carefully and then tidied it by accident must
not have to press undo eleven times and get it half right. Nothing is recorded
when there was nothing to move, because an undo that changes nothing is an undo
that looks broken. And it is an edit, which `Document::settle` deliberately is
not -- settling puts a node where the editor would never let one be dragged,
and arranging puts it where anybody could have dragged it.

What Is Wrong, and What Might Be
================================

Concerns
--------

Things that might be wrong are counted on a button rather than refused.

A **concern** is a shape that might be a mistake and might be meant: a payload
whose readings reach nothing, a vehicle node a stream arrives at and nothing
leaves, one something leaves and nothing arrives at. None of those stops a
configuration being read, and every one of them is the ordinary state of a
diagram halfway through being drawn -- so an editor that called them errors
would be an editor crying wolf at its own user, and one that said nothing would
leave a sensor nobody reads to be found in flight.

So the bar carries two buttons, and only when there is something to count:
*"3 concerns"* and *"1 set aside"*. Each opens a list with a line per concern,
a **Go to it** that chooses the node, scrolls the sheet to it and opens its
window, and a button that moves it between the two lists. Two buttons because
they are two questions: "is there anything I have not looked at", and "what did
I decide to leave" -- a decision nobody can find again is a decision nobody can
change.

Which rules apply to a node depends on which pane it is in, because that is
what the data flow is: a payload is where the payload's own data begins and a
gateway link is where it ends, and the other stream runs the other way. A
payload's ``world-out`` with nothing on it is a concern -- those readings
reach nobody -- where a computation's is not, a computation being free to send
its result the other way or to be read by nothing yet. What is asked of a
vehicle node is that a stream passing through it comes out somewhere, which is
the shape of the whole graph. A log is the exception by having no data source at
all: it is where a stream stops on purpose.

Setting one aside is an edit, so undo takes it back, and it is written in the
file's own ``tcs-gui`` section beside where the nodes were drawn: two people
editing one configuration should not each have to decide again that a payload
nobody reads yet is on purpose. It is remembered by node and kind, so a concern
that stops applying leaves both lists by itself and comes back if the line it
was about is deleted -- nothing has to be tidied up after drawing. The list,
while it is open, is worked out again after every change -- an edit, an undo,
a redo, edits put back after a crash -- as the errors' menu is (see
`Errors`_): a list that went on showing what the drawing no longer has would
be a list of decisions about nothing.

What ``tcs-verify`` warns of is a concern too, in its words and under the same
rules: a node with no line on it at all (kind ``unlinked``), said once where
the drawing's own rules already have something to say about that node, and a
node whose required fields are not all filled in (kind ``unfinished``). Both
are warnings to ``tcs-verify`` for the reason they are concerns here: the
configuration has not finished saying something rather than said something
wrong. The vehicle still refuses a file that has them, which is why they are
counted and not left out.

Errors
------

A configuration the editor can read and the vehicle could not fly is **warned
of when it is opened**, briefly: the warning says whether some nodes have
attribute values that are not valid, whether some links are not valid, and
whether there are other errors -- a group, the ``tcspecial`` parameters, the
simulator file -- and **not each error**. Each is in the errors button's menu,
and a list read once at startup is a list nobody has to hand when they come to
put the errors right. The warning has two buttons: **Ignore**, which takes it
away and leaves the errors as they stand, and **Quit**, which leaves the
editor. The errors are the editor's own faults and every error ``tcs-verify``
finds in the files, in its words: one about a node names the node, which
choosing it goes to, and any other starts with the file's name and the line
-- ``battery-sim.yaml:97:`` -- and not the path the session was given, which
would fill a menu's line before it said anything. ``tcs-verify``'s *warnings*
are not errors. They are what an unfinished configuration looks like -- a
node joined to nothing, a node whose required fields are not all in -- which
is every node the moment it is placed, and an errors button that counted
them would say "2 errors" about a node nobody had had the chance to fill in:
the editor would be crying wolf, against `The Editor Is Lazy`_. They are
concerns, and counted there (see `Concerns`_). So every node drawn red is in
the errors' menu, the red keying on the editor's own faults (see `A node
that is wrong`_), and nothing in the menu is something not yet done.

**One button finds every error.** While the configuration has any, the bar has
a button saying how many -- *3 errors* -- whose menu lists them, one line each,
attribute values and links alike. Choosing one goes to it:

* a node's -- a value a rule refuses, a type this build does not know, a group
  nobody declared -- is shown in the middle of its page, chosen, with its own
  window open, so the value can be put right where it is;
* a line's -- one that goes nowhere, one that goes back where it came from, one
  of two into a single input -- is shown in the middle of the page it is drawn
  on and **lit**, drawn over a wide pale line as tcspecial's visual window
  lights a line data crosses, so it is told from the others by its width and
  not by colour alone;
* a name two nodes share, or a node with no name, is first given a name
  nothing has -- the one repair the editor makes by itself -- and then shown as
  a node's is, so its window can be opened and the name made a better one;
* a group's opens the groups window on that group, and the parameters' the
  parameters window.

**An error put right leaves the menu.** The errors are worked out again after
every edit and when a node's window is closed, so a value mended in the window
is gone from the menu as the window closes, and a lit line is put out, and
leaves the menu, once what was wrong with it is put right. The button goes
when the last error does.

Ignoring the warning is a decision about the session and not about the file,
which is worth stating. A judgement about a *concern* is written into the
file, since somebody else reading the drawing should see what was decided;
nobody should inherit somebody else's decision to ignore an error, so this one
is forgotten when the editor closes. A fault that goes away and comes back is
raised again, the set being kept by what each one says.

Either way **the node is on the canvas in its own working pane**, which is what
lets somebody look at the drawing before putting it right. A type nothing knows
goes in the vehicle pane, the two others meaning something about what a node
*is* where that type says nothing. An editor that would not show a
configuration until it was correct could not be used to correct one.

And the alert says nothing about the state the file arrived in: what the file
came with is counted as said before the warning is shown. What the alert is
for afterwards is what goes wrong *while somebody works*, which is news.

Alerts
------

Anything that goes wrong is said twice: in red at the end of the bar along the
top of the window, and in a small window of its own that has to be acknowledged.
The line on the bar is the state -- what stopped the last write, in red, and
otherwise, not in red, what the last command did, ``not saved`` while Autosave
is off and there are edits the files do not have, or ``saved`` -- and the window
is the moment it happened. Both, because either alone is wrong: a line is easy
to carry on typing past without reading, and the thing it says is the one thing
in this editor that silence makes worse -- a full device stays full while every
edit after it is one more the files do not have. The window says what failed,
and says that the session is still open and nothing has been lost, because the
useful response is to put the cause right and press Save rather than to start
again.

Something wrong with the *configuration* goes there too, the first time it is
wrong. A fault -- two nodes of one name, two lines into one input, a value a
rule refuses -- is an error: ``tcs-verify`` fails on it and the flight software
would refuse the file. The editor said so in red inside the node it is about
and nowhere else, which is a sentence in a box forty-eight pixels tall, and on
a canvas somebody is not looking at. So it is said in the window as well, with
a heading of its own: "Something is wrong with the configuration" rather than
"Something did not work", because the second is about a command that failed and
there is nothing to press Save about. The red on the node stays, and so does
the sentence in that node's own window; the window is the moment it appeared.

It appears once per thing that goes wrong, not once per edit. With a directory
that cannot be written every edit fails with the same sentence, and a window
that came back on each of them would be a window nobody could type past; the
red line goes on saying it for as long as it is true. A failure that was put
right and then happened again is a new thing, and is shown again. A fault is
remembered by its own sentence and forgotten when it stops being true, which is
what makes "put right and made again" the same as "new". The window is
made at startup and hidden rather than made when it is needed: the moment it is
needed is the moment something has already failed, and a window that could not
be made then would leave nothing to say it with.

**Whatever comes up as the editor starts is in front of the main window, and
the main window takes nothing until it has been answered.** The warning about
the file's errors, and an alert about a file that could not be read, are
given *inside* the main window, as a dialog in front of its drawing, rather
than in windows of their own. A window of its own cannot be kept in front: the
order of windows is the window manager's, and on Wayland no program can ask
for its window to stay on top, so a question shown in a window of its own could
land behind the drawing waiting for it, where nobody finds it and nothing in
front of them takes a press. While the main window waits, a grey veil over the
whole of it greys everything under it and holds its presses, the dialog is in
front of the veil, and the menu items are disabled, the keyboard being another
way to them. The drawing is still there to be read through the veil, being
what the question is about. What goes wrong *while somebody works* is still
said in the alert's own window, the main window not waiting for it. tcs-moc
does the same for the question it asks when tcspecial serves a different
configuration: asked inside its main window, in front of a veil that greys the
panels. Its pop-ups about a command pressed before the station is matched --
the error and the advisory of `On the ground
<tcspecial-design.html#on-the-ground>`__ -- are put there the same way. tcs-moc
and tcs-sim show what they found at startup in a window that stays on top and is
closed before their main windows open.

The Windows
===========

The menu bar and the title
--------------------------

The menu bar holds what belongs to the configuration as a whole rather than to
anything on the canvas -- there is nothing to press on a diagram that would mean
"the configuration", or "the groups", or "the way out". **File** holds Save,
Save as, Autosave, the two kinds of group -- **Edit payload groups** and **Edit
simulated payload groups** -- and Exit; **Edit** holds **Edit tcspecial
parameters** and **Arrange the nodes**; **View** holds how large the sheet is
drawn. Undo and redo are not in a menu but on the bar, where each says what it
would take back (see `Undo and redo on the bar`_). The editor generates
nothing: what it writes is the configuration and the simulator's file.

The main window's title says what it is and the file it is editing -- "TCSpecial
Configuration: /home/someone/working/thermostat/thermostat.xtcs" -- the path
made absolute and changed by **Save as**; a read-only session says "TCSpecial
Configuration (read-only)", and a subdiagram's window adds the subdiagram's name
after the words, before the path. That is where somebody looks to tell two
editors apart, which look alike everywhere else. A path too long for the title
bar is shortened in the middle, as file managers and editors shorten one: its
first element, an ellipsis for what is left out, and as many of its last
elements as fit -- "/home/…/thermostat/thermostat.xtcs" -- and never less than
the first and the last. Shortened by the program and not left to the window
system, which cuts a title at the end, and the end of a path is the file's name.
The title is fitted again as the window is made wider or narrower.

What a node says about itself
-----------------------------

In a window of its own, one per node, opened by **Edit** in the node's
right-click menu -- or by **Go to it** in the list of concerns, or by choosing
the node's error from the errors button -- and closed by a press of the Close
button.

It was a panel down the right-hand side, which showed whichever node had last
been pressed. That made the commonest thing somebody filling in a
configuration does -- comparing two sensors on one bus -- a matter of pressing
one, reading it, pressing the other and remembering. Several windows can be
open at once, so the two can be read side by side, and closing takes a press of
its own: a window that went away when something else was pressed would be the
panel again.

Opened from the menu and not by a click, because a left press is how a node is
chosen and how a drag starts: a left click that moved nothing only selects the
node (see `Selecting, menus and deleting`_). It once opened the window too,
which put a window in front of somebody who had meant to choose a node or move
it. Opening a window is not an edit and leaves nothing in the history.

The window holds the attributes the node's type declares, whatever else the
node happens to hold, its comment, what is wrong with it if anything, and the
simulator's settings for it where there are any -- those with their labels in
italic, which says on every row what the rule above them says once: a reader
who has scrolled past the rule still knows which file they are typing into.
Removing the panel also gave the canvas the 260 pixels it was using, which the
three working panes now have.

Each attribute's ``about`` is shown at the foot of a node's window while that
attribute's field has the focus; the node's own ``about`` is shown there while
no field with something said about it has. At the foot rather than beside each
field, so that the rows stay a row each however long a sentence is, and as
words rather than a box, because what a description says is the tree's to say
and not the node's, and Tab passes it by.

Every field of a node's window is in view at once, none scrolled out of sight:
a field out of sight is one its reader does not know is there, and one Tab
never reaches, the toolkit's order leaving out what is clipped. So each field
is its name beside its box, one row tall: the node's own attributes first,
and under them the simulator's settings for it, which are written to a
different file and named so by the heading over them, in two columns of
their own, there being as many as twenty. Under and not beside: the three
columns side by side made a window wider than its share of a screen, and it
opened narrower than that, so its last column was cut off. The window is as
tall as all of that and never narrower than its widest part, so no field is
cut off at the foot or the side. Tab goes down the node's fields, then each
column in turn. A window whose fields grow,
a complaint appearing over them or a component with more settings shown in
the physics window, grows with them.

The name is a field like any other, taken as a value is, with a small "x" beside
it that clears the field to type a new name from nothing: the one control in a
node's window and a physics component's alike. A physics component's window has
nothing to choose another component by -- it is opened from the component -- and
no button to remove it, a component being deleted from its menu on the canvas as
a node is.

A value typed into a node's field is taken when it is done with: on Enter, on
leaving the field, on Close, or when the window is closed. Until then the field
holds it and nothing else knows of it. Taken a keystroke at a time it was an
edit a letter -- an Undo for each, the file written for each, and the node
judged half typed, its complaint about ``0x4`` appearing and vanishing above
the field and moving the field under the hand. The comment is taken the same
way, on leaving it or closing, Enter there being a new line. A tick box is
one press and is taken at once.

Groups
------

Groups are edited in a window of their own, reached from **File -> Edit payload
groups**. A group is not on the canvas and should not be: it is a set of
payloads read together, and which payloads those are is written on the nodes
rather than drawn between them. The window chooses which group, states its
name, how often it is read, and which payloads are in it -- and below those,
the attributes that *all* of those payloads declare. The chooser is in the
window rather than in the bar, where it was: a list of what a configuration
happens to contain is data, and a control holding data belongs with the form
that edits it.

Typing a value into one of those writes it to every payload in the group, as a
single edit. That is what a group is for beyond the schedule: a dozen sensors
on one bus are told the bus once. One edit and not a dozen, because undo has to
take back the gesture rather than the first of its consequences, and the edit
carries what each payload had so that undoing it puts back exactly what was
there -- including the payloads that had nothing, which have the attribute
taken off again rather than emptied. A field whose payloads disagree says so,
because typing into it will overwrite several different values with one.

Two attributes are never propagated. ``group`` is the group itself, and
``interval`` belongs to the group rather than to its members -- a member
stating one is a fault the flight reader refuses -- so it is the group's own
field and goes nowhere else. Renaming a group renames it on every node in it,
as one edit, for the same reason: a rename that left the members behind would
silently unschedule them.

The tcspecial parameters
------------------------

What tcspecial itself is configured with is edited from the Edit menu, under
**Edit tcspecial parameters**. That is the one part of a configuration which is
about the whole of it rather than about anything in it -- so there is nothing on
the canvas to press that would mean "the configuration", and a command is how it
is reached. The window offers every attribute the section may have, the ones
that must be stated first and marked as such, and the list it is built from is
the checker's own: two lists would drift, and what that would look like is a
configuration the editor called complete and ``tcs-verify`` refused.

A field is updated in place rather than rebuilt. Every character typed is an
edit, every edit a refresh, and a refresh used to replace the model a window's
fields are built from -- which destroys the elements the repeater made from it,
the text box among them, and takes the cursor with it. The symptom was a value
that had to be typed one character and one click at a time, with the focus
underline flickering under each. So a list of the same shape has its rows
updated where they differ and is otherwise left alone, and a list that is a
different list is replaced, the elements not being the same elements anyway.
It is the same rule the canvas already followed for a drag -- rebuild nothing
the pointer is holding -- and `refilled` is where it is written.

The section is read as the words a file holds rather than as the struct the
command interpreter wants, and the strictness applied afterwards. That is what
lets the editor write a section somebody is halfway through filling in: a port
cleared on the way to typing another leaves a file that still opens, says what
is missing, and can be put right, where a file read strictly would not open at
all -- and a file the editor cannot read back is a file it has destroyed. A
parameter somebody clears is written out of the file rather than written empty,
absent and empty meaning the same thing to the checking. The same goes for a
node's attributes, and on the way in as well as the way out: a hand-written
``address:`` with nothing after it is read as a question not yet answered -- as
is ``address: ""`` -- and saved as neither, the line simply not being there.

The simulator's file follows the same rule: a setting nobody has answered is
not written, and a file the editor begins names each payload and nothing more.

The simulator's file
--------------------

The simulator's own configuration is edited from the menu too, where the editor
was given one: tcs-gui takes a second file name, and **Edit simulated payload
groups**, in the File menu, opens the groups of settings it declares. The entry
is greyed when there is no second file rather than hidden -- a menu whose
entries come and go is a menu nobody learns.

Requirement
    The simulator configuration is written with every key that holds a value
    and no key that does not: a setting emptied leaves the file. A file the
    editor begins names each payload of the configuration under its type and
    nothing else, so that it says which payloads there are to configure; the
    windows are where what can be said about them is asked. It was a form
    once, every setting of every payload written empty, and that made every
    file the editor saved a form too.

    Which payloads there are is the configuration's to say, so the entries are
    its payload nodes, by name -- a computation, a log and the ground
    link are not simulated and get none. An entry the file already held for a
    name that is not a payload is written out after them: it is somebody's
    typing, and ``tcs-verify`` is what says a simulated payload names no
    payload.

Requirement
    The simulator's own reader takes a setting written empty as a setting not
    stated. Without that a file somebody left half filled in would be a file
    written to be rejected: an empty numeric setting was a type error. It is
    the rule the editor already states for every field it offers -- absent
    and empty are one thing to the checking, and a cleared
    field is not an answer -- and a value that is there and wrong is still
    refused, naming it.

Requirement
    That second file is YAML, and its name has to end in ``.yaml``. A name
    that does not is refused on the command line, naming the file and saying
    what is wanted, and the session does not start. A simulator configuration
    is a YAML file -- that is what the shipped sets are -- and one spelling of
    it is one thing for a reader to know rather than two. The editor could
    write it as XML, the formats being two spellings of one set of values
    everywhere else here, and does not: nothing wanted the second spelling,
    and a file nobody writes is a format nobody has read back. ``.yml`` is
    refused with the rest, being the same language under a second name.

Nothing of it is on the canvas, and that is not a shortcut. The simulator file
adds settings to payloads that are already drawn and declares groups of
settings for them to share; it introduces no nodes and no connections, so a
drawing of it would be the same drawing with numbers on it. What it is, is two
lists of settings.

One of those lists needs no window of its own. A simulated payload's settings
are about a payload that is already on the canvas, so they are in that node's
own window, below a rule: a window of its own would be a second place to look
for what one node says about itself. The rule is there because the fields above
it and the fields below go to different files, and nothing else would say so.
The settings appear only for a payload and only where the session was given the
simulator's file -- a computation, a log and the ground link have nothing
simulated in their place.

The names are the configuration's and are never invented here: the two files
are joined by name, every payload needs settings in the simulator's file, and a
name there that no payload has is an error rather than a line with no effect.
So a file whose payloads were renamed shows the new names with nothing in them,
which is the truth about it. The groups window offers what the simulator file
declares and can make and remove one, a group being that file's own.

Both files are one session. They are edited together, checked together and must
agree, so one Save writes both and one Undo takes back whichever of them the
last edit touched. Every setting is optional -- what a payload does not state
it takes from the group it names -- so nothing in either place is marked.

``make runsimguiy`` and ``make runsimguix`` open a configuration and the
simulator file beside it, in YAML and in XML; ``TCS_GUI_SIM`` names the second
file and defaults to the first's stem with ``sim`` on the end, which is what
the shipped sets are called. The simulator file is YAML whichever spelling the
configuration is in, that being the one spelling it has.

The Editor Is Lazy
==================

It requires every value to be **valid** and requires none of them to be
**entered**.

An editor that insisted on a complete node before it would let you move on is
an editor that fights a person working out what they are describing: a diagram
is drawn by placing things and filling them in afterwards, in whatever order
the thinking goes. So an empty field is a question not yet answered, not a
fault, and nothing in the window is red for being blank. A field the
configuration will need says so with a mark beside its name, which is
information rather than a complaint.

What *is* entered is held to the rules that have been tested against real
buses. A bus address of ``0xZZ`` is wrong the moment it is typed, an attribute
belonging to another type is wrong on this one, and a beacon address with no
port is wrong as soon as the section is complete -- each in
the words the flight software would refuse it with, because the editor reaches
those rules rather than restating them. A node is put to them once everything
its type must have is there: until then the only thing to say is "this is
unfinished", which the person at the keyboard already knows.

Requiring that it all be entered is ``tcs-verify``'s and the flight software's,
where the complaint can name the file, the line and the attribute::

   vehicle.ytcs:11: "temp_fore" is an i2c node, so it needs the device it is
   reached through: say path
   vehicle.ytcs:13: "gateway" is a network node, so it needs the host it is at:
   say address

That check is new, and the editor's laziness rests on it: before it, a node
that said nothing at all passed every ground check and was found when the
vehicle tried to open it.

The Description Tree
====================

What node types there are is data rather than code in the editor. A node type
arrives as two files: Rust in tcspecial, and a description tcs-gui reads
(``tcs_gui::nodes``). The shipped descriptions are under
``tcs-gui/share/nodes``:

.. code-block:: text

   tcs-gui/share/nodes/
     gateway_cfs.yaml  gateway_fprime.yaml  gateway_radio.yaml
     gateway_yamcs.yaml
     payload/          app_all_pl.yaml  app_pl.yaml  can.yaml ...
                       gpio_in.yaml  gpio_inout.yaml  gpio_out.yaml ...
     vehicle/          app_all_veh.yaml  log.yaml  ping.yaml ...
       compute/        process.xml  thermostat.yaml  to_kelvin.yaml
                       to_value.yaml  vote.yaml

Two trees with one structure, family by family. A description at
``vehicle/compute/vote.yaml`` has its Rust at
``tcspecial/src/node/vehicle/compute/vote.rs``; one at ``payload/i2c.yaml``
has ``tcspecial/src/node/payload/i2c/``, a directory with as many files under
it as it takes. The three kinds of discrete lines, ``gpio_in``, ``gpio_out``
and ``gpio_inout``, are described apiece and are one module,
``node/payload/gpio/``. The four gateways -- ``gateway_radio.yaml``,
``gateway_cfs.yaml``, ``gateway_fprime.yaml`` and ``gateway_yamcs.yaml`` --
sit at the root of the tree because a gateway node has no Rust of its own: it
is the vehicle's end of the link to what it answers to rather than part of the
vehicle, and the code that serves one is the network payload's.

The correspondence is checked both ways by
``tcs_gui::nodes::tests::every_description_has_its_rust``: a description with no
Rust is a node the editor offers and the vehicle cannot run, and a node type
implemented and not described is one nobody can place. The second direction is
asked of the directories the descriptions cover, so that once a family has one
description in it, every file in that family needs one -- which is what keeps a
family from being half described.

Which terminals a type has is declared there, and so is what a node of it may
be told. Both were written in Rust where nobody adding a node type could see
them, and both are still *read* from Rust by the parts that cannot afford to
read a file -- the checking and the flight software have
``tcslib_protocol::graph::terminals``, and a window has
``style::NodeType::attributes`` for a type the tree does not describe. So the
tree is held against those: ``the_trees_terminals_are_the_schemas`` and
``a_description_of_a_whole_type_says_what_the_type_says``. A description of one
*variant* of a type is exempt from the second, since ``vote`` lists what a vote
needs where the table lists what any compute node may say; for those the
description is the authority, a node's window being built from it.

Two trees rather than one was a decision, and the reason is what each tree is
for. The descriptions are data read at run time, installed on a workstation,
and editable; the Rust is compiled into the flight software. A ``.yaml`` beside
a ``.rs`` in ``src/`` would be a file cargo neither builds nor ships, sitting
where everything else is both.

A description is YAML or XML as its extension says, file by file, and nothing
cares which a given node used -- one of the shipped ones is XML for exactly
that reason, ``vehicle/compute/process.xml``. One file per node, in one
spelling, which
``every_node_is_described_once_and_in_one_spelling``, in
``tcs-gui/tests/schema.rs``, says: a node described
twice is two statements of what its terminals are. What it says:

.. code-block:: yaml

   node: vote                 # what the palette shows, and the name a placed
                              # node starts with
   type: compute              # the configuration's own word for the type
   about: One value out of a trio of sensors reading the same thing.
   sets:                      # attributes the editor fills in and does not ask
     function: vote
   payload-sink: true         # which of the four terminals a node of this
   payload-source: true       # type has: the payload's own data in and out,
   gateway-sink: true         # and what the ground sends in and out
   gateway-source: true
   attributes:                # what a node of this kind says for itself
     - name: inputs
       required: true
       about: Which inputs it votes over, comma separated.
     - name: age
       required: false
       about: How old a reading may be and still be voted on.

That is ``vehicle/compute/vote.yaml`` shortened. An attribute may also say its
``kind``, which is what its field is offered as: ``boolean`` is a control with
two states, and ``text``, ``path``, ``whole``, ``interval``, ``timeout`` and
``bytes`` are a box to write in, a kind with a rule of its own naming the
``tcslib-protocol`` rule a value of it is read by (``nodes::KINDS``). Unstated
is ``text``; ``process.xml`` states a ``path``, an ``interval`` and a
``boolean``.

The shape of one of those files has two schemas of its own, ``node.xsd`` and
``node.schema.json``, in ``tcs-gui/share/schema`` and installed with the tree;
they are held against each other and against the reader by
``tcs-gui/tests/schema.rs``.

``sets`` is what turns one node type into several things to place. ``compute``
is one type, and the tree makes it five entries -- ``process``,
``thermostat``, ``to_kelvin``, ``to_value`` and ``vote`` -- each a compute
node with its ``function`` already answered. Pressing ``vote`` puts down a node
called ``vote-1`` that is a vote from the moment it exists, so its window asks
it for a vote's attributes and the checking holds it to a vote's rules. Before
the tree it put down a node called ``compute`` with nothing answered, and a
person had to know that ``function`` was a word and that ``vote`` was one of
its values.

Which is also the rule for the selection panes: **a type the tree describes is
offered as its descriptions; a type the tree says nothing about is offered as
itself.** A page of its own is described by nothing -- what a page holds is
what has terminals -- so it is still offered as itself. A payload, a gateway
and every vehicle node but a computation are each described once, by their own
type's name, so each is offered under that name as before. An editor that
cannot find the tree at all offers every type as itself, which is exactly what
it offered before there was a tree.

What a computation is told is the flight software's own list, and the tree is
held to it: ``tcslib_protocol::node::vehicle::compute::attributes_of`` says what
each computation takes, ``params_of`` refuses anything else, and a test says
each description asks for exactly that list. A description that asked for
something else would be a panel filling in a field the checking then refuses, or
leaving out one it requires.

A file that is not a description is refused on its own and the rest of the tree
is still read: a tree people edit has a half-written file in it sooner or
later, and an editor that offered nothing because one file has a typo in it
would be an editor a typo turns off. What is wrong goes to the error output, as
an unreadable preferences file does. Two descriptions of one name are refused
the same way -- a name picks out one node here as it does in a configuration.

Schemas Beside the Tree
=======================

``tcslib-protocol/schema`` holds a schema for each file tcs-gui edits, in each
spelling: a JSON Schema for the YAML and an XML Schema for the XML, for the
configuration -- YTCS and XTCS -- and for the simulator's file. They are
tcslib-protocol's rather than this editor's, because every program that reads a
configuration reads those formats; they are installed under
``$(PREFIX)/share/tcspecial/schema``. The schemas of the editor's own node
descriptions stay with it, in ``tcs-gui/share/schema``.

What they are for is the one thing an editor cannot do for a file somebody is
writing by hand or with another tool: tell them, in their own editor, that
``payloads`` is a section and ``payload`` is not. A JSON Schema is what an
editor's YAML language server actually reads, and a YAML document is a JSON
document in another spelling, so the pair covers both.

What they check is shape: which sections there are, what an entry may say, and
which words are words. What they do not check is whether a configuration can be
flown -- which attributes a node of a given type must state, whether two
payloads claim one socket, whether a vote's inputs are the ones its links bind.
Those are rules about meaning, they are tested against real buses, and
``tcs-verify`` is the one place they live. A file a schema accepts may still be
refused, and the refusal will say why; the README beside the schemas says this
to whoever installs them.

Two things follow from that division. A node's attributes are the union over
every node type -- a bus address, a baud rate and a setpoint are all allowed on
any node -- because a schema that knew which type takes which would be a second
copy of rules that already exist. And the XSD cannot say that a node's
``group`` appears at most once: ``world-out`` may appear many times, several
destinations being several elements, and XSD 1.0 allows neither repeats nor a
mixture inside ``xs:all``. A node that says a word twice is refused where the
file is read instead, and so is a type written twice in a section. A node's
name is an attribute of its element, which the XSD does require exactly once.
The same holds for a configuration's sections: ``include``, ``payload_groups``
and ``commands`` are lists, repeated, so the ``tcspecial``, node and
``tcs-gui`` sections written twice are refused where the file is read. The
``description`` is held to once by the XSD as well, through an identity
constraint, which can count a section that is a value and not one of child
elements. A simulator file has no list among its sections, so its XSD is an
``xs:all`` and holds every one of them to once.

A schema an editor has to be pointed at by hand is a schema nobody points it
at, so every configuration this project writes names its own. In XML
that is the two attributes every XML tool reads::

    <configuration
        xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
        xsi:noNamespaceSchemaLocation="tcslib-protocol/schema/tcspecial.xsd">

and in YAML it is a comment, there being no other way to say it in YAML::

    # yaml-language-server: $schema=tcslib-protocol/schema/tcspecial.schema.json

Both are written by ``nodes::schema_reference``, which gives the path relative
to the file where the schemas are above it -- which is a checkout, where these
examples live and where the relative path is the one that is true -- and the
installed path where they are not. A file saved anywhere else therefore points
at ``$(PREFIX)/share/tcspecial/schema``, which is where the schemas are on a
machine the program is installed on.

Two consequences. The YAML comment is followed by a blank line, which is what
keeps it a note about the file rather than about the first entry: a comment's
claim on what comes after it ends at a blank line, and that rule is
``read::comments_of``'s. And ``GraphFile`` names those two XML attributes as
fields it reads and ignores, because ``deny_unknown_fields`` is worth keeping:
a misspelled section is one the software silently does nothing about, so every
word a configuration may contain is a word named in that struct.

The simulator's file says the same thing about ``tcspecial-sim.schema.json``,
written by ``sim::to_yaml`` from the path the file is going to. It names no
XSD, the editor writing that file in YAML and nothing else -- the XML spelling
of that schema is shipped for a file written by hand.

Nothing aboard reads any of these references. They are for the person editing
the file, in the same way the ``tcs-gui`` section is.

A schema nothing is checked against is worse than no schema: it refuses files
the software accepts, or accepts what the software refuses, and either way the
person believes the schema. So ``tcs-gui/tests/schema.rs`` holds each against
the lists the software itself reads from -- the section's parameters from
``graph::parameters::ATTRIBUTES``, the simulator's settings from
``sim_config::SETTINGS`` -- rather than against a copy written out there, which
would be the same problem one step further away. Every shipped XML file is
validated with ``xmllint``, and every key of every shipped YAML file is looked
for in the JSON Schema, every shipped configuration and every shipped simulator
configuration is checked for the reference to its own schema and for that schema
being where it says, and every shipped XML file is checked for being well formed
XML at all. That last check found four files that were not: they said ``--``
inside a comment, which XML forbids, and nothing had noticed because
``quick-xml`` reads them anyway and no other tool had been asked to.

The same check found a real fault the moment it was written: an XML comment may
not contain two hyphens in a row, and this project writes ``--`` as an em dash
everywhere -- so a note somebody typed in the editor made the file unreadable
rather than merely different. The writer now folds a run of hyphens to one in
an XML comment, which is a thing the XML spelling does not preserve.

Where the tree is -- the first of these that exists, the schemas being found
in the same place: ``$TCS_GUI_DATA``, then
``$XDG_DATA_HOME/tcs-gui`` (or ``~/.local/share/tcs-gui``), then
``share/tcs-gui`` one level up from the directory the editor's own executable
is in, then ``/usr/local/share/tcs-gui``, then ``/usr/share/tcs-gui``, then
``tcs-gui/share`` and ``share`` under the working directory. The user's own
directory before the installed one, because that is where edited
descriptions belong and where a person's own node goes; the install the
editor is part of before any other, found from the program rather than
named, so that ``/opt/tcs/bin/tcs-gui`` reads ``/opt/tcs/share/tcs-gui`` and
an install under another prefix reads its own; the working directory last,
so that running the editor out of a checkout finds the checkout's own.
``make install-data``, which ``make install-files`` runs, puts them under
``$(PREFIX)/share/tcs-gui``, ``/opt/tcs/share/tcs-gui`` by default:
architecture-independent data belonging to one package, which is where the
filesystem hierarchy standard puts it. ``/usr/local/share/tcs-gui`` and
``/usr/share/tcs-gui`` are where installing put them before everything went
under one prefix, and are still read.

See Also
========

`The TCSpecial design documents <design.html>`__
    What each of TCSpecial's design documents covers.

`The tcspecial design <tcspecial-design.html>`__
    The flight software and what the programs share: the node architecture,
    the configuration file, the link, fault resiliency and the requirements.

`The tcs-gui manual <tcs-gui.html>`__
    Using tcs-gui.

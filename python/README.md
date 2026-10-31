# Python

Python 3 spellings of the Rust libraries a ground station is built on, for a
ground station written in Python. Each package is named after the crate it is
written after, and each module after the Rust it says again:

| Python | Written after | What it is |
|---|---|---|
| `tcslib/beacon.py` | `tcslib/src/beacon.rs` | Listening for beacons: the socket loop, how stale the last one is, the beacons that did not arrive, and whether the configuration it announces is the one this station read. No window. |
| `tcsui/beacon_panel.py` | `tcsui/src/lib.rs`, `tcsui/ui/beacon.slint` | The beacon panel -- the indicator, when the last beacon arrived, and what it said -- in tkinter, with the same colours, labels and placeholders. |
| `tcsui/theme.py` | `tcsui/ui/titled.slint`, the Slint windows' style | The ttk theme every Tk window is drawn in, and the kinds of text a panel holds: a heading, a value, the figure it is read for, a note. |
| `tcslibgs/beacon.py` | `tcslibgs/src/beacon.rs` | The beacon as it goes on the link: six fields of text between colons, written and read by hand, and held to the Rust by the same examples in both tests. |
| `tcslibgs/ping.py` | `tcslibgs/src/node/vehicle/ping/mod.rs` | What a ping and its answer say. |
| `tcslibgs/ready.py` | `tcslibgs/src/ready.rs` | What a harness and tcssim say on tcssim's readiness socket. |
| `tcslib/simulator.py` | `tcslib/src/simulator.rs` | Waiting for tcssim to have made its stand-ins before tcspecial is started. |
| `tcslibgs/configuration.py` | `tcslibgs::graph::parameters`, `graph::order`, `graph::telemetry`, `types::came_from` | What a ground station reads out of a configuration: where the beacons are, where a gateway node is bound, and which port a source's data arrives at. |

And beside the ping example, because it is the example's own part of the
ground station: `examples/ping/ping_ground.py`, written after
`tcslib/src/ping.rs` -- the ground's half of a ping, which tcsmoc pings with.

They use nothing outside the standard library, but for one thing. That has
two consequences:

- A configuration is read in either spelling, by its extension: the XML with
  the standard library, and the YAML with PyYAML, which the standard library
  has no equivalent of. PyYAML is imported only when a YAML file is read, so a
  station reading XML needs nothing else; the two spellings say the same
  thing, and the tests hold them to reading alike.
- The configuration's digest, which every beacon's is compared with, is asked
  of `tcsverify --digest` rather than worked out here: it is taken over what a
  configuration says, and a second statement of that would be a second digest
  on the day the first changes.

The interface digest every beacon carries is worked out with tcs-xtce's
Python runtime, which is not the standard library either but is in this tree,
in `vendor/tcs-xtce/python`. The three modules of it the digest needs are
linked into `tcslibgs/_xtce`, and the XTCE documents it is taken over into
`tcslibgs/xtce`, a link to the crate's `tcslibgs/xtce`: so there is one copy
of each in the tree, and installing the package installs them, and an
installed station needs nothing from the source tree. A beacon itself is
plain text and needs nothing to read.

`pyproject.toml` makes the three packages one distribution,
`tcspecial-ground`, at the workspace's version, which `tests/test_version.py`
holds it to:

```
pip install ./python
pip install './python[yaml]'     # with PyYAML, for YAML configurations
```

The `tcspecial-ground` system package installs them in
`/opt/tcs/lib/python3/site-packages`, a path that names no Python version,
and puts that on python3's path with a `.pth` file: see "Installing" in
docs/manual.rst.

Written twice, a rule can drift, so each one here is held to its Rust:

- `tests/` are the Rust tests said again, with the same figures.
- `examples/ping/tests/agrees.rs` checks that what `configuration.py` reads
  out of `ping.xml` is what the Rust reads out of `ping.yaml`.
- `examples/ping/tests/pings.rs` plays the vehicle with the Rust -- each ping
  answered by the function the `ping` node answers with, each beacon
  serialized as tcspecial serializes it -- against a ground station built on
  these.

`cargo test` runs all three, through `examples/ping/tests/python.rs`, so a
change to the Rust the Python has not followed fails where the Rust change is
tested. To run the Python's own tests alone:

```
python3 -m unittest discover -s python/tests -t python
```

The ping example, `examples/ping`, is the ground station built on them.

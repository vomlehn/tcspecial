# Python

Python 3 spellings of the Rust libraries a ground station is built on, for a
ground station written in Python. Each package is named after the crate it is
written after, and each module after the Rust it says again:

| Python | Written after | What it is |
|---|---|---|
| `tcslib/beacon.py` | `tcslib/src/beacon.rs` | Listening for beacons: the socket loop, how stale the last one is, the beacons that did not arrive, and whether the configuration it announces is the one this station read. No window. |
| `tcsui/beacon_panel.py` | `tcsui/src/lib.rs`, `tcsui/ui/beacon.slint` | The beacon panel -- the light, when the last beacon arrived, and what it said -- in tkinter, with the same colours, labels and placeholders. |
| `tcslibgs/ping.py` | `tcslibgs/src/node/vehicle/ping/mod.rs` | What a ping and its answer say. |
| `tcslibgs/configuration.py` | `tcslibgs::graph::parameters`, `graph::order`, `graph::telemetry`, `types::came_from` | What a ground station reads out of a configuration: where the beacons are, where a `moc` node is bound, and which port a source's data arrives at. |

They use nothing outside the standard library. That has two consequences:

- A configuration is read in its XML spelling. The standard library has no
  YAML, and the two spellings say the same thing.
- The configuration's digest, which every beacon's is compared with, is asked
  of `tcsverify --digest` rather than worked out here: it is taken over what a
  configuration says, and a second statement of that would be a second digest
  on the day the first changes.

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

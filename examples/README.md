# Examples

Each example is a directory holding three files:

- `<name>.yaml` is a tcspecial configuration.
- `<name>.xml` is the same configuration in XML.
- `<name>-sim.yaml` is the tcssim configuration that simulates its payloads.

An example whose payload is a program of its own holds that program too,
beside those files: `simple-app` does, with a Python 3 payload and a Rust
ground station.

| Directory | Description |
|---|---|
| `minimal` | The smallest working configuration: one UDP payload whose telemetry goes straight to the ground (MOC). |
| `thermostat` | Three temperature sensors (I2C and SPI), converted to kelvin, averaged, and fed to a hysteresis thermostat that drives GPIO heaters. The setpoint is commanded from the ground. |
| `simple-app` | A payload that is a program rather than a device: an `app_pl` node sending a line of text one word per packet, and a ground station that prints the line. Two programs of its own beside the configuration: the payload in Python 3, the ground station in Rust. |

A configuration and its simulator file are matched by payload name. Every
data handler in the configuration needs an entry in the simulator file, and
the simulator file can't name a payload the configuration doesn't have.

The YAML and XML files must say the same thing, so change both together.

Two sets of tests find the examples on their own, so adding one means adding
its directory here, and a row to the table above; there's no list in the
tests to update:

- `tcsverify/tests/examples.rs` runs `tcsverify` on each configuration and its
  simulator file (any error or warning fails), and checks that the YAML and
  XML read as the same configuration.
- `tcsgui/tests/schema.rs` validates each XML file against `tcspecial.xsd`,
  checks each YAML key against `tcspecial.schema.json`, and checks that every
  file names its schema (`../../tcsgui/share/schema/...`).

`make` in this directory builds `README.html` from this file, with
`style.html` in its head -- which is where the tables' cells are told to sit
at the top of their row, markdown having no way to say so. `make help` lists
the targets, including the ones that run tcspecial on each example.

## simple-app

The one example that runs end to end on one host, and the one to look at to
see what an app node is. Two programs, both in `examples/simple-app`:

- `lorem_pl.py` is the payload, a Python 3 program that uses nothing outside
  the standard library. tcspecial starts it from the configuration, as
  the `executable` of the `app_pl` node named `lorem`, and runs it with the
  node's name and `app-payload-arg` -- which for this payload is the line of
  the text to send, counting from one. It sends one word per packet and then a
  packet holding the line terminator.
- `lorem_moc` is the ground station, a Rust program in the crate beside the
  configuration. It gets `lorem`'s words from the configuration's `moc`
  node with one function call, `tcslib::moc::Moc::receive`, which works out
  from the configuration which port they arrive at; it writes out each word
  with one space between them, and stops when the terminator arrives.

```
make run-simple-app
```

That checks for Python 3, builds the vehicle and the ground station, starts
the vehicle in the background and the ground in the foreground, prints the
line, and then stops the vehicle. To watch the two separately, run them in two
terminals instead, in either order:

```
cargo build                     # lorem_moc and tcspecial
cargo run --bin lorem_moc       # in one terminal: the ground
cargo run --bin tcspecial -- examples/simple-app/simple-app.yaml   # in another
```

Run the vehicle from the repository's root, where the configuration's
`executable` path starts. Either order works because the payload waits a couple
of seconds before its first word: the vehicle sends each word to the ground
station's address as soon as it has it, and a word sent before `lorem_moc` is
listening is lost.

The text itself is `simple_app`'s library: the first paragraph of the lorem
ipsum text as one array per line, each holding that line's words. Nothing
holds a line terminator; the end of a word array is the end of the line, and
what stands for it on the wire is the one constant both programs read.

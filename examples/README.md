# Examples

Each example is a directory holding three files:

- `<name>.yaml` is a tcspecial configuration.
- `<name>.xml` is the same configuration in XML.
- `<name>-sim.yaml` is the tcssim configuration that simulates its payloads.

An example whose payload is a program of its own holds that program too,
beside those files: `simple-app` does, with a Python 3 payload and a Rust
ground station. An example may hold a ground program without holding a
payload: `beacon` does, its ground station watching something tcspecial sends
on its own account, and so does `ping`, whose ground station is Python 3.

A configuration and its simulator file are matched by payload name. The
simulator file can't name a payload the configuration doesn't have, nor one
the simulator would not stand in for: a payload that is a program, which
tcspecial runs itself. `tcsverify` refuses either, as tcssim does. A payload
that is only driven, such as `tees`' heater, is simulated like any other: its
stand-in counts what the vehicle sends it.

The YAML and XML files must say the same thing, so change both together.

Every payload a simulator file names says `simulate: true` -- itself or in
its group, not both -- for tcssim to stand in for it; `simulate: false` would
leave it to the real device. Every simulator file also has a `tcssim` section
giving the address at which
tcssim says it has made its stand-ins, a port no other example uses, so that
tcsmoc starts tcspecial only once they are there. The `make run-*` targets
give every program `--simulate=all`, the run being one with stand-ins only: a
program started without `--simulate` gets the flight setting, `none`, and
refuses every stand-in. See "Stand-ins or hardware" in the manual.

The examples' programs -- `beacon`, `lorem_moc` and `ping_moc.py` --
are quiet unless given `-v` or `--verbose`: they print what they are for and
any error, and with the flag they also say what they are doing, such as where
they are listening. A tcspecial one of them starts is started `--quiet`, so
that its startup summary does not fill the console, unless the program was
given `-v` itself; whatever stops tcspecial starting is said either way.

They also take `--yaml` or `--xml`, but not both, saying which of the
example's two configuration files to read. With neither, `beacon` and
`lorem_moc` read the YAML and `ping_moc.py` the XML. A file named on the
command line beside one of the two has to be the spelling it asks for, and a
vehicle the program starts reads the same file.

Two sets of tests find the examples on their own, so adding one means adding
its directory here, and a row to the table of its section below; there's no
list in the tests to update:

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

## Simple examples

Each shows one thing at its simplest: a payload's data reaching the ground,
the vehicle saying it is alive, the vehicle answering a ping. Start with
`minimal`, the smallest configuration that does anything.

| Directory | Description |
|---|---|
| `minimal` | The smallest working configuration: one UDP payload whose telemetry goes straight to the ground (MOC). |
| `beacon` | The one thing a ground station knows without asking: a window showing the spacecraft's beacon, built from the two halves any ground station would use -- `tcslib::beacon` to listen and `tcsui`'s panel to show it. |
| `ping` | The ground pings a `ping` vehicle node and times the answer: the round trip, and the two halves of it. The ground station is Python 3, with the beacon panel and the halves it is built from written again in Python under `python/`, and tests that hold them to the Rust. |
| `tees` | The four nodes that do nothing to what passes: a `tee-moc` sends each command on to a heater and back to the ground, a `tee-payload` sends each reading to the ground and on aboard, and a `null-moc` and a `null-payload` drop what they are sent. `tcspecial/tests/tees.rs` flies it. |
| `fprime` | A vehicle that answers to an F´ flight computer rather than to the ground: a `gateway_fprime` node sends each reading to the deployment's bridge component at its `bridge_base`, and takes the bridge's commands on to a heater. `tcspecial/tests/fprime.rs` flies it. |

### minimal

The smallest configuration that does anything: one payload, `sensor`, a
program on this host at `127.0.0.1:5001` that sends its readings over UDP.
Nothing is drawn from it, so its readings go to the ground over the
`tcspecial` section's `payload_telemetry`: tcspecial sends each one to
`dest_base` plus the payload's device id, `127.0.0.1:6100` for `sensor`, which
is device 0. tcssim stands in for `sensor`, sending a packet every second.

```
make run-simple
```

That runs tcsmoc on the example, which starts tcspecial and tcssim itself and
shows a panel for `sensor`: when its last packet arrived, what it said, and
how many there have been.

Languages:

- MOC: Rust -- tcsmoc.
- Vehicle: Rust -- tcspecial.
- Payload: Rust -- tcssim stands in for `sensor`, a network payload.

### beacon

What a ground station can know about a spacecraft without asking it anything.
tcspecial sends a beacon every `beacon.default_interval` for as long as it
runs, over a socket of its own, whether or not anything has connected to it --
so this example commands nothing and still says whether the vehicle is alive,
which build it is, and whether the configuration it is serving is the one the
ground read.

It is also the example of how a ground program is put together, because almost
nothing in it is its own:

- `tcslib::beacon` binds the socket, reads each datagram, counts the beacons
  that did not arrive, compares the digest in each one with the ground's own
  and says how stale the last one is. No window, no toolkit: a ground station
  with no screen -- a logger, a test -- uses exactly this.
- `tcsui` holds the panel, as markup other windows import, and works out what
  to show on it: which colour the indicator is, and the two lines beside it.
- `beacon`, the program here, reads the configuration to find where the
  beacons are, starts the loop on a thread, and puts each pass in the window.
  That is all of it, but for a button.

A beacon is 57 bytes: `tcspecial` and a space, the version's major, minor and
patch a byte each, a space, the configuration's MD5 digest, the beacon's
number and the generation number in four bytes each, most significant first,
a byte whose top bit says a restart has begun, the interface's digest and a
CRC-16. The layout is described once,
in XTCE, in `tcslibgs/xtce/beacon.xml`; tcspecial packs every beacon and
`tcslib::beacon` unpacks it with the code tcs-xtce generates from that file,
`tcslibgs::Beacon`, and this example's test packs the beacon it sends the same
way. tcs-xtce itself is in `vendor/tcs-xtce`.

After the indicator, on its `Status:` row, is the stage the indicator is at,
in a word: `Unheard` before any beacon has arrived, `Heard` while beacons are
arriving, `Aging` once several are missing, and `Waiting` once nothing is
being heard. It changes as the stage does.

The window is white, as the ping example's is, and its button is drawn the
way that window draws its own.

The button starts tcspecial on the same configuration and stops it, so that
the indicator can be watched coming on with the first beacon and going out
after the last. The window opens with it saying `Start tcspecial`, because
tcspecial is not running. Pressed, it starts tcspecial and says
`Stop tcspecial`; pressed again, it stops tcspecial with `kill()` and says
`Start tcspecial`, which is where it began. tcspecial is the binary beside
`beacon`, run directly rather than through `cargo run`, so that the kill stops
tcspecial itself. Closing the window stops it as well, and so does the watcher dying,
which the kernel is told to see to; and a tcspecial that stops by itself puts
the button back to `Start tcspecial`.

tcsmoc shows the same panel from the same library, in a row beside its command
link, which is the point of their being apart: an operator who has read one has
read the other.

The beacon's ports are this example's own -- a hundred above the ones the other
examples use -- so the watcher can be left running while another example is
looked at. It opens a window, so it wants a display.

```
make run-beacon
```

That builds tcspecial and the watcher and opens the window, and tcspecial
starts when the button is pressed. Without make:

```
cargo build --bin tcspecial --bin beacon
cargo run --bin beacon               # beacon.yaml
cargo run --bin beacon -- --xml      # beacon.xml
```

Languages:

- MOC: Rust -- the `beacon` program, on `tcslib::beacon` and `tcsui`.
- Vehicle: Rust -- tcspecial.
- Payload: Rust -- tcssim stands in for `sensor`, a network payload.

### ping

The ground sends a ping up to the `moc` node, which hands it to the `ping`
node; the `ping` node answers with the vehicle's time on the end, and the
answer comes back down. What a ping and its answer say is
`tcslibgs::node::vehicle::ping`:

```
ping #<count> <seconds>.<nanoseconds>                    up
ping #<count> <seconds>.<nanoseconds> <seconds>.<nanoseconds>   down
```

The ground station, `ping_moc.py`, is Python 3 and uses nothing outside the
standard library to read its configuration's XML, the default; with `--yaml`
it reads the YAML instead, which needs PyYAML (`python3-yaml` on Debian and
Ubuntu). Its window shows the beacon panel, the last ping sent and
when, the last answer and when, and three times: the round trip, MOC to
tcspecial, and tcspecial to MOC. The last two are each read across two clocks,
so each includes their offset and may be negative; their sum is the round
trip. Press `Send ping` to send one: it clears the time received until its own
answer, matched by count, arrives.

It is built the way the `beacon` program is, from the same two halves written again
in Python: `python/tcslib/beacon.py` after `tcslib::beacon`,
`python/tcsui/beacon_panel.py` after `tcsui`, and `python/tcslibgs` for the
ping format, the beacon's layout and the configuration. Beacons are unpacked
with tcs-xtce's Python runtime, from `vendor/tcs-xtce` in this tree. `ping_ground.py` here is the ping's own
part, with no window. The Rust crate beside them is the tests that hold the
Python to the Rust; `cargo test` runs them.

```
make run-ping                               # the vehicle and the window
python3 examples/ping/ping_moc.py --pings 3 # no window: three pings, printed
python3 examples/ping/ping_moc.py --yaml    # ping.yaml rather than ping.xml
```

`ping_moc.py` starts the vehicle itself: tcspecial from the build, on the same
configuration. It kills it when it exits, however it exits -- if it is killed
or crashes, the kernel is told to kill tcspecial -- so a vehicle is running
exactly while its ground station is. Build tcspecial first (`make run-ping`
does). To run the vehicle yourself, in another terminal, say `--no-vehicle`:

```
cargo run --bin tcspecial -- examples/ping/ping.xml          # in one terminal
python3 examples/ping/ping_moc.py --no-vehicle               # in another
```

Its ports are all in the seven thousands, which no other example uses.

tcsmoc pings it too: `make runmocy TCSPECIAL_CONFIG_FILE=examples/ping/ping`,
then the `Ping node` button beside `Command`. It is the same panel, kept by
`tcslib::ping`, which `ping_ground.py` is written after.

Languages:

- MOC: Python 3 -- `ping_moc.py`, on `python/`; or Rust, in tcsmoc.
- Vehicle: Rust -- tcspecial, whose `ping` node answers.
- Payload: none. The `ping` node is the vehicle's own, and nothing is
  simulated.

## More complex examples

Each puts the vehicle to work on its payloads: what they send, and what is done
with it aboard and on the ground.

| Directory | Description |
|---|---|
| `thermostat` | Three temperature sensors (I2C and SPI), converted to kelvin, voted on, and fed to a hysteresis thermostat that drives GPIO heaters. The setpoint is commanded from the ground. |
| `simple-app` | A payload that is a program rather than a device: an `app_pl` node sending a line of text one word per packet, and a ground station that prints the line. Two programs of its own beside the configuration: the payload in Python 3, the ground station in Rust. |

### thermostat

A control loop, aboard. Three temperature sensors are read together, every
100 ms, as the group `cabin_temps`: `temp_1` and `temp_2` on the I2C bus
`/dev/i2c-1` at `0x48` and `0x49`, and `temp_3` on the SPI peripheral
`/dev/spidev0.0`. Each reading goes through a conversion of its own to kelvin
-- `k_1`, `k_2`, `k_3`, the parts differing in format and scale -- and the
three meet at `cabin_temp`, which takes their median, so that one sensor
failing cannot take the cabin's temperature with it. `thermostat` holds that
to a setpoint, 293.15 K with a half band of 2 K, and drives the two sets of
heater lines, `heaters_1` and `heaters_2`, GPIO lines 3-5 and 6-8 of
`/dev/gpiochip0`. The ground commands the setpoint, and `housekeeping` logs the
readings and decisions in `/tmp/telemetry`.

tcssim stands in for the sensors with a thermal mass, `cabin`, that all three
read, so their readings agree and the vote produces a temperature. The mass
cools towards 283.15 K, ten kelvin below the setpoint, and what the example
shows is the vehicle deciding to heat it. The heaters are not attached to the
mass: they are GPIO lines, and tcssim cannot yet be driven through one, so the
loop is not closed here. tcssim's own tests close it, with heaters that are
sockets.

```
make run-thermostat
```

That runs tcsmoc on the example, as `run-simple` does. tcspecial opens the
devices, so they have to be there: `make mkstubs`, in the repository's root,
makes stand-ins for the I2C bus and the GPIO lines where there is no hardware.
There is no stand-in for a SPI peripheral -- nothing in the kernel can be one
-- so `temp_3` needs a real one at `/dev/spidev0.0`.

Languages:

- MOC: Rust -- tcsmoc.
- Vehicle: Rust -- tcspecial, whose conversions, vote, thermostat and log are
  its own nodes.
- Payload: Rust -- tcssim stands in for the three sensors, with the thermal
  mass they read. The heater lines are the kernel's GPIO stand-in, or real
  ones.

### simple-app

The one example that runs end to end on one host, and the one to look at to
see what an app node is. Two programs, both in `examples/simple-app`:

- `lorem_pl.py` is the payload, a Python 3 program that uses nothing outside
  the standard library. tcspecial starts it from the configuration, as
  the `executable` of the `app_pl` node named `lorem`, and runs it with the
  node's name and `app-payload-arg` -- which for this payload is the line of
  the text to send, counting from one. It sends one word per packet and then a
  packet holding the line terminator.
- `lorem_moc` is the ground station, a Rust program in the crate beside the
  configuration. It gets `lorem`'s words from the configuration's moc node,
  `moc-1`, with one function call, `tcslib::moc::Moc::receive_into`, which
  works out from the configuration which port they arrive at and puts each
  word in a buffer made before the loop, so that the loop allocates nothing; it writes out each word
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
cargo run --bin lorem_moc       # in one terminal: the ground (--xml for XML)
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

Languages:

- MOC: Rust -- `lorem_moc`, on `tcslib::moc`.
- Vehicle: Rust -- tcspecial, which runs the payload as its `app_pl` node.
- Payload: Python 3 -- `lorem_pl.py`.

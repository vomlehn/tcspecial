# Examples

Each example is a directory holding four files:

- `<name>.ytcs` is a tcspecial configuration.
- `<name>.xtcs` is the same configuration in XML.
- `<name>.xtce` is the configuration's own XTCE: what the vehicle sends the
  ground, each node a parameter of the type it sends. tcs-verify writes it
  from the configuration (`make xtce` writes every example's), and nobody
  edits it.
- `<name>-sim.yaml` is the tcs-sim configuration that simulates its payloads.

An example whose payload is a program of its own holds that program too,
beside those files: `simple-app` does, with a Python 3 payload and a Rust
ground station. An example may hold a ground program without holding a
payload: `beacon` does, its ground station watching something tcspecial sends
on its own account, and so does `ping`, whose ground station is Python 3.

Two examples that are one example with another ground station share what
they have in common from a directory of its own, which is not an example:
`ping-common` holds the ping link and the window that `ping` and
`ping-yamcs` both show, and `ping.ytcs` and `ping.xtcs`, the ping node,
which both configurations include; `thermostat-common` holds
`thermostat.ytcs` and `thermostat.xtcs`, the vehicle both thermostat
configurations include, and `thermostat-sim.yaml`, the simulator file,
physics model and all, which each thermostat example's `-sim.yaml` is a
link to, so that each is found beside its configuration by name. Each
configuration includes its common file as a subdiagram called
`thermostat-common`, at its path from its own directory,
`../thermostat-common/thermostat`, and keeps for itself only what
differs: its `tcspecial` section, its gateway node -- called `gateway` in
both, which is the name the common file's links send to -- and, for the
thermostat, the three sensors, whose ports differ. A common file is not
checked on its own, having no `tcspecial` section; it is checked as part
of each configuration that includes it. A test holds the links to the
shared simulator file:
`examples/thermostat-yamcs/tests/shared.rs`. `yamcs-common` is not an
example either: it is the Rust the tests of the examples through Yamcs share.

A configuration and its simulator file are matched by payload name. The
simulator file can't name a payload the configuration doesn't have, nor one
the simulator would not simulate: a payload that is a program, which
tcspecial runs itself. `tcs-verify` refuses either, as tcs-sim does. A payload
that is only driven, such as `tees`' heater, is simulated like any other:
tcs-sim counts what the vehicle sends it.

The YAML and XML files must say the same thing, so change both together.

Every payload a simulator file names says `simulate: true` -- itself or in
its group, not both -- for tcs-sim to simulate it; `simulate: false` would
leave it to the real device. Every simulator file also has a `tcs-sim` section
giving the address at which
tcs-sim says it has made its simulated devices, a port no other example uses,
so that tcs-moc starts tcspecial only once they are there. The `make run-*`
targets give every program `--simulate=all`, the run being one with
simulated and dummy devices only: a program started without `--simulate`
gets the flight setting, `none`, and refuses every simulated and dummy
device. A `network` or `device-file` payload is a real device under any
mode, whether or not tcs-sim answers at its far end. See "Simulated, Dummy or
Real Devices: --simulate" in the manual.

The examples' programs -- `beacon`, `lorem_gateway` and `ping_gateway.py` --
are quiet unless given `-v` or `--verbose`: they print what they are for and
any error, and with the flag they also say what they are doing, such as where
they are listening. A tcspecial one of them starts is started `--quiet`, so
that its startup summary does not fill the console, unless the program was
given `-v` itself; whatever stops tcspecial starting is said either way.

They also take `--yaml` or `--xml`, but not both, saying which of the
example's two configuration files to read. With neither, `beacon` and
`lorem_gateway` read the YAML and `ping_gateway.py` the XML. A file named on the
command line beside one of the two has to be the spelling it asks for, and a
vehicle the program starts reads the same file.

Two sets of tests find the examples on their own, so adding one means adding
its directory here, and a row to the table of its section below; there's no
list in the tests to update:

- `tcs-verify/tests/examples.rs` runs `tcs-verify` on each configuration and its
  simulator file (any error or warning fails), and checks that the YAML and
  XML read as the same configuration.
- `tcs-gui/tests/schema.rs` validates each XML file against `tcspecial.xsd`,
  checks each YAML key against `tcspecial.schema.json`, and checks that every
  file names its schema (`../../tcslib-protocol/schema/...`).

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
| `beacon` | The one thing a ground station knows without asking: a window showing the spacecraft's beacon, built from the two halves any ground station would use -- `tcslib_moc::beacon` to listen and `tcs_ui`'s panel to show it. |
| `ping` | The ground pings a `ping` vehicle node and times the answer: the round trip, and the two halves of it. The ground station is Python 3, with the beacon panel and the halves it is built from written again in Python under `python/`, and tests that hold them to the Rust. |
| `ping-yamcs` | The ping example with Yamcs between the ground station and the vehicle: a `gateway_yamcs` gateway puts a CCSDS primary header on every packet to and from the ground, the beacon included, and `ping_yamcs.py` runs Yamcs on tcspecial's own XTCE and pings through it from the ping example's window. Needs a Yamcs release unpacked somewhere. |
| `tees` | The four nodes that do nothing to what passes: a `tee-gateway` sends each command on to a heater and back to the ground, a `tee-payload` sends each reading to the ground and on aboard, and a `null-gateway` and a `null-payload` drop what they are sent. `tcspecial/tests/tees.rs` flies it. |
| `fprime` | A vehicle that answers to an F´ flight computer rather than to the ground: a `gateway_fprime` node sends each reading, the temperature a `to_kelvin` node makes of it and what a `thermostat` node decides of that, to the deployment's bridge component at its `bridge_base`, and takes the bridge's commands on to a heater. `tcspecial/tests/fprime.rs` flies it, and `make fprime-test` runs the bridge itself, in `fprime/`, against it. |
| `cfs` | A vehicle that answers to a cFS flight computer: tcspecial joins its software bus as an SBN peer, a `gateway_cfs` node publishes each reading as its `tlm_base` plus the reading's device id, and a command published as its `cmd_msgid` goes on to a heater. Every link is carried over the one peer, as message IDs. `make run-cfs` runs tcs-moc on it, tcs-sim simulating the cFS side; `tcspecial/tests/cfs.rs` flies it against a test playing the cFS side, and `tcspecial/tests/cfs_ground.rs` commands it from tcslib_moc. |

### minimal

The smallest configuration that does anything: one payload, `sensor`, a
program on this host at `127.0.0.1:5001` that sends its readings over UDP.
Nothing is drawn from it, so its readings go to the ground over the
`tcspecial` section's `payload_telemetry`: tcspecial sends each one to
`dest_base` plus the payload's device id, `127.0.0.1:6100` for `sensor`, which
is device 0. tcs-sim simulates `sensor`, sending a packet every second.

```
make run-simple
```

That runs tcs-moc on the example, which starts tcspecial and tcs-sim itself and
shows a panel for `sensor`: when its last packet arrived, what it said, and
how many there have been.

Languages:

- MOC: Rust -- tcs-moc.
- Vehicle: Rust -- tcspecial.
- Payload: Rust -- tcs-sim simulates `sensor`, a network payload.

### beacon

What a ground station can know about a spacecraft without asking it anything.
tcspecial sends a beacon every `beacon.default_interval` for as long as it
runs, over a socket of its own, whether or not anything has connected to it --
so this example commands nothing and still says whether the vehicle is alive,
which build it is, and whether the configuration it is serving is the one the
ground read.

It is also the example of how a ground program is put together, because almost
nothing in it is its own:

- `tcslib_moc::beacon` binds the socket, reads each datagram, counts the beacons
  that did not arrive, compares the digest in each one with the ground's own
  and says how stale the last one is. No window, no toolkit: a ground station
  with no screen -- a logger, a test -- uses exactly this.
- `tcs-ui` holds the panel, as markup other windows import, and works out what
  to show on it: which colour the indicator is, and the two lines beside it.
- `beacon`, the program here, reads the configuration to find where the
  beacons are, starts the loop on a thread, and puts each pass in the window.
  That is all of it, but for a button.

A beacon is a line of text, six fields with a colon between each: the name
`tcspecial`, the version as `major.minor.patch`, the configuration's MD5
digest in lower-case hex, the beacon's number in decimal, `restart` while a
restart the ground commanded is under way and the MOC has not connected
since or `normal` otherwise, and the interface's digest in hex:

```
tcspecial:0.1.0:36999e46a4bf436854442d8f97a2248d:42:normal:0123...cdef
```

It is text so that it can be read as it is, in a packet capture or with any UDP
listener. tcspecial writes every beacon and `tcslib_moc::beacon` reads it with
`tcslib_protocol::Beacon`, and this example's test writes the beacon it sends
the same way. The window shows it differently, as `tcspecial v0.1.0 md5
<digest>`.

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

tcs-moc shows the same panel from the same library, in a row beside its command
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
cargo run --bin beacon               # beacon.ytcs
cargo run --bin beacon -- --xml      # beacon.xtcs
```

Languages:

- MOC: Rust -- the `beacon` program, on `tcslib_moc::beacon` and `tcs_ui`.
- Vehicle: Rust -- tcspecial.
- Payload: Rust -- tcs-sim simulates `sensor`, a network payload.

### ping

The ground sends a ping up to the gateway node, which hands it to the `ping`
node; the `ping` node answers with the vehicle's time on the end, and the
answer comes back down. What a ping and its answer say is
`tcslib_protocol::node::vehicle::ping`:

```
ping #<count> <seconds>.<nanoseconds>                    up
ping #<count> <seconds>.<nanoseconds> <seconds>.<nanoseconds>   down
```

The ground station, `ping_gateway.py`, is Python 3 and uses nothing outside the
standard library to read its configuration's XML, the default; with `--yaml`
it reads the YAML instead, which needs PyYAML (`python3-yaml` on Debian and
Ubuntu). Its window shows the beacon panel, the last ping sent and
when, the last answer and when, and three times: the round trip, MOC to
tcspecial, and tcspecial to MOC. The last two are each read across two clocks,
so each includes their offset and may be negative; their sum is the round
trip. Press `Send ping` to send one: it clears the time received until its own
answer, matched by count, arrives.

It is built the way the `beacon` program is, from the same two halves written
again in Python: `python/tcslib_moc/beacon.py` after `tcslib_moc::beacon`,
`python/tcs_ui/beacon_panel.py` after `tcs_ui`, and `python/tcslib_protocol` for
the ping format, the beacon and the configuration. The interface digest beacons
are checked against is worked out with tcs-xtce's Python runtime, from the
tcs-xtce checkout beside this tree. `ping-common/ping_ground.py` is the ping's
own part, with no window, and `ping-common/ping_window.py` the window, both
shared with `ping-yamcs`. The Rust crate beside them is the tests that hold the
Python to the Rust; `cargo test` runs them.

```
make run-ping                                   # the vehicle and the window
python3 examples/ping/ping_gateway.py --pings 3 # no window: three pings
python3 examples/ping/ping_gateway.py --yaml    # ping.ytcs, not ping.xtcs
```

It sends a CONNECT of its own as soon as the vehicle is started, carrying the
configuration's checksum, with `python/tcslib_moc/listening.py`, and again each
second until the vehicle answers with that checksum: a vehicle sends nothing
down, the ping node's answers included, until it has taken one. `--pings`
waits for it before the first ping; `-v` says when it is matched. It is sent
from the configuration's `tcs_cmd.send`, so tcs-moc cannot be run on the same
configuration beside it.

`ping_gateway.py` starts the vehicle itself: tcspecial from the build, on the
same configuration. It kills it when it exits, however it exits -- if it is
killed or crashes, the kernel is told to kill tcspecial -- so a vehicle is
running exactly while its ground station is. Build tcspecial first (`make
run-ping` does). To run the vehicle yourself, in another terminal, say
`--no-vehicle`:

```
cargo run --bin tcspecial -- examples/ping/ping.xtcs   # in one terminal
python3 examples/ping/ping_gateway.py --no-vehicle    # in another
```

Its ports are all in the seven thousands, which no other example uses.

tcs-moc pings it too: `make runmocy TCSPECIAL_CONFIG_FILE=examples/ping/ping`,
then the `Ping node` button beside `Command`. It is the same panel, kept by
`tcslib_moc::ping`, which `ping_ground.py` is written after.

Languages:

- MOC: Python 3 -- `ping_gateway.py`, on `python/`; or Rust, in tcs-moc.
- Vehicle: Rust -- tcspecial, whose `ping` node answers.
- Payload: none. The `ping` node is the vehicle's own, and nothing is
  simulated.

### ping-yamcs

The ping example with [Yamcs](https://yamcs.org) at the ground end. The
vehicle and what is timed are the same; what differs is the configuration's
gateway, `yamcs`, so every packet to and from the ground carries a CCSDS
primary header, which tcspecial adds as each goes and takes off as each
arrives:

| What | Which way | APID |
|------|-----------|------|
| the beacon | down | `beacon.apid`, 0x100 |
| CONNECT and its answer | up, down | `tcs_cmd.apid`, 0x101 |
| a ping | up | the gateway node's `apid_base`, 0x200 |
| its answer | down | `apid_base` plus the ping node's device id, 0x201 |

A payload nothing in the drawing touches would report as telemetry of
`payload_telemetry.apid_base`, 0x300, plus its device id. This vehicle has
none, so Yamcs is given no container for one -- until one is added to the
configuration, when it is.

`ping_yamcs.py` writes a Yamcs configuration from the tcspecial one -- the ports
and the APIDs, stated once -- with a mission database of tcspecial's own XTCE
and a wrapper that puts the header around it. The wrapper and Yamcs's links are
generated by `python/tcslib_protocol/yamcs.py`: a container named after each
node whose data reaches the ground, `pingPacket` here; a command for each
gateway node; and a link on every port. The ping node has an interface document
of its own, `tcslib-protocol/xtce/ping.xtce`, which states a ping and its answer
field by field -- the prefix `ping #`, the count, the seconds and the nine
digits of nanoseconds, and for the answer the space and the vehicle's time -- so
`pingPacket` holds the answer as those fields, `ReplyCount`, `ReplySentSeconds`
and the rest, and the gateway node, whose data goes only to the ping node, takes
the document's command, `Ping`, with the arguments `Count`, `SentSeconds` and
`SentNanoseconds`. Yamcs's web interface shows both as the text they are. A node
with no such document is given a parameter named after it, of the type the
example's own XTCE, `ping-yamcs.xtce`, says it sends -- bytes, whole, for a
string -- and a gateway node whose data goes anywhere else a command named after
it that carries its data whole. Nothing in the wrapper is written by hand per
APID, so a node added to the configuration is decoded by Yamcs the next time the
example runs, and no line of it is wider than 80 columns. A node whose data the
configuration types -- a conversion's or a vote's value in its unit, a
thermostat's decision -- is given a parameter of that type instead: see
thermostat-yamcs. It starts Yamcs and tcspecial, has Yamcs send CONNECT until
the vehicle answers with the configuration's checksum, then pings through Yamcs
-- the ping split into the document's fields by the code tcs-xtce generates from
it, `python/tcslib_protocol/ping_packet.py` -- and reads the answers back over
Yamcs's REST API, putting each together again from its fields.

Its window is the ping example's, from `ping-common`: the beacon panel, and
the ping panel with its `Send ping` button. Starting Yamcs, its REST API,
CONNECT and the beacon through Yamcs are `python/tcslib_moc/yamcs_ground.py`'s,
which thermostat-yamcs shares. The beacon panel is fed the beacon's own bytes,
which the Yamcs wrapper keeps whole as the `BeaconBody` parameter, so it
checks the configuration's checksum and the beacon's number and ages the
indicator just as the ping example's does. With `--pings` there is no window:
it prints the first beacon and each exchange, and stops.

```
make run-ping-yamcs YAMCS_HOME=~/yamcs-5.14.0     # the window
python3 examples/ping-yamcs/ping_yamcs.py --yamcs ~/yamcs-5.14.0 --pings 3
python3 examples/ping-yamcs/ping_yamcs.py --describe   # what it read
```

Yamcs and tcspecial are stopped when the window is closed or the pings are
done, and are killed with `ping_yamcs.py` if it is killed or crashes, so
neither is left holding the example's ports.

An answer's arrival time is when Yamcs received it, to the millisecond, as
finely as Yamcs says; so a half of a sub-millisecond round trip can come out
a fraction of a millisecond negative.

The Rust crate beside it is its tests, which `cargo test` runs: `agrees.rs`
holds the ports, APIDs and checksum `ping_yamcs.py --describe` reads from
`ping-yamcs.xtcs` to what `tcslib-protocol` reads from `ping-yamcs.ytcs`, and
the generated wrapper's APIDs, for the example and for a copy with more nodes,
to the ones `tcslib-protocol` counts; `wrapper.rs` works the generated wrapper
as Yamcs would, decoding the beacon, the answers -- a ping's field by field,
every byte of it read -- and each added node's data as the vehicle frames them,
and encoding CONNECT and a ping, its fields as text, for the vehicle to unframe
and read as the ping `tcslib-protocol` writes, with the interpreter of
`examples/yamcs-common`; and `python.rs` runs `test_ping_yamcs.py`, which also
holds every line of the written wrapper to 80 columns. None needs Yamcs or opens
a window.

`YAMCS_HOME` is where a Yamcs release is unpacked, holding `bin/yamcsd`;
<https://github.com/yamcs/yamcs/releases> has them, and it was written
against 5.14.0. Yamcs's web interface is at <http://localhost:16090>
meanwhile, showing every packet and parameter.

Yamcs sends its commands from a port the system chooses and takes the
answers on the same socket, so the configuration's `tcs_cmd.send` is
`127.0.0.1:0`: commands from any port of this host.

Its ports are all in the sixteen thousands, which no other example uses.

Languages:

* tcspecial configuration: YAML (`ping-yamcs.ytcs`) or XML (`ping-yamcs.xtcs`)
* Yamcs configuration: YAML templates (`python/tcslib_moc/yamcs-templates`)
  and an XTCE template (`python/tcslib_protocol/yamcs.xtce`)
* ground program: Python 3 (`ping_yamcs.py`), with Yamcs (Java)

## Payloads on the kernel's dummy devices

Each is one of the payload types whose far end is a device the kernel makes
rather than a file or a socket tcs-sim can make, and each is flown against the
kernel's dummy device for it by `tcspecial/tests/kernel-dummies.rs`,
which plays the device's far end through the kernel: that is what makes CAN
and GPIO supported. IIO stays experimental until its test has passed, which it
cannot on a kernel whose `iio_dummy` has no buffer, such as Ubuntu's: see
"What Is Supported and What Is Experimental" in `docs/tcspecial-design.rst`. The
devices need root to make, so nothing here makes them. Run this once, from
the account that runs the examples and the tests:

```
sudo bin/mk-kernel-dummies
```

With no arguments it makes the CAN bus and the GPIO chip; `sudo
bin/mk-kernel-dummies iio` makes the IIO sensor, and `can gpio iio` all
three. It refuses, and makes nothing, where this kernel lacks a module one
of the types named needs, saying which. It gives what it makes to the user
who ran it, and prints the number the kernel gave the GPIO chip and the IIO
sensor. `sudo bin/rm-kernel-dummies` takes away whichever were made.
Nothing either does survives a reboot.

| Directory | Description |
|---|---|
| `can` | A motor controller on the CAN bus `vcan0`, as two payloads: its status frames, identifier 0x123, go to the ground, and each command the ground sends goes to it as one frame with identifier 0x124. |
| `gpio` | Two lines of a GPIO chip: a switch on line 0, a `gpio_in`, reports its level to the ground twice a second as a `u8`, and a `u8` 1 or 0 from the ground drives a lamp on line 1, a `gpio_out`. |
| `iio` | Experimental. A sensor's sample buffer, read through the kernel's IIO subsystem, its whole sample sets going to the ground. Nothing goes up: a buffer is read and never written. |

### can

`vcan0` is the kernel's virtual CAN bus: a bus with no wire, whose members are
the sockets bound to it. tcspecial joins it as two of them, one for each
identifier, and tcs-sim joins it as the motor, sending a status frame a second
and counting the commands. On a vehicle the interface is the controller's,
such as `can0`, and nothing else changes.

```
make run-can
```

### gpio

The switch is a `gpio_in`, whose line is watched, and the lamp a `gpio_out`,
whose line is driven: see "A GPIO Payload" in the manual. The chip is one of
the kernel's `gpio-sim`, whose lines exist in memory: a
line's `pull` file sets what an input is pulled to, which tcspecial reads as
the line's level, and its `value` file says what an output is driven to. tcs-sim pulls the switch's line the other way every second and counts the
lamp's changes. The configuration names `/dev/gpiochip1`, which is what the
simulated chip is on a host with one chip of its own; the number is given as
chips appear, so write the one `bin/mk-kernel-dummies` printed.

```
make run-gpio
```

### iio

The sensor is one of the kernel's `iio_dummy`, whose buffer a software
trigger fills a hundred times a second: the dummy device is the whole payload,
and tcs-sim makes nothing. Its buffer is there only where the kernel's
`iio_dummy` was built with `CONFIG_IIO_SIMPLE_DUMMY_BUFFER`, which a stock
distribution kernel's is not; `bin/mk-kernel-dummies iio` says so where it
was not, and that is why the type is still experimental. The
configuration names `/dev/iio:device1`, which is what the dummy is on a host
with one sensor of its own; write the number `bin/mk-kernel-dummies`
printed.

```
make run-iio
```

Languages, for all three:

- MOC: Rust -- tcs-moc.
- Vehicle: Rust -- tcspecial.
- Payload: the kernel's dummy device, with tcs-sim playing the device's far end
  for CAN and GPIO.

## More complex examples

Each puts the vehicle to work on its payloads: what they send, and what is done
with it aboard and on the ground.

| Directory | Description |
|---|---|
| `thermostat` | Three temperature sensors on a thermal mass, two on I2C and one on SPI, converted to kelvin, voted on, and fed to a thermostat that drives two heaters, every one simulated by tcs-sim, so that it runs on any desk. The setpoint is commanded from the ground, and tcs-moc plots the temperatures and the heaters. |
| `thermostat-yamcs` | The thermostat, with Yamcs as its ground station, over CCSDS packets: the zone's temperature and the three readings arrive in Yamcs as temperatures in kelvin, and the thermostat's decision as on or off, each a parameter the configuration types, and a Python 3 program shows them. |
| `battery` | A power bus through an orbit: a power monitor asked over a serial line, a shunt, the battery's charge counted by a Python 3 program, and the science load shed in eclipse by a thermostat. tcs-sim simulates every payload, joined to a physics model of a sun, a solar array, loads, a bus and a battery. |
| `descent` | A lander's terminal descent: a radar altimeter's altitude flown down by a Python 3 guidance program throttling the engine, a Doppler velocimeter, and a touchdown switch that speaks the moment the lander is down. tcs-sim simulates every payload, joined to a physics model of an engine and the body it brakes. |
| `simple-app` | A payload that is a program rather than a device: an `app_pl` node sending a line of text one word per packet, and a ground station that prints the line. Two programs of its own beside the configuration: the payload in Python 3, the ground station in Rust. |

### thermostat

A control loop, aboard. Three temperature sensors are read together, once a
second, as the group `zone_temps`: `temp_1`, `temp_2` and `temp_3`. Each
reading goes through a conversion of its own to kelvin -- `to_k1`, `to_k2`,
`to_k3`, the parts differing in format and scale -- and the three meet at
`consensus_temp`, which takes their median, so that one sensor failing cannot
take the zone's temperature with it. `thermostat` holds that to a setpoint,
293.15 K with a half band of 2 K, and drives two heaters, each through a GPIO
line of its own, `gpio-out-tmass1` and `gpio-out-tmass2`, which go on and off
together. Each switch is a `gpio_inout`: it drives its line, and every second
sends the ground the last value it was told, so the ground follows the
heaters on a beat. The ground commands the setpoint.

The sensors are temperature sensors on the zone, in
`thermostat-common/thermostat.ytcs` with the rest of the vehicle: `temp_1` and
`temp_2` on an I2C bus, of type `i2c_tmass`, and `temp_3` a SPI peripheral,
of type `spi_tmass`. The heaters' switches are lines 4 and 5 of
`/dev/gpiochip1`. On a desk every one of them is a simulated device: each is
reached over UDP at its `sim_address` -- the sensors at 127.0.0.1:5201 to
5203, where tcs-sim answers with the zone's temperature, and the switches at
5204 and 5205, where tcs-sim takes what the vehicle drives them to -- so the
example needs no dummy device, no sudo and no hardware. Flown, the buses and
the chip are opened. The buses themselves are flown against simulated devices
by `make simulated-devices-test`, with `tests/manual/thermostat.ytcs`.

tcs-sim joins the simulated payloads to the vehicle's physics model, in the world
section of `thermostat-sim.yaml`, a link to
`thermostat-common/thermostat-sim.yaml`, which thermostat-yamcs's simulator
file is a link to too: a zone with two heaters of its own, four watts each,
`heater_1_power` and `heater_2_power`. All three sensors are joined to
`zone.temperature`, each from
a place of its own on the zone, so their readings agree and the vote
produces a temperature. The zone starts at 292.15 K, inside the band, and
cools towards its surroundings, 288.15 K, just inside the
plot's lower red, which swing half a kelvin over a ninety minute orbit; each
heater's switch is joined to `zone.heater_1` or `zone.heater_2`, so the
heaters warm it while the vehicle has them on: the loop closes through
tcspecial.
The heaters are four watts each, and either alone holds the band, the
thermostat cycling whichever are working; a heater disabled in tcs-sim shows
in the temperature, and with both disabled the zone falls into the plot's
lower red.

A heater whose switch's panel in tcs-sim is not enabled is unpowered, or
failed: it stays off whatever the thermostat decides, and enabled again it
takes the latest decision at once. The ground sees it in the temperature: its
switch goes on reporting what it was told.

```
make run-thermostat
```

That runs tcs-moc on the example, as `run-simple` does, and tcs-moc opens its
plot window beside its own: each sensor's temperature and their vote over
the last ten seconds, a dot at each reading, with the setpoint's rule and the
thermostat's band, and under them a strip for each heater's switch,
`gpio-out-tmass1` and `gpio-out-tmass2`, on while it was told on. Press
Connect in tcs-moc, and Enable on each payload in tcs-sim, to start the data and
the heating; press Disable on a heater's switch in tcs-sim to take it away, and
the zone falls behind in the cold part of the orbit.

Languages:

- MOC: Rust -- tcs-moc.
- Vehicle: Rust -- tcspecial, whose conversions, vote, thermostat and log are
  its own nodes.
- Payload: Rust -- tcs-sim simulates the three sensors, with the zone
  they read, and the heaters' switches, on the kernel's simulated GPIO
  chip.

### thermostat-yamcs

The thermostat example with [Yamcs](https://yamcs.org) at the ground end,
as ping-yamcs is the ping example with it. The vehicle is the thermostat's
-- the same conversions, vote, thermostat, heaters, GPIO lines and log,
from the file both include, `thermostat-common/thermostat.ytcs`, with the same
three sensors, and the same simulator file,
`thermostat-common/thermostat-sim.yaml`, physics model and all, each
example's being a link to it; the two cannot run at once, their heaters
being the same lines of one GPIO chip -- and what differs is
the configuration's gateway, `yamcs`, so every packet to and from the
ground carries a CCSDS primary header:

| What | Which way | APID |
|------|-----------|------|
| the beacon | down | `beacon.apid`, 0x110 |
| CONNECT and its answer | up, down | `tcs_cmd.apid`, 0x111 |
| what Yamcs sends the gateway node | up | its `apid_base`, 0x400 |
| `consensus_temp` | down | `apid_base` plus its device id, 0x400 |
| `gpio-out-tmass1`, `gpio-out-tmass2` | down | 0x402 and 0x403 |
| `to_k1`, `to_k2`, `to_k3` | down | 0x408 to 0x40a |

`thermostat_yamcs.py` writes Yamcs's configuration with
`python/tcslib_moc/yamcs_ground.py`, as `ping_yamcs.py` does, its mission
database generated by `python/tcslib_protocol/yamcs.py`. That gives each node
drawn into the gateway node a parameter of the type the example's own
XTCE, `thermostat-yamcs.xtce`, says its data are, copied from there:
tcs-verify writes that file from the configuration (`make xtce`), so the
Python works out nothing of its own. A conversion's or a vote's value is a
float in its unit, with the unit in a `UnitSet`, read from its four bytes of
a signed count of the unit's steps through a calibrator, and what a heater's
switch was told is the whole number its GPIO lines are, a `u8`. So Yamcs's
web interface, at <http://localhost:17090>, shows `consensus_temp` and
`to_k1` to `to_k3` as temperatures in K and `gpio-out-tmass1` and
`gpio-out-tmass2` as the numbers their lines were told. Beside them is
`world_ambient_temp`, the simulated world's ambient temperature, which the
vehicle does not send: a local parameter in K, which `thermostat_yamcs.py`
sets from the simulator file's `ambient_temp`, read by
`tcs-verify --ambient`, once the vehicle has answered CONNECT.

It starts Yamcs, then tcs-sim -- every payload sending from the start, as
though its panel's button had been pressed -- and, once tcs-sim says it is
ready, tcspecial, each with `--simulate=all`; has Yamcs send CONNECT until
the vehicle answers with the configuration's checksum; and reads the
parameters back over Yamcs's REST API. Its window shows the beacon panel
above the thermostat's: the zone's temperature, each reading, and each line
sensing a heater, on or off. With `--seconds` there is no window: it prints
every value each `--every` seconds, and a line each time a heater's line
changes, and stops. tcs-sim draws a window of its own, so either wants a
display. Every payload sending from the start includes the heaters'
switches, so both start enabled; press Disable on one in tcs-sim's window to
take its heater away, and its line goes low and stays low while the zone
falls behind in the cold part of the orbit. Its GPIO lines are the thermostat
example's, so the two are not run at once.

```
make run-thermostat-yamcs YAMCS_HOME=~/yamcs-5.14.0   # the window
python3 examples/thermostat-yamcs/thermostat_yamcs.py --yamcs ~/yamcs-5.14.0 \
    --seconds 120
python3 examples/thermostat-yamcs/thermostat_yamcs.py --describe
```

Yamcs, tcs-sim and tcspecial are stopped when it ends and killed with it if
it is killed or crashes.

The Rust crate beside it is its tests: `agrees.rs` holds the ports, APIDs,
forms and checksum `thermostat_yamcs.py --describe` reads from
`thermostat-yamcs.xtcs` to what `tcslib-protocol` reads from
`thermostat-yamcs.ytcs`, the Python's form of every node of every example
to `Form::of`, and its table of units to `tcslib_protocol::unit`; `wrapper.rs`
works the generated mission database as Yamcs would, with
`examples/yamcs-common`'s interpreter, on what the vehicle sends -- each
value as `value_bytes` makes it, in every unit there is, and each decision;
`shared.rs` holds its simulator file to the thermostat example's; and
`python.rs` runs `test_thermostat_yamcs.py`. None needs
Yamcs or opens a window.

Its own ports are all in the seventeen thousands, which no other example
uses. Its sensors and its heaters' switches are thermostat-common's,
simulated at the thermostat example's ports, 5201 to 5205, so the two are
not run at once.

Languages:

* tcspecial configuration: YAML (`thermostat-yamcs.ytcs`) or XML
  (`thermostat-yamcs.xtcs`)
* ground program: Python 3 (`thermostat_yamcs.py`), with Yamcs (Java)
* payloads: Rust -- tcs-sim, as in the thermostat example

### battery

A spacecraft's power bus through an orbit: the example of a payload that is
asked, of a computation that is a program, and of physics with events in it.

`monitor`, a power monitor on a serial line at `/tmp/tcs-battery-monitor`, is
asked for the bus voltage twice a second -- its `trigger` is `"V?\r"` -- and
`bus_volts` makes its two bytes volts. `shunt`, the shunt in the battery's
lead, sends the battery's current twice a second over UDP, port 5301, and
`battery_amps` makes it amperes. `charge` is a `process` node running
`charge.py`, a Python 3 program beside the configuration that counts the
current coulomb by coulomb into the battery's state of charge, a ratio, and
sets itself full whenever the current falls to nought. `shedding`, a
thermostat around 0.4, sheds the science instrument below three tenths by
driving `science`, port 5302, and restores it above half.

tcs-sim simulates all three, joined to the vehicle's physics model, in
the world section of `battery-sim.yaml`: a sun, a solar array, two loads, a bus and a battery,
each a part simulated alone and joined under `connections`. The orbit is six
minutes and the battery small, so that a run sees, in one orbit, the battery
charge and fill, the eclipse drain it, the science load shed and the sun bring
it back. Going into shadow and out, and the battery reaching full, are events
of the model's, met at their moments.

```
make run-battery
```

Press Connect in tcs-moc, and Enable on each payload in tcs-sim, to start the
data.

Languages:

- MOC: Rust -- tcs-moc.
- Vehicle: Rust -- tcspecial, whose conversions, thermostat and log are its
  own nodes -- and Python 3, the charge count, which is a program of the
  example's.
- Payload: Rust -- tcs-sim simulates the power monitor, with a
  pseudo-terminal at its path, the shunt and the science switch.

### descent

A lander's terminal descent over the Moon: the example of a computation that
drives a device with a value, and of a device that speaks the moment
something happens.

`radar`, a radar altimeter on UDP port 5401, sends the altitude ten times a
second, which `altitude` makes metres. `guidance` is a `process` node running
`guidance.py`, a Python 3 program beside the configuration that works out the
rate of descent from the altitudes, flies a rate that slows as the ground
nears, and answers a throttle, a ratio, which drives `engine`, port 5404:
four bytes of a value, which tcs-sim's engine reads as the ratio it is.
`doppler`, port 5402, sends the velocity for the ground, and `touchdown`,
port 5403, a switch on a landing leg, says `1` when the lander is down.

tcs-sim simulates all four, joined to the vehicle's physics model, in
the world section of `descent-sim.yaml`: an engine and the body it brakes, five hundred metres
up and falling at twenty metres a second. The body falls by the rocket
equation, and the moment it reaches the ground is found ahead of time and met
exactly; tcs-sim's touchdown switch waits on the model for it, and
speaks then. A run lands in about seventy-five seconds.

```
make run-descent
```

Press Connect in tcs-moc, and Enable on each payload in tcs-sim, `engine` and
`radar` first: the lander does not start falling until a simulated payload
starts.

Languages:

- MOC: Rust -- tcs-moc.
- Vehicle: Rust -- tcspecial, whose conversions and log are its own nodes --
  and Python 3, the guidance, which is a program of the example's.
- Payload: Rust -- tcs-sim simulates the radar, the velocimeter, the
  switch and the engine.

### simple-app

The one example that runs end to end on one host, and the one to look at to
see what an app node is. Two programs, both in `examples/simple-app`:

- `lorem_pl.py` is the payload, a Python 3 program that uses nothing outside
  the standard library. tcspecial starts it from the configuration, as
  the `executable` of the `app_pl` node named `lorem`, and runs it with the
  node's name and `app-payload-arg` -- which for this payload is the line of
  the text to send, counting from one. It sends one word per packet and then a
  packet holding the line terminator.
- `lorem_gateway` is the ground station, a Rust program in the crate beside the
  configuration. It gets `lorem`'s words from the configuration's gateway node,
  `gateway-1`, with one function call,
  `tcslib_moc::gateway::GatewayReceiver::receive_into`, which works out from the
  configuration which port they arrive at and puts each
  word in a buffer made before the loop, so that the loop allocates nothing; it writes out each word
  with one space between them, and stops when the terminator arrives. Before
  its first word it sends a CONNECT of its own, carrying the configuration's
  checksum, with `tcslib_moc::listening`, and sends it again each second until
  the vehicle answers with that checksum: a vehicle sends nothing down until
  it has taken one, and nothing else in this example sends one. It sends it
  from the configuration's `tcs_cmd.send`, so tcs-moc cannot be run on the
  same configuration beside it; `-v` says when it is matched.

```
make run-simple-app
```

That checks for Python 3, builds the vehicle and the ground station, starts
the vehicle in the background and the ground in the foreground, prints the
line, and then stops the vehicle. To watch the two separately, run them in two
terminals instead, in either order:

```
cargo build                   # lorem_gateway and tcspecial
cargo run --bin lorem_gateway # in one terminal: the ground (--xml for XML)
cargo run --bin tcspecial -- examples/simple-app/simple-app.ytcs   # in another
```

Run the vehicle from the repository's root, where the configuration's
`executable` path starts. Either order works because the payload waits a couple
of seconds before its first word: the vehicle sends each word to the ground
station's address as soon as it has it, and a word sent before
`lorem_gateway` is listening is lost.

The text itself is `simple_app`'s library: the first paragraph of the lorem
ipsum text as one array per line, each holding that line's words. Nothing
holds a line terminator; the end of a word array is the end of the line, and
what stands for it on the wire is the one constant both programs read.

Languages:

- MOC: Rust -- `lorem_gateway`, on `tcslib_moc::gateway`.
- Vehicle: Rust -- tcspecial, which runs the payload as its `app_pl` node.
- Payload: Python 3 -- `lorem_pl.py`.

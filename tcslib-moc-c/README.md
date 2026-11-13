# tcslib-moc-c

**tcslib-moc-c is tcslib-moc for C and C++.** It is the library a ground station
written in C or C++ is built on: the vehicle's beacons, the vehicle's data
from a gateway node, and commands to the vehicle, behind one header,
`include/tcslib-moc-c.h`.

It is not a second implementation of anything. Each function is a call into
[tcslib-moc](../tcslib-moc/README.md), so a C or C++ ground station reads the
configuration, counts ports and ages beacons exactly as tcsmoc does, and the
two cannot come to disagree.

C and C++ alike: the header is C, includable from C++ as it stands, with C
linkage throughout, and nothing in it that C++ will not compile.

A configuration that answers to cFS is used through the same calls: tcslib-moc
joins the cFS end as tcssim simulates it as an SBN peer, on a
thread of its own, and the beacons, the data and the commands go over that
peer (see "A vehicle on a cFS software bus" in
[tcslib-moc's README](../tcslib-moc/README.md)). Nothing in the header changes.

## What is in it

- **The configuration.** `tcs_configuration_open` reads and checks a
  configuration, YAML or XML, as the vehicle does. It gives the
  configuration's digest and each node's id.
- **Beacons.** `tcs_beacons_listen` listens where the configuration says
  beacons arrive, and each `tcs_beacons_pass` says how long since the last
  one: the stage (Unheard, Heard, Aging or Waiting), whether an indicator
  showing it is lit, whether the spacecraft is being heard from, when the last
  beacon arrived and what it said. It also says whether this pass's beacon
  started the count again, which is how a restart is known, and whether its
  restart flag said `restart`, which it does from every start of the vehicle
  and from its taking a restart, until it takes a connect carrying its
  configuration's checksum. Each pass also gives remarks, which are news, and
  warnings, which are to be said whether or not anybody asked.
- **Matching.** Each pass keeps the configuration's *match* with the
  vehicle, which every beacon watch, client and gateway node made from that
  configuration shares. The station is matched for listening while the last
  beacon is recent, has the restart flag clear and carries the
  configuration's checksum, and for commanding once, besides, its own
  connect has been answered with that checksum. `tcs_configuration_matched`
  says which; `tcs_configuration_counts` says how many packets have come
  from the vehicle, data and answers, and how many of them were processed,
  malformed or dropped for a configuration mismatch, which add up.
- **The vehicle's data.** `tcs_gateway_open` opens a gateway node, and each
  `tcs_gateway_receive` returns one datum and the name of the node that produced
  it. A gateway node reads data only while the station is matched for listening:
  what arrives before is dropped and counted, as is whatever is waiting when
  the match begins. So a station that reads data passes its beacons too.
  `tcs_configuration_sends_values` says whether a node's data are *values*
  -- four bytes of a count of a unit's steps, which a conversion, a vote or
  a program's answer makes -- as the configuration says, a datum's width
  being no guide. `tcs_configuration_value_unit` says what unit a node's
  values are in, `tcs_value_in` reads one as a number in it, and
  `tcs_value_kelvin` reads a temperature.
  `tcs_configuration_sends_decisions` says whether they are *decisions* --
  the text `1` or `0`, which a thermostat makes -- and `tcs_decision_on`
  reads one as on or off.
- **Data up.** `tcs_uplink_open` opens the way up to a gateway node that
  binds an address and port aboard, and each `tcs_uplink_send` sends one
  datum there, which leaves the node by its `moc-out` links for the
  nodes they reach: a heater driven, a line set. It is tcslib-moc's
  `ping::Uplink`, the way the ping goes up. What goes up drives a payload
  as a command does, so it goes only while the station is matched for
  commanding, and fails unsent before, saying the configurations are not
  yet matched.
- **Commands.** `tcs_client_open` opens the two command links the
  configuration states. `tcs_client_connect` says whether the vehicle is
  flying this configuration; then ping; start, stop or query a node by its
  id, set one of its attributes, or ask for the last datum it moved each way;
  and arm a restart and restart the vehicle. Until the station is matched
  for commanding, every command but connect, arm and restart fails, unsent,
  saying the configurations are not yet matched; the vehicle itself answers
  any other command that reaches it before a match `TCS_STATUS_UNMATCHED`,
  and does not act on it. A start of the vehicle ends the match, since its
  beacons carry the restart flag, so after one the client again sends
  nothing but connect, arm and restart until a connect is answered with this
  configuration's checksum: no answer carries a generation, and none is
  needed. `tcs_client_restart_arm` and `tcs_client_restart` name the
  configuration to restart onto by its checksum, NULL meaning this one's; a
  vehicle that does not hold it answers `TCS_STATUS_NOT_FOUND`.
  Nothing is held back after a restart is sent: the vehicle answers every
  command but connect, arm and restart `TCS_STATUS_UNMATCHED` from the
  moment it takes the restart until the start that comes back is matched,
  and that start's beacons end the station's match. A status this
  library does not know -- a vehicle of a later build may add one -- is
  written as the number it came as: it is a refusal, and
  `tcs_status_name` gives "" for it, so a station says it by its number.
- **Pinging a ping node.** `tcs_pinger_open` finds the first `ping` node the
  ground can reach both ways and its gateway node. `tcs_pinger_ping` sends a
  ping up; its answer comes down as the ping node's data, which a station
  takes with `tcs_gateway_receive` and hands to `tcs_pinger_answered`; and
  `tcs_pinger_exchange` gives the round trip and the two halves of it.

Every handle is made by an `_open` and freed by its `_close`. A function that
cannot do what it was asked returns `NULL` or -1, and `tcs_last_error()` says
why. The header says the rest.

## Building with it

Cargo builds `libtcs_moc_c.a` and `libtcs_moc_c.so`, from the repository's
root:

```
cargo build -p tcslib-moc-c
cc  my_station.c   -I tcslib-moc-c/include \
    target/debug/libtcs_moc_c.a -lpthread -ldl -lm
c++ my_station.cpp -I tcslib-moc-c/include \
    target/debug/libtcs_moc_c.a -lpthread -ldl -lm
```

Installed -- by the `tcspecial-moc` package, or `make install` at the
repository's root -- it is `/opt/tcs/lib/libtcs_moc_c.so.<ABI>`, with the
link `libtcs_moc_c.so` to it, `libtcs_moc_c.a`, the header as
`/opt/tcs/include/tcslib-moc-c.h`, and `/opt/tcs/lib/pkgconfig/tcslib-moc-c.pc`.
A station then includes `<tcslib-moc-c.h>` and is built with pkg-config:

```
cc  my_station.c   $(pkg-config --cflags --libs tcslib-moc-c)
c++ my_station.cpp $(pkg-config --cflags --libs tcslib-moc-c)
```

### The ABI version

`<ABI>` is the library's ABI version, the one whole number in `abi-version`, now
4. It is the shared library's soname, `libtcs_moc_c.so.<ABI>`, which a
station linked with the library asks for at run time; `build.rs` sets the soname
from the file, and the Makefile installs the library under the same name, so the
number is stated once. It is not the crate's version, and changes for another
reason. Add one to it when, and only when, a station built against the old
`tcslib-moc-c.h` would not work with the new library:

- a function's signature changes, or its behaviour changes in a way a
  station relying on the old behaviour would notice;
- a function is removed;
- a struct or enum in `tcslib-moc-c.h` changes its layout or its values, a
  constant such as `TCS_TEXT_BYTES` included.

Adding a function, or anything else an old station cannot notice, leaves it
alone. A new ABI version is a new soname, so a station built against the
old one is refused by the loader, saying which library it wanted, rather
than run against a library it does not match.

It went from 1 to 2 when the link lost its header: `tcs_client_restart_arm`
and `tcs_client_restart` gained the target checksum, and the generation the
client used to track went.

It went from 2 to 3 with typed links: a decision became a `boolean`, the one
byte 1 or 0, where it was the text `1` or `0`, so `tcs_decision_on` reads
another datum than it did, and `tcs_configuration_sends_decisions` answers
yes only for a thermostat whose `type-out` is `boolean`.

It went from 3 to 4 when a setting became typed: the value
`tcs_client_set_node` carries is a datum of the attribute's type in its own
encoding -- a setpoint as four bytes of micro-kelvin -- where it was the text
an operator typed, and `tcs_configuration_setting` writes it from that text.

`examples/ground.c` is a ground station in C, and `examples/ground.cpp` the
same one in C++: each reads the configuration, waits for the first beacon,
connects and pings, asks about a node and sets one of its attributes,
pings the vehicle's ping node and times the answer, and
prints the vehicle's data from a gateway node, a value as a temperature. They
take `--yaml` or `--xml`, as every example program does, `--restart` to arm a
restart and restart the vehicle, and `--send=<text>` to send the text up to
the gateway node, each of which they do only when asked.

## How the header is kept honest

It is written by hand, for people, so the proof that it says what the library
does is a program that compiles against it and works. `tests/c_and_cpp.rs`
builds `libtcs_moc_c.a` from the source as it is, compiles both example
programs against the header with gcc and g++ -- warnings as errors -- and runs
each against a vehicle the test plays. The two must print the same lines, and
the right ones, and they compare what they read against the header's own
constants, so a header whose numbers had drifted from the library's fails. The
header alone is also compiled as C99 and as C++11.

## Known limits

- **A query's counts are not given.** `tcs_client_query_node` says whether a
  node is running; the counts of what it has moved, which tcslib-moc's answer
  carries, have no C spelling yet.
- **One thread per handle.** A handle is used from one thread at a time;
  nothing in the library locks one. `tcs_last_error()` is per thread.
- **A beacon pass may wait for ever.** Before any beacon has arrived,
  `tcs_beacons_pass` waits for one, with no timeout: run it on a thread of
  its own, as tcsmoc does. To stop that thread, make a waker with
  `tcs_beacons_waker_open` before starting it, and call `tcs_beacons_wake`
  from the thread stopping it, then join it: the pass it is in ends,
  finding nothing. The F´ bridge stops its beacon thread this way.
- **Strings are borrowed.** A string a function returns is good until the
  next call on the same handle or its `_close`. Copy it to keep it.
- **Linking.** On Linux, `libtcs_moc_c.a` needs `-lpthread -ldl -lm` beside
  it, which is what the tests link with. That is enough where the C library has
  folded the rest into itself, as recent glibc has; the whole list for a system
  is what `cargo rustc -p tcslib-moc-c --lib --crate-type staticlib -- --print
  native-static-libs` prints, which here is `-lgcc_s -lutil -lrt -lpthread -lm
  -ldl -lc`. Nothing but Linux has been built or tested.
- **Tests without a C compiler.** Where there is no gcc or no g++,
  `tests/c_and_cpp.rs` says so and passes without building that program: a
  machine with no C compiler has no C program to break, but it has also not
  checked the header.
- **Fixed-size text.** What the library hands back is held in buffers of
  fixed size, stated in the header: `TCS_ERROR_BYTES` (512) for
  `tcs_last_error()`, `TCS_TEXT_BYTES` (256) for what a beacon said, each
  remark and warning, and why the vehicle refused a SET_NODE
  (`tcs_client_reason`), and at most `TCS_MAX_REMARKS` (8) remarks and as many
  warnings a pass. Text too long is cut short at a character and ends "...";
  a pass with more to say keeps seven and says how many more there were. A
  SET_NODE carries an attribute of at most `TCS_ATTRIBUTE_BYTES` (64) and a
  value of at most `TCS_VALUE_BYTES` (256); longer is refused, and not sent.
- **Where it still allocates.** Avoiding dynamic allocation is a rule of this
  repository. This layer allocates nothing of its own as a station runs; what
  remains is said in the source where it happens, and is:
  - *Once, at startup.* Each `_open` allocates its handle, which has to outlive
    the call and whose size C does not see. `tcs_configuration_open` reads and
    checks a whole configuration, as tcslib-moc does; `tcs_gateway_open` makes
    the names of the node's sources, which every receive then points at, and the
    buffer every datum is received in; and `tcs_configuration_node_id` asks
    tcslib-moc for every node's id.
  - *In tcslib-moc, as a station runs, only where something happens.* A beacon
    pass, a datum, a ping and its answer, and a command sent allocate
    nothing. What does: a remark or warning, on the pass where something
    changed; an answer to a command, whose strings tcslib-moc reads as its own;
    and what came down from the ping node that is not an answer, to say why.

`make` in this directory builds `README.html` from this file.

## See also

- [This page as HTML](https://vomlehn.github.io/tcspecial/tcslib-moc-c/README.html).
- [tcslib-moc](https://vomlehn.github.io/tcspecial/tcslib-moc/README.html), the Rust library this one wraps, and [tcslib-protocol](https://vomlehn.github.io/tcspecial/tcslib-protocol/README.html), what the ground and the vehicle share.
- [The tcspecial
manual](https://vomlehn.github.io/tcspecial/docs/tcspecial.html) and
[design](https://vomlehn.github.io/tcspecial/docs/design.html).

## License

Licensed under either of the Apache License, Version 2.0 ([LICENSE-APACHE](https://github.com/vomlehn/tcspecial/blob/main/LICENSE-APACHE)
or <http://www.apache.org/licenses/LICENSE-2.0>) or the MIT license
([LICENSE-MIT](https://github.com/vomlehn/tcspecial/blob/main/LICENSE-MIT) or <http://opensource.org/licenses/MIT>), at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

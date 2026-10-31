# tcslib-c

**tcslib-c is tcslib for C and C++.** It is the library a ground station
written in C or C++ is built on: the vehicle's beacons, the vehicle's data
from a gateway node, and commands to the vehicle, behind one header,
`include/tcs.h`.

It is not a second implementation of anything. Each function is a call into
[tcslib](../tcslib/README.md), so a C or C++ ground station reads the
configuration, counts ports and ages beacons exactly as tcsmoc does, and the
two cannot come to disagree.

C and C++ alike: the header is C, includable from C++ as it stands, with C
linkage throughout, and nothing in it that C++ will not compile.

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
- **Commands.** `tcs_client_open` opens the two command links the
  configuration states. `tcs_client_connect` says whether the vehicle is
  flying this configuration; then ping; start, stop or query a node by its
  id, set one of its attributes, or ask for the last datum it moved each way;
  and arm a restart and restart the vehicle. Until the station is matched
  for commanding, every command but connect, arm and restart fails, unsent,
  saying the configurations are not yet matched; the vehicle itself answers
  any other command that reaches it before a match `TCS_STATUS_UNMATCHED`,
  and does not act on it. The client
  learns which generation of tcspecial is flying from the answer to connect,
  so until one is answered, and again after a beacon whose number went back
  or an answer from another generation, it refuses every command but ping
  and connect, unsent. `tcs_client_beacon_heard` tells it of each pass's
  beacon; after a restart is sent it is what ends the wait for the vehicle
  to come back, during which nothing but connect, arm and restart goes.
- **Pinging a ping node.** `tcs_pinger_open` finds the first `ping` node the
  ground can reach both ways and its gateway node. `tcs_pinger_ping` sends a
  ping up; its answer comes down as the ping node's data, which a station
  takes with `tcs_gateway_receive` and hands to `tcs_pinger_answered`; and
  `tcs_pinger_exchange` gives the round trip and the two halves of it.

Every handle is made by an `_open` and freed by its `_close`. A function that
cannot do what it was asked returns `NULL` or -1, and `tcs_last_error()` says
why. The header says the rest.

## Building with it

Cargo builds `libtcs.a` and `libtcs.so`, from the repository's root:

```
cargo build -p tcslib-c
cc  my_station.c   -I tcslib-c/include target/debug/libtcs.a -lpthread -ldl -lm
c++ my_station.cpp -I tcslib-c/include target/debug/libtcs.a -lpthread -ldl -lm
```

Installed -- by the `tcspecial-ground` package, or `make install` at the
repository's root -- it is `/opt/tcs/lib/libtcs.so.<ABI>`, with the link
`libtcs.so` to it, `libtcs.a`, the header as `/opt/tcs/include/tcs.h`, and
`/opt/tcs/lib/pkgconfig/tcs.pc`. A station then includes `<tcs.h>` and is
built with pkg-config:

```
cc  my_station.c   $(pkg-config --cflags --libs tcs)
c++ my_station.cpp $(pkg-config --cflags --libs tcs)
```

### The ABI version

`<ABI>` is the library's ABI version, the one whole number in `abi-version`,
now 1. It is the shared library's soname, `libtcs.so.<ABI>`, which a station
linked with the library asks for at run time; `build.rs` sets the soname
from the file, and the Makefile installs the library under the same name,
so the number is stated once. It is not the crate's version, and changes
for another reason. Add one to it when, and only when, a station built
against the old `tcs.h` would not work with the new library:

- a function's signature changes, or its behaviour changes in a way a
  station relying on the old behaviour would notice;
- a function is removed;
- a struct or enum in `tcs.h` changes its layout or its values, a
  constant such as `TCS_TEXT_BYTES` included.

Adding a function, or anything else an old station cannot notice, leaves it
alone. A new ABI version is a new soname, so a station built against the
old one is refused by the loader, saying which library it wanted, rather
than run against a library it does not match.

`examples/ground.c` is a ground station in C, and `examples/ground.cpp` the
same one in C++: each reads the configuration, waits for the first beacon,
connects and pings, asks about a node and sets one of its attributes,
pings the vehicle's ping node and times the answer, and
prints the vehicle's data from a gateway node. They take `--yaml` or `--xml`,
as every example program does, and `--restart` to arm a restart and restart
the vehicle, which they do only when asked.

## How the header is kept honest

It is written by hand, for people, so the proof that it says what the library
does is a program that compiles against it and works. `tests/c_and_cpp.rs`
builds `libtcs.a` from the source as it is, compiles both example programs
against the header with gcc and g++ -- warnings as errors -- and runs each
against a vehicle the test plays. The two must print the same lines, and the
right ones, and they compare what they read against the header's own
constants, so a header whose numbers had drifted from the library's fails.
The header alone is also compiled as C99 and as C++11.

## Known limits

- **A query's counts are not given.** `tcs_client_query_node` says whether a
  node is running; the counts of what it has moved, which tcslib's answer
  carries, have no C spelling yet.
- **One thread per handle.** A handle is used from one thread at a time;
  nothing in the library locks one. `tcs_last_error()` is per thread.
- **A beacon pass may wait for ever.** Before any beacon has arrived,
  `tcs_beacons_pass` waits for one, with no timeout: run it on a thread of
  its own, as tcsmoc does.
- **Strings are borrowed.** A string a function returns is good until the
  next call on the same handle or its `_close`. Copy it to keep it.
- **Linking.** On Linux, `libtcs.a` needs `-lpthread -ldl -lm` beside it,
  which is what the tests link with. That is enough where the C library has
  folded the rest into itself, as recent glibc has; the whole list for a
  system is what `cargo rustc -p tcslib-c --lib --crate-type staticlib --
  --print native-static-libs` prints, which here is `-lgcc_s -lutil -lrt
  -lpthread -lm -ldl -lc`. Nothing but Linux has been built or tested.
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
    checks a whole configuration, as tcslib does; `tcs_gateway_open` makes the
    names of the node's sources, which every receive then points at, and the
    buffer every datum is received in; and `tcs_configuration_node_id` asks
    tcslib for every node's id.
  - *In tcslib, as a station runs, only where something happens.* A beacon
    pass, a datum, a ping and its answer, and a command sent allocate
    nothing. What does: a remark or warning, on the pass where something
    changed; an answer to a command, whose strings tcslib reads as its own;
    and what came down from the ping node that is not an answer, to say why.

`make` in this directory builds `README.html` from this file.

## See also

- [This page as HTML](https://vomlehn.github.io/tcspecial/tcslib-c/README.html).
- [tcslib](https://vomlehn.github.io/tcspecial/tcslib/README.html), the Rust library this one wraps, and [tcslibgs](https://vomlehn.github.io/tcspecial/tcslibgs/README.html), what the ground and the vehicle share.
- [The tcspecial manual](https://vomlehn.github.io/tcspecial/docs/manual.html) and [design](https://vomlehn.github.io/tcspecial/docs/design.html).

## License

Licensed under either of the Apache License, Version 2.0 ([LICENSE-APACHE](https://github.com/vomlehn/tcspecial/blob/main/LICENSE-APACHE)
or <http://www.apache.org/licenses/LICENSE-2.0>) or the MIT license
([LICENSE-MIT](https://github.com/vomlehn/tcspecial/blob/main/LICENSE-MIT) or <http://opensource.org/licenses/MIT>), at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

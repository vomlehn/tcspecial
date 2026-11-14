# tcspecial

tcspecial is "telemetry and control, special": flight software that runs a
spacecraft's payloads as nodes in a configured graph, together with the ground
software that commands it and the tools that write and check its
configuration. Each program has a design document, its specification, and a
manual, its user's guide: `docs/design.rst` and `docs/manual.rst` list them
(`docs/tcspecial-design.rst`, `docs/tcs-gui.rst` and so on).

## What decides a design

The qualities are ranked, and the ranking settles a trade-off:
**reliability**, then **ease of use**, then **maintainability**, then
**flexibility**. A configuration refused when it is loaded beats one that
fails in flight. A fixed, checkable shape beats a clever, dynamic one.
Extensibility that costs reliability is not wanted. When two designs compete,
argue from this order and say so.

## The crates

- `tcsprotocol` -- what the ground and the vehicle share: the command and
  telemetry protocol, and how every configuration file is read and checked.
  A rule about what a configuration may say belongs here, and nowhere else.
- `tcsmoc` -- what a ground station is built on (only in the MOC):
  `TcsClient` for commands, `gateway::GatewayReceiver` for the vehicle's data,
  `beacon` for its beacons, `simulator` for waiting until tcs-sim has made its
  simulated devices, `bus` for a vehicle on a cFS software bus (the station's
  SBN peer, and `SimulatedCfs`, the cFS end as tcs-sim simulates it).
- `tcsmoc-c` -- tcsmoc for C and C++: beacons, data and commands behind
  one hand-written header, `include/tcsmoc-c.h`, which `tests/c_and_cpp.rs`
  holds to the library by compiling a ground station against it with gcc and
  g++.
- `tcspecial` -- the flight software.
- `tcs-moc` -- the test ground station (GUI).
- `tcs-sim` -- the payload simulator (GUI).
- `tcs-gui` -- the graphical configuration editor.
- `tcsverify` -- checks a configuration and gives every problem with its
  file and line. tcspecial, tcs-verify, tcs-moc, tcs-sim and tcs-gui all
  check through its `verify`, so they agree about what is wrong.
- `tcs-verify` -- the program that prints those problems, and what only the
  tool does: `--summary`, `--set-interface`, `--xtce`, `--fprime-model`, and
  the start-up window tcs-moc and tcs-sim share.
- `tcsui` -- window pieces the ground programs share.
- `tcs-diagram` -- how a configuration is drawn: the working panes, node
  shapes, link routes and colours, as Rust and as `ui/diagram.slint`. tcs-gui
  draws with it, and so does tcspecial's visual window (`--visual`, in a
  build with the `visual` feature, which never flies).
- `tcsapp` and the `app_*` crates -- the file interfaces an app node's program
  runs with, and the five test apps.
- `examples/` -- configurations and programs; see `examples/README.md`.

A node type is three modules with the same name in three crates: its
attributes and checks in `tcsprotocol::node`, the flight-side node in
`tcspecial::node`, and the simulator's simulated device in `tcs_sim::node`.

## Vocabulary

- **node**: anything in the graph -- a device reached over an interface, a
  gateway, a computation, a log, an app.
- **gateway**: the side of the graph that commands tcspecial and takes its
  telemetry, and the node it is reached through: `gateway_radio` for the
  ground, `gateway_fprime` for an F´ computer, `gateway_cfs` for cFS's
  software bus, joined as an SBN peer. Its section is `gateway:`, and
  `tee-gateway`, `null-gateway` and `app_gateway_veh` stand beside it. All
  of these were `moc` once; a file still saying so is refused, told the new
  word (`tcsprotocol::graph::renamed`).
- **MOC**: the mission operations center, on the ground. Still the word for
  the ground itself -- tcs-moc, "the MOC sends" -- and for the terminals
  below, but never for a node.
- **terminals**: `world-in` (upper left), `world-out` (upper right),
  `moc-out` (lower left), `moc-in` (lower right). `world-` carries a
  payload's readings towards the ground, `moc-` what the ground sends back.
  A link is written under the source as `node.name`, and the node it
  reaches lists `name` under the sink -- `world-in: [readings]`; a vote's
  world-in names are its inputs. They were `payload-` and `gateway-`, and
  a file still saying so is refused, told the new word.
- **device file**: a filesystem entry, `device_file` in code and `type:`.
- App node types are named stream-then-side: `app_payload_veh`,
  `app_gateway_veh` and `app_all_veh` on the vehicle; `app_pl` and
  `app_all_pl` on the payload side.
- Older code still says `payload`, `PayloadConfig`, `payloads:`. That rename
  is held back on purpose until the configuration schema is complete, because
  those names mirror configuration keys. Do not start it piecemeal.

## Fixed facts

- The link between the MOC and tcspecial is UDP, and will never be TCP.
  (TCP payloads are a different thing and are supported.)
- C support always means C and C++: every C interface must also work from
  C++ (headers includable from C++ as they stand, `extern "C"` guards), and
  is built, tested and documented as C/C++.
- Configurations are YAML or XML, chosen by the file's extension. The two
  spellings of a configuration must say the same thing, so change both
  together. `tcs-verify a.xtcs a.ytcs a-sim.yaml` compares them, and
  `make comparespellings`, part of `make check`, compares every shipped
  pair. tcs-gui writes both spellings when both exist; an edit by hand
  is made to both.

## Building and testing

```
cargo build --workspace
cargo test --workspace          # before claiming anything works
cargo clippy --workspace --all-targets
cargo run -q --bin tcs-verify -- <configuration> [simulator file]
make html-check                 # every committed page made again, and current
```

`make help` lists the Makefile's targets. `make` in `docs/`, `examples/`,
`tcsmoc/`, `tcsmoc-c/` and `tcsprotocol/` builds their HTML pages.

tcspecial, tcs-sim and tcs-moc take `--simulate=none|some|all`. Left out it is
`none`, the flight setting, under which tcspecial refuses every simulated
and dummy device and tcs-sim will not start, so a run on a desk gives
`--simulate=all`, as the Makefiles' run targets do. tcspecial judges each
payload a simulated device (tcs-sim's: a pseudo-terminal, a socket), a dummy
device (the kernel's: i2c-stub, gpio-sim, vcan, iio_dummy) or a real device
from what is at its endpoint (`tcsprotocol::simulate`), never from a file.
`network` and `device-file` nodes are always real devices and not judged.
Which payloads are meant to be simulated is the simulator file's `simulate`,
which tcs-sim holds to the same judgement; tcspecial never reads that file.
An hdlc, spi, rpmsg, iio, i2c_tmass or spi_tmass node may give
`sim_address`, a loopback UDP address it is reached at instead of its device
under `some` and `all`; the old name, `stand_in`, is refused.

The GUIs and tcspecial may be launched when what is being checked can only be
seen by running them, but `cargo test` comes first. They bind fixed ports:
commands on 4000, payload commands on 4200, beacons on 5550 and 5650, payload
telemetry on 6000 and 6100. tcs-moc starts tcspecial and tcs-sim itself, and
they are sent SIGTERM when it ends, however it ends, even by `kill -9`. Ones
started some other way are not, so look for strays on those ports afterwards.

## Working in this tree

- **The tree is edited by more than one person at once.** Run
  `git status --short` before staging and account for every line. Stage
  files by name, never `git add -A` or `git add .`. Commit with
  `git commit -- <paths>` when someone else has things staged, so their
  staging survives. If a file you meant to commit holds hunks you did not
  write, commit only your hunks, or ask.
- When a build breaks in a crate you did not touch, it is probably someone's
  work in progress. Test around it (`--exclude <crate>`) and say so; do not
  "fix" it.
- Keep the docs current with each change. A requirement in a design document
  (`docs/*-design.rst`) that the code no longer meets is a bug in one or the
  other.
- A test for a fix should fail without the fix. Check that it does.

## Style

- No dynamic memory allocation after a program enters its main loop -- the
  loop it runs in for as long as it runs, and anything called from it.
  Startup may allocate; the loop reuses what startup set up: fixed-size
  buffers, arrays, stack storage. Where an allocation in the loop is
  absolutely unavoidable, say so in a comment at that spot, with why.
- Comments say why, in full sentences, as the surrounding code does. Match
  the density and voice of the file being edited.
- Error messages name what is wrong and what to do about it, in the user's
  terms, such as which attribute or line to change, not in the parser's.
- Each rule is stated once, in one place, and everything that needs it calls
  that place.
- Makefiles: each phony target has its own `.PHONY: <target>` line directly
  above its rule, never one combined list.
- The tree is kept under `rustfmt` with its defaults: run `cargo fmt --all`
  before committing, and `make check`'s `cargo fmt --check` step must pass.
  The commit that first formatted the tree is listed in
  `.git-blame-ignore-revs`; run
  `git config blame.ignoreRevsFile .git-blame-ignore-revs` once so that
  `git blame` looks through it.

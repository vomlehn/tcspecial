# tcspecial

tcspecial is "telemetry and control, special": flight software that runs a
spacecraft's payloads as nodes in a configured graph, together with the ground
software that commands it and the tools that write and check its
configuration. `docs/design.rst` is the specification and `docs/manual.rst`
the user's guide.

## What decides a design

The qualities are ranked, and the ranking settles a trade-off:
**reliability**, then **ease of use**, then **maintainability**, then
**flexibility**. A configuration refused when it is loaded beats one that
fails in flight. A fixed, checkable shape beats a clever, dynamic one.
Extensibility that costs reliability is not wanted. When two designs compete,
argue from this order and say so.

## The crates

- `tcslibgs` -- what the ground and the vehicle share: the command and
  telemetry protocol, and how every configuration file is read and checked.
  A rule about what a configuration may say belongs here, and nowhere else.
- `tcslib` -- what a ground station is built on: `TcsClient` for commands,
  `moc::Moc` for the vehicle's data, `beacon` for its beacons.
- `tcspecial` -- the flight software.
- `tcsmoc` -- the test ground station (GUI).
- `tcssim` -- the payload simulator (GUI).
- `tcsgui` -- the graphical configuration editor.
- `tcsverify` -- checks a configuration and prints every problem with its file
  and line. tcspecial, tcsmoc, tcssim and tcsgui all check through its
  `verify`, so they agree about what is wrong.
- `tcsui` -- window pieces the ground programs share.
- `tcsapp` and the `app_*` crates -- the file interfaces an app node's program
  runs with, and the five test apps.
- `examples/` -- configurations and programs; see `examples/README.md`.

A node type is three modules with the same name in three crates: its
attributes and checks in `tcslibgs::node`, the flight-side node in
`tcspecial::node`, and the simulator's stand-in in `tcssim::node`.

## Vocabulary

- **node**: anything in the graph -- a device reached over an interface, a
  link to the MOC, a computation, a log, an app.
- **MOC**: the mission operations center, on the ground.
- **device file**: a filesystem entry, `device_file` in code and `type:`.
- App node types are named stream-then-side: `app_payload_veh`,
  `app_moc_veh` and `app_all_veh` on the vehicle; `app_pl` and `app_all_pl`
  on the payload side.
- Older code still says `payload`, `PayloadConfig`, `payloads:`. That rename
  is held back on purpose until the configuration schema is complete, because
  those names mirror configuration keys. Do not start it piecemeal.

## Fixed facts

- The link between the MOC and tcspecial is UDP, and will never be TCP.
  (TCP payloads are a different thing and are supported.)
- Configurations are YAML or XML, chosen by the file's extension. The two
  spellings of a configuration must say the same thing, so change both
  together. `tcsverify a.xml a.yaml a-sim.yaml` compares them.

## Building and testing

```
cargo build --workspace
cargo test --workspace          # before claiming anything works
cargo clippy --workspace --all-targets
cargo run -q --bin tcsverify -- <configuration> [simulator file]
```

`make help` lists the Makefile's targets. `make` in `docs/`, `examples/`,
`tcslib/` and `tcslibgs/` builds their HTML pages.

The GUIs and tcspecial may be launched when what is being checked can only be
seen by running them, but `cargo test` comes first. They bind fixed ports:
commands on 4000, payload commands on 4200, beacons on 5550 and 5650, payload
telemetry on 6000 and 6100. tcsmoc starts tcspecial and tcssim itself, so
killing tcsmoc can leave them running; look for strays afterwards.

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
- Keep the docs current with each change. A requirement in `design.rst` that
  the code no longer meets is a bug in one or the other.
- A test for a fix should fail without the fix. Check that it does.

## Style

- Comments say why, in full sentences, as the surrounding code does. Match
  the density and voice of the file being edited.
- Error messages name what is wrong and what to do about it, in the user's
  terms, such as which attribute or line to change, not in the parser's.
- Each rule is stated once, in one place, and everything that needs it calls
  that place.
- Makefiles: each phony target has its own `.PHONY: <target>` line directly
  above its rule, never one combined list.
- The tree is not kept under `rustfmt`: it would change most files, so
  `make check`'s `cargo fmt --check` step fails. Do not run `cargo fmt` over
  files you are editing, because it would reformat code you did not change.

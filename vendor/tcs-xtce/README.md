# tcs-xtce, vendored

A copy of the parts of tcs-xtce this tree uses, taken from
`github.com:vomlehn/tcs-xtce` (commit fa667a9, of 2026-10-21) because
tcs-xtce is not yet published. **When tcs-xtce is on crates.io, replace this
directory with a dependency on it**: `xtce = "<version>"` in
`tcslibgs/Cargo.toml` in place of the path, the `exclude` in the workspace's
`Cargo.toml` removed, and the generator and Python runtime taken from the
release.

tcs-xtce compiles an XTCE description of a packet into code that packs and
unpacks it. This tree uses it for one packet, the beacon: the layout is
`tcslibgs/xtce/beacon.xml`, and `make beacon-packet` in `tcslibgs/` runs the
generator here on it to write `tcslibgs/src/beacon_packet.rs` and
`python/tcslibgs/beacon_packet.py`. See `tcslibgs/src/beacon.rs`.

| Directory | What it is | Used by |
|---|---|---|
| `rust/` | The `xtce` crate: the packet runtime the generated Rust calls, and the XML binding the crate also holds | `tcslibgs`, through a path dependency. Not a member of the workspace, so `cargo test`, `clippy` and `fmt` leave it alone. |
| `python/xtce/` | The same runtime in Python | `python/tcslibgs/beacon_packet.py`, which `python/tcslibgs/beacon.py` puts on the path |
| `generator/` | `tcs_xtce`, the compiler, which runs as `python3 -m tcs_xtce` because a Python module's name cannot hold a hyphen, and the two packages it is built from: `xtcepkt` and `xtcegen` | `make beacon-packet`. It reads XTCE with `python/xtce/`. |

Copied as they are, with nothing changed. To refresh the copy, copy those
directories again from a tcs-xtce checkout (`rust/` without its `tests/` or
`target/`), leaving out `__pycache__`, then run `make beacon-packet` in
`tcslibgs/` and `cargo test --workspace`.

tcs-xtce is licensed under either of the Apache License, Version 2.0, in
`LICENSE-APACHE`, or the MIT license, in `LICENSE-MIT`, at your option; its
Rust crate says `MIT OR Apache-2.0`.

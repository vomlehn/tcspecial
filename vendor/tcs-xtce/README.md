# tcs-xtce, vendored

A copy of the parts of tcs-xtce this tree uses, taken from
`github.com:vomlehn/tcs-xtce` (commit 59facf3, of 2026-10-28) because
tcs-xtce is not yet published. **When tcs-xtce is on crates.io, replace this
directory with a dependency on it**: `tcs-xtce = "<version>"` in
`tcslibgs/Cargo.toml` in place of the path, the `exclude` in the workspace's
`Cargo.toml` removed, and the generator and Python runtime taken from the
release.

tcs-xtce compiles an XTCE description of a packet into code that packs and
unpacks it. This tree uses it for one packet, the beacon: the layout is
`tcslibgs/xtce/beacon.xml`, and `make beacon-packet` in `tcslibgs/` runs the
generator here on it to write `tcslibgs/src/beacon_packet.rs`, without
dynamic allocation, and `python/tcslibgs/beacon_packet.py`. See
`tcslibgs/src/beacon.rs`. It also computes the digest of the interface every
beacon carries -- `make interface-digest` there, and `tcslibgs::interface` and
`python/tcslibgs/interface.py` at run time -- and the CRC that ends every
beacon and tcspecial's state file.

| Directory | What it is | Used by |
|---|---|---|
| `rust/` | The `tcs-xtce` crate: the packet runtime the generated Rust calls, its fixed-capacity buffers and its digest, and the XML binding the crate also holds | `tcslibgs`, through a path dependency. Not a member of the workspace, so `cargo test`, `clippy` and `fmt` leave it alone. |
| `python/xtce/` | The same runtime in Python, digest included | `python/tcslibgs/beacon_packet.py`, which `python/tcslibgs/beacon.py` puts on the path, and `python/tcslibgs/interface.py` |
| `generator/` | `tcs_xtce`, the compiler, which runs as `python3 -m tcs_xtce` because a Python module's name cannot hold a hyphen, and the two packages it is built from: `xtcepkt` and `xtcegen`; and `xtce_digest`, the digest's command line | `make beacon-packet` and `make interface-digest`. They read XTCE with `python/xtce/`. |

Copied as they are, with nothing changed. To refresh the copy, copy those
directories again from a tcs-xtce commit (`rust/` without its `tests/`,
`examples/` or `target/`), leaving out `__pycache__`. Take them from the
commit -- `git archive` -- and not from a working tree, which may hold work in
progress. `rust/src/generated.rs` and `python/xtce/generated.py` are not in
the commit: `make rust/src/generated.rs python/xtce/generated.py` in the
exported copy writes them from the XTCE schema. Then run `make beacon-packet
interface-digest` in `tcslibgs/` and `cargo test --workspace`.

tcs-xtce is licensed under either of the Apache License, Version 2.0, in
`LICENSE-APACHE`, or the MIT license, in `LICENSE-MIT`, at your option; its
Rust crate says `MIT OR Apache-2.0`.

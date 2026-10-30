# tcs-xtce, vendored

A copy of the parts of tcs-xtce this tree uses that are not published,
taken from `github.com:vomlehn/tcs-xtce` (commit 7a51180, of 2026-10-30,
which is release 0.3.0). The Rust runtime is published, and `tcslibgs` takes
it from crates.io: `tcs-xtce = "=0.3.0"` in `tcslibgs/Cargo.toml`. The
generator and the Python runtime are not, so they are here. **The copy and
the crate must be of the same release**, as the code the generator writes
calls the runtime, so the version is pinned exactly and changes with the
copy. When the generator and the Python runtime are published, replace this
directory with them.

tcs-xtce compiles an XTCE description of a packet into code that packs and
unpacks it. This tree uses it for the link's commands and answers: the
layout is `tcslibgs/xtce/link.xml`, and `make link-packet` in `tcslibgs/`
runs the generator here on it to write `tcslibgs/src/link_packet.rs`,
without dynamic allocation. See `tcslibgs/src/link.rs`. It also computes the
digest of the interface every beacon carries -- `make interface-digest`
there, and `tcslibgs::interface` and `python/tcslibgs/interface.py` at run
time -- and the CRC that ends every packet and tcspecial's state file.

| Directory | What it is | Used by |
|---|---|---|
| `python/xtce/` | The packet runtime in Python, as the crate is in Rust, digest included | `python/tcslibgs/interface.py` |
| `generator/` | `tcs_xtce`, the compiler, which runs as `python3 -m tcs_xtce` because a Python module's name cannot hold a hyphen, and the two packages it is built from: `xtcepkt` and `xtcegen`; and `xtce_digest`, the digest's command line | `make link-packet` and `make interface-digest`. They read XTCE with `python/xtce/`. |
| `tests/doc_headers.py` | tcs-xtce's check that every non-trivial function has a header in its language's convention | `make doc-headers`, which `make test` runs |

Copied as they are, with nothing changed. To refresh the copy, take a
tcs-xtce release: copy `generator/` without its `tests/`, `python/xtce/`
and `tests/doc_headers.py` again from its commit, leaving out
`__pycache__`, and set the version in `tcslibgs/Cargo.toml` to the
release's. Take them from the commit -- `git archive` -- and not from a
working tree, which may hold work in progress. `python/xtce/generated.py`
is not in the commit: `make python/xtce/generated.py` in the exported copy
writes it from the XTCE schema. Then run `make link-packet
interface-digest` in `tcslibgs/` and `make test`.

tcs-xtce is licensed under either of the Apache License, Version 2.0, in
`LICENSE-APACHE`, or the MIT license, in `LICENSE-MIT`, at your option.

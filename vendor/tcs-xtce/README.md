# tcs-xtce, vendored

A copy of the parts of tcs-xtce this tree uses that are not published: its
Python package, `tcs_xtce`, which holds the generator and the Python runtime.
Taken from `github.com:vomlehn/tcs-xtce` (commit 7ad16b9, of 2026-10-30,
after release 0.3.0, when the package took the name `tcs_xtce`). The Rust
runtime is published, and `tcslibgs` takes it from crates.io:
`tcs-xtce = "=0.3.0"` in `tcslibgs/Cargo.toml`. **The code the generator
writes calls the crate, so the two must agree**: the version is pinned
exactly, and changes with the copy. This copy is newer than 0.3.0 only in
how the generator handles XTCE that `tcslibgs/xtce/link.xml` does not use,
and the code it writes for this tree calls nothing 0.3.0 lacks -- `make
link-packet` writes what is committed. Take the next copy from a release
and set the crate's version to match. When the Python package is
published, replace this directory with it.

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
| `python/tcs_xtce/` | The `tcs_xtce` package: the Python runtime at its top, digest included; the compiler, in `tcs_xtce.compiler`, which runs as `python3 -m tcs_xtce` from `python/`; the XML binding's generator in `tcs_xtce.binding`, whose line wrapping the compiler uses; and the digest's command line, `python3 -m tcs_xtce.digest_cli` | `python/tcslibgs/_xtce/`, whose links to its digest modules `python/tcslibgs/interface.py` uses, and `make link-packet` and `make interface-digest` in `tcslibgs/` |
| `tests/doc_headers.py` | tcs-xtce's check that every non-trivial function has a header in its language's convention | `make doc-headers`, which `make test` runs |

Copied as they are, with nothing changed. To refresh the copy, take a
tcs-xtce release: copy `python/tcs_xtce/` and `tests/doc_headers.py` again
from its commit, leaving out `__pycache__`, and set the version in
`tcslibgs/Cargo.toml` to the release's. Take them from the commit -- `git
archive` -- and not from a working tree, which may hold work in progress.
`python/tcs_xtce/generated.py` is not in the commit: `make
python/tcs_xtce/generated.py` in the exported copy writes it from the XTCE
schema. Then run `make link-packet interface-digest` in `tcslibgs/` and
`make test`.

tcs-xtce is licensed under either of the Apache License, Version 2.0, in
`LICENSE-APACHE`, or the MIT license, in `LICENSE-MIT`, at your option.

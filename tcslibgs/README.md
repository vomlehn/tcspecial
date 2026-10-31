# tcslibgs

**tcslibgs is what the ground software and the vehicle software share.** The
"gs" is ground and space. Anything both ends of the link have to agree on is
written here, once, and both ends use it: the vehicle (tcspecial) on one side,
the ground stations and tools on the other.

That agreement covers two things.

**The link itself.** The commands the ground sends, the telemetry and beacons
the vehicle answers with, and how each is encoded. If the two ends defined
these separately, a change to one would be a link that no longer works.

**The configuration.** Every configuration file is read here, in YAML or XML,
and every rule about what a configuration may say is checked here. A
configuration describes the vehicle as a graph of nodes and the links between
them (`graph`), and every program that opens one reads it through this crate:

- tcspecial runs it aboard.
- tcsmoc and tcssim build their panels from it.
- tcsgui edits it.
- tcsverify checks it.

So no program reads a file in a way of its own, and the error a person sees in
the editor is the one the vehicle would have refused the file with.

## What is in it

- `commands`, `telemetry`, `protocol`: the command and telemetry protocol.
- `link`: the commands and their answers, with no header: an opcode, a
  sequence and what each carries. The three fixed commands -- CONNECT,
  RESTART_ARM and RESTART -- are described in XTCE in `xtce/fixed.xml`,
  which never changes, and every other command in `xtce/link.xml`. The
  code that packs and unpacks them, `src/fixed_packet.rs` and
  `src/link_packet.rs`, is generated from those files, and the Python
  `../python/tcslibgs/fixed_packet.py` from `fixed.xml`: run `make
  link-packet` here after changing `link.xml`, and `make interface-digest`.
  A test pins `fixed.xml`'s digest, and fails, saying it must never change,
  if it does.
- `beacon`: the beacon, a line of text with six fields between colons,
  `tcspecial:1.2.3:<md5>:42:normal:<interface>`, at most 106 bytes, so that
  a person can read it in a capture or a log with no decoder. Its layout is
  in `xtce/fixed.xml` beside the fixed commands, as text fields, packed and
  unpacked by the generated code; what each field says is checked here and
  in `../python/tcslibgs/beacon.py`, held to each other by the same examples
  in their tests.
- `interface`: the digest of `xtce/link.xml`, which every beacon carries,
  and which every configuration names as its `tcspecial` section's
  `interface`, so that a configuration written for another interface is
  refused by every program. `tcsverify --interface-digest` prints it. `make
  interface-digest` here regenerates `src/interface_digest.rs` after
  `link.xml` changes; every configuration is then given the new digest.
- `crc`: the CCSDS CRC-16, for what is kept on storage rather than carried
  by a transport that checks it, such as tcspecial's state file.
- `config_digest`: the digest of a configuration, which both ends report, so
  each can tell whether the other read the same files.
- `graph`: the configuration, a graph of nodes, with its checks, and the
  ports each link to the ground arrives at.
- `node`: one module per node type, giving each type's attributes and how
  they are checked. tcspecial and tcssim have trees of the same shape, one
  for the node aboard and one for the simulator's stand-in for it.
- `format`, `config`, `types`, `endpoint_config`, `whole`, `trigger`: reading
  the files. One parser per format, one set of types for both, one grammar
  for whole numbers, one for trigger bytes.
- `sim_config`: the simulator's own configuration file.
- `verify`: what a problem in a configuration is about, so that tcsverify and
  the editor can put it beside the line that has to change.
- `parameters`, `display`: what both test windows, tcsmoc and tcssim, show
  about a payload, and where on the screen each window goes.
- `error`: the errors all of the above return.

## What tcslibgs is not

It has no network code for the ground. Opening a link to the vehicle,
sending a command, receiving data and listening for beacons are all in
`tcslib`, which builds on this crate and re-exports it. The flight software
itself is `tcspecial`.

## Who uses it

Every crate in the workspace that has anything to do with the link or a
configuration: tcspecial, tcslib, tcsmoc, tcssim, tcsgui and tcsverify, and
`examples/beacon`.

## Building and testing

```
cargo build -p tcslibgs
cargo test -p tcslibgs
cargo doc -p tcslibgs --open
```

`make` in this directory builds `README.html` from this file, and
`make clean` removes it.

## See also

- [This page as HTML](https://vomlehn.github.io/tcspecial/tcslibgs/README.html).
- [tcslib](https://vomlehn.github.io/tcspecial/tcslib/README.html), what a ground station is built on, and [tcslib-c](https://vomlehn.github.io/tcspecial/tcslib-c/README.html), the same for C and C++.
- [The tcspecial manual](https://vomlehn.github.io/tcspecial/docs/manual.html) and [design](https://vomlehn.github.io/tcspecial/docs/design.html).

## License

Licensed under either of the Apache License, Version 2.0 ([LICENSE-APACHE](https://github.com/vomlehn/tcspecial/blob/main/LICENSE-APACHE)
or <http://www.apache.org/licenses/LICENSE-2.0>) or the MIT license
([LICENSE-MIT](https://github.com/vomlehn/tcspecial/blob/main/LICENSE-MIT) or <http://opensource.org/licenses/MIT>), at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

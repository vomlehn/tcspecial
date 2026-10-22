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
- `beacon`: the beacon, which is not JSON as the answers to commands are but
  34 bytes in a fixed layout, described in XTCE in `xtce/beacon.xml`. The code
  that packs and unpacks it, `src/beacon_packet.rs` and
  `../python/tcslibgs/beacon_packet.py`, is generated from that file by
  tcs-xtce (vendored in `../vendor/tcs-xtce`): run `make beacon-packet` here
  after changing it.
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

# tcslib

**tcslib is the library a ground station is built on.** It is the ground's
side of the link to a spacecraft running tcspecial: everything a program on
the ground needs in order to command the vehicle, receive the vehicle's data,
and know that the vehicle is there.

A ground station does three things with a spacecraft, and tcslib has one part
for each:

- **Commanding it.** `TcsClient` sends commands to tcspecial's command
  interpreter and returns its answers: ping it, start, stop, set and query a
  node, arm and restart the vehicle, change how often it beacons. A mission
  control system such as YAMCS or MCT drives a `TcsClient` instead of speaking
  the command and telemetry protocol itself.
- **Receiving its data.** `moc::Moc` is the ground's end of a `moc` node in a
  configuration. Name the node, open it, and each call to `receive` returns
  one datum along with the name of the node that produced it. The ports the
  data arrive on are worked out from the configuration, the same way
  tcspecial works them out, so the two ends cannot disagree about them.
- **Knowing it is there.** `beacon` listens for the beacon tcspecial sends on
  its own, every `beacon.interval`, whether or not anyone has connected. It
  says which build is running, which configuration it is serving and the
  digest of the files it read. It is the one thing a ground station can know
  about a spacecraft without having asked it anything.

The link to tcspecial is UDP, and only UDP: `TcsClient` sends its commands
through a `UdpConnection`.

## What tcslib is not

tcslib has no windows. Showing what it receives is the job of the programs
built on it: `tcsui` holds the panels the ground programs share, and `tcsmoc`
is the test ground station that puts them on the screen.

tcslib does not define the link either. The commands, the telemetry, and the
configuration files both ends read belong to `tcslibgs`, which the vehicle
uses too. tcslib re-exports all of it, so a ground program needs only tcslib.

## Who uses it

- `tcsmoc`, the test ground station.
- `tcsui`, the window pieces shared by the ground programs.
- `examples/simple-app`, whose ground station `lorem_moc` gets its data with
  one call to `Moc::receive_into`, into a buffer it made beforehand.
- `examples/beacon`, whose window watches the beacon through `beacon`.

## Building and testing

```
cargo build -p tcslib
cargo test -p tcslib
cargo doc -p tcslib --open
```

`make` in this directory builds `README.html` from this file, and
`make clean` removes it.

## See also

- [This page as HTML](https://vomlehn.github.io/tcspecial/tcslib/README.html).
- [tcslibgs](https://vomlehn.github.io/tcspecial/tcslibgs/README.html), what the ground and the vehicle share, and [tcslib-c](https://vomlehn.github.io/tcspecial/tcslib-c/README.html), this library for C and C++.
- [The tcspecial manual](https://vomlehn.github.io/tcspecial/docs/manual.html) and [design](https://vomlehn.github.io/tcspecial/docs/design.html).

## License

Licensed under either of the Apache License, Version 2.0 ([LICENSE-APACHE](https://github.com/vomlehn/tcspecial/blob/main/LICENSE-APACHE)
or <http://www.apache.org/licenses/LICENSE-2.0>) or the MIT license
([LICENSE-MIT](https://github.com/vomlehn/tcspecial/blob/main/LICENSE-MIT) or <http://opensource.org/licenses/MIT>), at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

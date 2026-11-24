//! tcs-relay: relays between a ground station and a vehicle that hold each
//! datagram a while, lose some, or both, for a ground with no link of its
//! own to simulate them in. All in the library: see `tcs_moc::relay`.

/// Runs tcs-relay.
fn main() {
    tcs_moc::relay::main();
}

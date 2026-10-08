//! A simulated payload on an I2C bus, by way of the kernel's `i2c-stub`.
//!
//! A bus is the one kind of endpoint the simulator cannot simply be the far
//! end of. A serial line has a pty; a socket has another socket; a device on
//! a bus has to answer an address when a master addresses it, and nothing in
//! user space can. What the kernel offers instead is `i2c-stub`: an adapter
//! whose chips are a bank of registers in memory. Every master on that bus
//! reads and writes the same bank, so the bank is the payload -- tcspecial
//! reads what the simulator has written into it.
//!
//! That makes a simulated bus a register rather than a stream, and the
//! difference shows: bytes do not queue. A handler reads whatever is in the
//! register now, so it may read one packet twice or miss one entirely,
//! depending on how its polling falls against the packet interval. Which is
//! also true of a real sensor read over I2C, and is why a payload link over a
//! bus is a different thing from one over a line.
//!
//! The stub is a kernel module and loading it needs root, so the simulator
//! does not create the bus: it finds one and says how to make one if there is
//! none. The chips are fixed when the module is loaded, too, which is why the
//! address a payload file gives has to be one the stub was loaded with.
//!
//! See [`crate::payload`] for the pacing, which is common to every kind.

use std::fs::File;
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use rand::Rng;

use crate::payload::{
    make_the_path_lead_to, send_in_segments, wait_out_packet, Pacing, PayloadConfig, PayloadStats,
};

/// Where the kernel lists its I2C adapters, and what the stub's is called.
///
/// A constant so that a test can look at a tree of its own: the scan is the
/// part of this that can be tested without the module loaded.
const I2C_ADAPTERS: &str = "/sys/bus/i2c/devices";
const STUB_NAME: &str = "SMBus stub driver";

/// Requests of an open bus, from `linux/i2c-dev.h`. The same numbers
/// tcspecial's I2C endpoint uses, for the same reasons.
const I2C_SLAVE: libc::c_ulong = 0x0703;
const I2C_SMBUS: libc::c_ulong = 0x0720;

const I2C_SMBUS_WRITE: u8 = 0;
const I2C_SMBUS_I2C_BLOCK_DATA: u32 = 8;

/// The most bytes one SMBus block transfer carries, so the most a segment of a
/// packet can be on this kind of endpoint.
pub(crate) const I2C_SMBUS_BLOCK_MAX: usize = 32;

/// The register the payload's data goes into: the same one tcspecial reads.
const PAYLOAD_REGISTER: u8 = 0;

/// One SMBus transfer, as `linux/i2c-dev.h` describes it.
#[repr(C)]
struct SmbusTransfer {
    read_write: u8,
    command: u8,
    size: u32,
    data: *mut SmbusData,
}

#[repr(C)]
struct SmbusData {
    block: [u8; I2C_SMBUS_BLOCK_MAX + 2],
}

/// A stub bus, with the configured path leading to it and one chip addressed.
struct SimulatedBus {
    bus: File,
    /// The path a handler was told to open, which is a link to the stub.
    named: PathBuf,
    /// Whether the link is this payload's to remove.
    linked: bool,
}

impl SimulatedBus {
    /// Find a stub bus, point `path` at it, and address the chip.
    fn open(path: &str, address: u16) -> Result<Self, String> {
        let stub = find_stub(Path::new(I2C_ADAPTERS))?;
        let named = PathBuf::from(path);

        // A payload file may name the stub itself, in which case there is
        // nothing to link: the handler and the simulator open the same node.
        let linked = named != Path::new(&stub);
        if linked {
            make_the_path_lead_to(&named, &stub)?;
        }

        let bus = File::options()
            .read(true)
            .write(true)
            .open(&stub)
            .map_err(|e| {
                format!(
                    "{stub} cannot be opened: {e}. A stub bus belongs to root unless \
                     something has been done about it -- a udev rule, or a group"
                )
            })?;

        // SAFETY: the bus is open, and the request takes an integer by value.
        if unsafe { libc::ioctl(bus.as_raw_fd(), I2C_SLAVE, address as libc::c_int) } < 0 {
            return Err(format!(
                "{stub} would not take the address {address:#04X}: {}",
                std::io::Error::last_os_error()
            ));
        }

        let simulated = Self { bus, named, linked };

        // The stub's chips are fixed when the module is loaded, so an address
        // nobody asked it for answers nothing. Found out here, by writing to
        // it, rather than left for the handler to discover as silence.
        simulated.prove_a_chip_answers(&stub, address)?;

        eprintln!(
            "simulated I2C device {address:#04X} on {stub}{}",
            if simulated.linked {
                format!(", reached as {}", simulated.named.display())
            } else {
                String::new()
            }
        );

        Ok(simulated)
    }

    /// Write one byte to the payload register and require it to be taken.
    ///
    /// `ENXIO` here means the stub has no chip at this address: it was loaded
    /// with others, or with none. The remedy is in the message, because
    /// "no such device or address" on a bus that is plainly there explains
    /// nothing by itself.
    fn prove_a_chip_answers(&self, stub: &str, address: u16) -> Result<(), String> {
        let mut bytes = SmbusData {
            block: [0u8; I2C_SMBUS_BLOCK_MAX + 2],
        };
        bytes.block[0] = 1;

        if let Err(e) = block_write(self.bus.as_raw_fd(), &mut bytes) {
            return Err(format!(
                "{stub} has no chip at {address:#04X} ({e}): the stub's chips are fixed \
                 when it is loaded, so load it with this one -- \
                 sudo modprobe i2c-stub chip_addr={address:#04X}"
            ));
        }
        Ok(())
    }
}

impl Drop for SimulatedBus {
    /// Take the link away, as the serial line does. The bus itself is the
    /// kernel's and stays.
    fn drop(&mut self) {
        if self.linked && self.named.is_symlink() {
            let _ = std::fs::remove_file(&self.named);
        }
    }
}

/// The stub adapter's device node, or how to make one.
///
/// Found by name rather than by number: the kernel numbers adapters as it
/// finds them, so the stub is `/dev/i2c-13` on one machine and `/dev/i2c-3`
/// on the next, and a payload file cannot know which.
fn find_stub(adapters: &Path) -> Result<String, String> {
    let entries = std::fs::read_dir(adapters)
        .map_err(|e| format!("{} cannot be read: {e}", adapters.display()))?;

    let mut found = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let number = match name.strip_prefix("i2c-").and_then(|n| n.parse::<u32>().ok()) {
            Some(number) => number,
            None => continue,
        };
        let said = match std::fs::read_to_string(entry.path().join("name")) {
            Ok(said) => said,
            Err(_) => continue,
        };
        if said.trim() == STUB_NAME {
            found.push((number, name));
        }
    }

    // By number rather than by spelling, so that i2c-4 comes before i2c-11.
    found.sort();
    match found.first() {
        Some((_, adapter)) => Ok(format!("/dev/{adapter}")),
        None => Err(format!(
            "no I2C stub bus is loaded, so there is nothing to stand in for a device \
             on one: sudo modprobe i2c-stub chip_addr=0x48, with the address the \
             payload file gives"
        )),
    }
}

/// One SMBus block write to the payload register.
fn block_write(fd: RawFd, bytes: &mut SmbusData) -> std::io::Result<()> {
    let mut transfer = SmbusTransfer {
        read_write: I2C_SMBUS_WRITE,
        command: PAYLOAD_REGISTER,
        size: I2C_SMBUS_I2C_BLOCK_DATA,
        data: bytes as *mut SmbusData,
    };

    // SAFETY: the bus is open and the transfer points at a live block for the
    // duration of the call.
    if unsafe { libc::ioctl(fd, I2C_SMBUS, &mut transfer) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Run I2C payload simulation
///
/// As every other kind: a packet every packet interval, in segments, from
/// Start until Stop. A segment here is one SMBus block transfer into the
/// chip's register bank, so no segment can exceed
/// [`I2C_SMBUS_BLOCK_MAX`] and the segment size is capped at it rather than
/// refused -- a packet size is a property of the payload, and a bus that
/// carries thirty-two bytes at a time is a property of SMBus.
pub fn run_i2c_payload(
    config: PayloadConfig,
    running: Arc<AtomicBool>,
    stats: Arc<std::sync::Mutex<PayloadStats>>,
) {
    let bus = match SimulatedBus::open(&config.address, config.bus_address) {
        Ok(bus) => bus,
        Err(e) => {
            eprintln!("Failed to stand in for the I2C device: {}", e);
            return;
        }
    };

    let mut rng = rand::thread_rng();
    let fd = bus.bus.as_raw_fd();

    while running.load(Ordering::SeqCst) {
        let started = Instant::now();
        let pacing = Pacing::read(&config);

        // Nothing is read back. A register bank has no direction: what the
        // handler writes to it is what the simulator would read, and reading
        // its own data back and counting it as received would be a lie about
        // where it came from.
        if pacing.a_packet_is_due(config.triggered, 0) {
            let packet: Vec<u8> = (0..pacing.packet_size).map(|_| rng.gen()).collect();
            let segment_size = match pacing.segment_size {
                0 => I2C_SMBUS_BLOCK_MAX,
                asked => asked.min(I2C_SMBUS_BLOCK_MAX),
            };

            let (bytes, whole) = send_in_segments(
                &packet,
                segment_size,
                pacing.segment_interval,
                &running,
                &mut |segment| {
                    let mut bytes = SmbusData {
                        block: [0u8; I2C_SMBUS_BLOCK_MAX + 2],
                    };
                    bytes.block[0] = segment.len() as u8;
                    bytes.block[1..=segment.len()].copy_from_slice(segment);
                    block_write(fd, &mut bytes).map(|()| segment.len())
                },
            );

            if bytes > 0 {
                let mut guard = stats.lock().unwrap();
                guard.bytes_sent += bytes;
                if whole {
                    guard.packets_sent += 1;
                }
            }
        }

        wait_out_packet(&pacing, started, &running);
    }

    // The link goes with the payload; see SimulatedBus::drop.
    drop(bus);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tree of adapters as the kernel lays them out, for the part of this
    /// that can be tested without the module loaded.
    fn adapters(named: &[(&str, &str)]) -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("a directory");
        for (adapter, name) in named {
            let dir = root.path().join(adapter);
            std::fs::create_dir(&dir).expect("an adapter");
            std::fs::write(dir.join("name"), name).expect("its name");
        }
        root
    }

    /// The stub is found by name, because its number is whatever the kernel
    /// gave it and a payload file cannot know that.
    #[test]
    fn the_stub_is_found_by_name_among_the_real_adapters() {
        let root = adapters(&[
            ("i2c-0", "Synopsys DesignWare I2C adapter"),
            ("i2c-7", STUB_NAME),
            ("i2c-9", "i915 gmbus tc5"),
        ]);

        assert_eq!(
            find_stub(root.path()).expect("the stub is there"),
            "/dev/i2c-7"
        );
    }

    /// With no stub loaded, the refusal is the command that loads one. A
    /// module nobody has loaded is the ordinary case, not a fault.
    #[test]
    fn with_no_stub_the_refusal_says_how_to_make_one() {
        let root = adapters(&[("i2c-0", "Synopsys DesignWare I2C adapter")]);

        let e = match find_stub(root.path()) {
            Ok(found) => panic!("a real adapter was taken for the stub: {found}"),
            Err(e) => e,
        };
        assert!(
            e.contains("modprobe i2c-stub"),
            "the refusal does not say how to make a stub: {e}"
        );
    }

    /// The scan works on the machine it is running on, whichever answer that
    /// machine has.
    ///
    /// There is no test here of a bus carrying data, and there cannot be
    /// without a stub loaded and a node somebody may open: the module needs
    /// root to load, and its node belongs to root until a udev rule or a group
    /// says otherwise. What can be required of any machine is that the scan
    /// reads the real adapters and comes back with either a stub to use or the
    /// command that makes one -- never with something unusable.
    #[test]
    fn the_real_adapters_give_a_stub_or_say_how_to_make_one() {
        match find_stub(Path::new(I2C_ADAPTERS)) {
            Ok(stub) => assert!(
                stub.starts_with("/dev/i2c-"),
                "the stub was found at {stub}, which is not a bus node"
            ),
            Err(e) => assert!(
                e.contains("modprobe i2c-stub"),
                "no stub, and no word on how to make one: {e}"
            ),
        }
    }

    /// Several stubs is not a thing the kernel usually offers, but if it did,
    /// the lowest-numbered one is chosen -- by number, so that i2c-4 comes
    /// before i2c-11, which it does not by spelling.
    #[test]
    fn the_lowest_numbered_stub_is_chosen() {
        let root = adapters(&[("i2c-11", STUB_NAME), ("i2c-4", STUB_NAME)]);
        assert_eq!(find_stub(root.path()).expect("a stub"), "/dev/i2c-4");
    }
}

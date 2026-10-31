//! Device files: a filesystem entry, opened and read or written.
//!
//! Only how a device file's stand-in is told apart is here. What a
//! `device-file` node may say is a path and the terms every endpoint takes,
//! which `crate::graph::reach` reads with the rules of
//! `crate::endpoint_config`; there is nothing of its own to check.

/// What would make a device-file payload a stand-in.
const TO_STAND_IN: &str =
    "start tcssim first, which makes a file of text at path";

/// Judges whether a device file is a stand-in: a regular file, which is what
/// tcssim makes for a device-file payload and which no hardware is, or a
/// pseudo-terminal. See [`crate::simulate`].
///
/// # Arguments
///
/// * `path` - The device file's path, from the node's `path`.
///
/// # Returns
///
/// The judgement, with what to change if it is not a stand-in.
pub fn judged(path: &str) -> crate::simulate::Judged {
    if std::fs::metadata(path).is_ok_and(|found| found.is_file()) {
        return crate::simulate::Judged::stand_in(
            format!("{path} is a regular file, which no hardware is"),
            "path",
            TO_STAND_IN,
        );
    }
    crate::simulate::pseudo_terminal(path, "path", TO_STAND_IN)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulate::Verdict;

    /// A pseudo-terminal is a stand-in, through a link to it, and so is a
    /// regular file, as tcssim makes one; a device that is neither is
    /// hardware.
    #[test]
    fn a_file_or_a_pseudo_terminal_is_a_stand_in_and_a_device_is_not() {
        let mut master = 0;
        let mut slave = 0;
        // SAFETY: openpty writes the two descriptors it opens into the two
        // integers it is given and reads nothing else.
        let opened = unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
            )
        };
        assert_eq!(opened, 0, "a pseudo-terminal opens");
        let named =
            std::fs::read_link(format!("/proc/self/fd/{slave}")).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("payload");
        std::os::unix::fs::symlink(&named, &link).unwrap();

        let judged = judged(link.to_str().unwrap());
        assert_eq!(judged.verdict, Verdict::StandIn, "{}", judged.found);
        assert!(
            judged.found.contains(&named.display().to_string()),
            "{}",
            judged.found
        );

        assert_eq!(judged_of("/dev/null"), Verdict::Hardware);
        let text = dir.path().join("text");
        std::fs::write(&text, "Lorem ipsum").unwrap();
        assert_eq!(judged_of(text.to_str().unwrap()), Verdict::StandIn);
        assert_eq!(
            judged_of(dir.path().to_str().unwrap()),
            Verdict::CannotTell
        );
        assert_eq!(judged_of("/nonexistent/payload"), Verdict::Absent);

        // SAFETY: the two descriptors openpty opened, closed once each.
        unsafe {
            libc::close(master);
            libc::close(slave);
        }
    }

    fn judged_of(path: &str) -> Verdict {
        judged(path).verdict
    }
}

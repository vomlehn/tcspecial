//! The flags that say how much a run looks for and what fails it, which
//! tcs-verify and tcs-conflict both take, spelled and read alike.

use crate::{Limits, Warnings};

/// How many errors are printed before the run stops looking: every one
/// unless asked.
///
/// Nought is a limit like any other: whether the files are good is the exit
/// status, which does not depend on how much of the reason was printed, so a
/// script that wants only the answer may ask for none of the reasons and is
/// answered at the first error.
pub const ERROR_LIMIT_FLAG: &str = "--error-limit=";

/// What is done with warnings: printed and not counted unless asked.
pub const WARNINGS_FLAG: &str = "--warnings=";

/// Whether a warning fails the run, as an error does.
pub const FATAL_WARNINGS_FLAG: &str = "--fatal-warnings";

/// What the three flags say, for a program's `--help`.
pub const USAGE: &str =
    "  --error-limit=N    stop after N errors (0 included); \"unlimited\", the
                     default, looks for every one
  --warnings=on|off|errors
                     print warnings (on, the default), print none (off),
                     or count each against --error-limit as an error is
                     (errors)
  --fatal-warnings   a warning fails the run, as an error does; refused
                     with --warnings=off, which would hide what failed it";

/// Takes one of the three flags into `limits`.
///
/// # Arguments
///
/// * `said`   - One argument of the command line.
/// * `limits` - What the run looks for, changed by the flag.
///
/// # Returns
///
/// `None` where `said` is none of the three; otherwise whether it was read.
///
/// # Errors
///
/// Within the `Some`: a sentence saying what the flag may be, where its
/// value is not one.
pub fn take(said: &str, limits: &mut Limits) -> Option<Result<(), String>> {
    if said == FATAL_WARNINGS_FLAG {
        limits.fatal_warnings = true;
        return Some(Ok(()));
    }
    if let Some(value) = said.strip_prefix(ERROR_LIMIT_FLAG) {
        return Some(
            error_limit(value).map(|limit| limits.error_limit = limit),
        );
    }
    if let Some(value) = said.strip_prefix(WARNINGS_FLAG) {
        return Some(
            warnings(value).map(|warnings| limits.warnings = warnings),
        );
    }
    None
}

/// Checks that the flags taken do not contradict one another.
///
/// # Arguments
///
/// * `limits` - What the flags said.
///
/// # Returns
///
/// Nothing, where they agree.
///
/// # Errors
///
/// A sentence naming the two that do not: a warning that fails the run and
/// is not printed would be a failure nothing explains.
pub fn agree(limits: &Limits) -> Result<(), String> {
    if limits.fatal_warnings && limits.warnings == Warnings::Off {
        return Err(format!(
            "{FATAL_WARNINGS_FLAG} fails the run on a warning, and \
             {WARNINGS_FLAG}off prints none: give one or the other"
        ));
    }
    Ok(())
}

/// Reads what `--error-limit=` was given, as how many to list.
///
/// # Arguments
///
/// * `value` - What follows the `=`.
///
/// # Returns
///
/// The limit, or `None` for "unlimited", which is every one.
///
/// # Errors
///
/// A sentence saying what the limit may be, if it is anything else.
fn error_limit(value: &str) -> Result<Option<usize>, String> {
    if value == "unlimited" {
        return Ok(None);
    }
    value.parse::<usize>().map(Some).map_err(|_| {
        format!(
            "{ERROR_LIMIT_FLAG}{value}: the limit is a whole number, \
             0 included, or \"unlimited\""
        )
    })
}

/// Reads what `--warnings=` was given.
///
/// # Arguments
///
/// * `value` - What follows the `=`: "on", "off" or "errors".
///
/// # Returns
///
/// What is done with warnings.
///
/// # Errors
///
/// A sentence saying what it may be, if it is anything else.
fn warnings(value: &str) -> Result<Warnings, String> {
    match value {
        "on" => Ok(Warnings::On),
        "off" => Ok(Warnings::Off),
        "errors" => Ok(Warnings::Errors),
        _ => Err(format!(
            "{WARNINGS_FLAG}{value}: warnings are \"on\", \"off\" or \"errors\""
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A whole number, nought included, or "unlimited"; anything else is a
    /// command line that is not one of the shapes, and is told so.
    #[test]
    fn an_error_limit_is_a_number_or_unlimited() {
        assert_eq!(error_limit("0"), Ok(Some(0)));
        assert_eq!(error_limit("5"), Ok(Some(5)));
        assert_eq!(error_limit("250"), Ok(Some(250)));
        assert_eq!(error_limit("unlimited"), Ok(None));
        for bad in ["", "-1", "five", "Unlimited", "5.0", "all"] {
            assert!(error_limit(bad).is_err(), "{bad:?} was taken as a limit");
        }
    }

    /// On, off or errors, spelled so; anything else is told so.
    #[test]
    fn warnings_are_on_off_or_errors() {
        assert_eq!(warnings("on"), Ok(Warnings::On));
        assert_eq!(warnings("off"), Ok(Warnings::Off));
        assert_eq!(warnings("errors"), Ok(Warnings::Errors));
        for bad in ["", "On", "none", "error", "yes"] {
            assert!(warnings(bad).is_err(), "{bad:?} was taken");
        }
    }

    /// A fatal warning that is never printed is refused.
    #[test]
    fn fatal_warnings_and_no_warnings_do_not_agree() {
        let mut limits = Limits::EVERYTHING;
        assert_eq!(take("--fatal-warnings", &mut limits), Some(Ok(())));
        assert_eq!(take("--warnings=off", &mut limits), Some(Ok(())));
        assert!(agree(&limits).is_err());
        assert_eq!(take("payload.ytcs", &mut limits), None);
    }
}

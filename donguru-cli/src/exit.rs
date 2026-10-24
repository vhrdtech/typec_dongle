/// The exit status of `donguru`: a public contract, documented in `docs/design/cli/index.md` ("Exit codes") and
/// `docs/user/scripting.md`. Never renumber one; a new category takes the next free number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    /// The canonical ExitCode for successful termination on this platform.
    Success,
    /// The canonical ExitCode for successful termination on this platform.
    Failure,
    /// Wrong or invalid argument or argument vector.
    BadArgs,
    /// No Device could be found.
    NoDevice,
    /// The Device reported/raised an error.
    DeviceError,
    /// Operation timed out.
    Timeout,
}

impl ExitCode {
    pub const BAD_ARGS: u8 = 2;
    pub const NO_DEVICE: u8 = 3;
    pub const DEVICE_ERROR: u8 = 4;
    pub const TIMEOUT: u8 = 5;

    /// The number the process exits with.
    pub const fn code(self) -> u8 {
        match self {
            ExitCode::Success => 0,
            ExitCode::Failure => 1,
            ExitCode::BadArgs => Self::BAD_ARGS,
            ExitCode::NoDevice => Self::NO_DEVICE,
            ExitCode::DeviceError => Self::DEVICE_ERROR,
            ExitCode::Timeout => Self::TIMEOUT,
        }
    }

    /// The stable name of the category, as `--format json` errors carry it (`"exit": "no_device"`).
    pub const fn name(self) -> &'static str {
        match self {
            ExitCode::Success => "success",
            ExitCode::Failure => "failure",
            ExitCode::BadArgs => "bad_args",
            ExitCode::NoDevice => "no_device",
            ExitCode::DeviceError => "device_error",
            ExitCode::Timeout => "timeout",
        }
    }
}

impl From<&ExitCode> for std::process::ExitCode {
    fn from(value: &ExitCode) -> Self {
        std::process::ExitCode::from(value.code())
    }
}

impl From<ExitCode> for std::process::ExitCode {
    fn from(value: ExitCode) -> Self {
        std::process::ExitCode::from(&value)
    }
}

impl From<&anyhow::Error> for ExitCode {
    fn from(value: &anyhow::Error) -> Self {
        match value.downcast_ref::<donguru_core::Error>() {
            Some(donguru_core::Error::NoDevice(_)) => ExitCode::NoDevice,
            // the device answered, but not with what was asked: its error, an item its firmware lacks, an API
            // this CLI can't talk to
            Some(
                donguru_core::Error::Device { .. }
                | donguru_core::Error::Unimplemented { .. }
                | donguru_core::Error::Incompatible(_),
            ) => ExitCode::DeviceError,
            Some(donguru_core::Error::Timeout(_)) => ExitCode::Timeout,
            _ => ExitCode::Failure,
        }
    }
}

/// The error is a write to a closed pipe, `dg ... | head`.
pub fn is_broken_pipe(err: &anyhow::Error) -> bool {
    err.chain()
        .filter_map(|e| e.downcast_ref::<std::io::Error>())
        .any(|e| e.kind() == std::io::ErrorKind::BrokenPipe)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The numbers and names are what scripts branch on: this test is the contract, change it only with a CHANGELOG
    /// entry.
    #[test]
    fn codes_and_names_are_stable() {
        let all = [
            (ExitCode::Success, 0, "success"),
            (ExitCode::Failure, 1, "failure"),
            (ExitCode::BadArgs, 2, "bad_args"),
            (ExitCode::NoDevice, 3, "no_device"),
            (ExitCode::DeviceError, 4, "device_error"),
            (ExitCode::Timeout, 5, "timeout"),
        ];
        for (code, n, name) in all {
            assert_eq!((code.code(), code.name()), (n, name));
        }
    }

    #[test]
    fn broken_pipe_is_found_under_context() {
        let io = std::io::Error::from(std::io::ErrorKind::BrokenPipe);
        let err = anyhow::Error::new(io).context("writing the table");
        assert!(is_broken_pipe(&err));
        assert!(!is_broken_pipe(&anyhow::anyhow!("other")));
    }
}

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
}

impl From<&ExitCode> for std::process::ExitCode {
    fn from(value: &ExitCode) -> Self {
        match value {
            ExitCode::Success => std::process::ExitCode::SUCCESS,
            ExitCode::Failure => std::process::ExitCode::FAILURE,
            ExitCode::BadArgs => std::process::ExitCode::from(ExitCode::BAD_ARGS),
            ExitCode::NoDevice => std::process::ExitCode::from(ExitCode::NO_DEVICE),
            ExitCode::DeviceError => std::process::ExitCode::from(ExitCode::DEVICE_ERROR),
            ExitCode::Timeout => std::process::ExitCode::from(ExitCode::TIMEOUT),
        }
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

    #[test]
    fn broken_pipe_is_found_under_context() {
        let io = std::io::Error::from(std::io::ErrorKind::BrokenPipe);
        let err = anyhow::Error::new(io).context("writing the table");
        assert!(is_broken_pipe(&err));
        assert!(!is_broken_pipe(&anyhow::anyhow!("other")));
    }
}

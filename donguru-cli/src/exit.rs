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
            Some(donguru_core::Error::Device(_)) => ExitCode::DeviceError,
            Some(donguru_core::Error::Timeout(_)) => ExitCode::Timeout,
            _ => ExitCode::Failure,
        }
    }
}

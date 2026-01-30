#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ExitCode(pub u8);

impl ExitCode {
    pub const SUCCESS: Self = ExitCode(0);
    pub const FAILURE: Self = ExitCode(1);

    pub fn as_i32(&self) -> i32 {
        self.0 as i32
    }
}

impl std::fmt::Display for ExitCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u8> for ExitCode {
    fn from(val: u8) -> Self {
        ExitCode(val)
    }
}

impl From<std::process::ExitStatus> for ExitCode {
    fn from(val: std::process::ExitStatus) -> Self {
        ExitCode(val.code().unwrap_or(1) as u8)
    }
}

impl From<ExitCode> for i32 {
    fn from(val: ExitCode) -> Self {
        val.as_i32()
    }
}

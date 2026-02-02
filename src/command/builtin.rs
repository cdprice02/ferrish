#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltInCommand {
    name: BuiltInName,
}

impl std::fmt::Display for BuiltInCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl BuiltInCommand {
    pub fn new(name: BuiltInName) -> Self {
        Self { name }
    }

    pub fn name(&self) -> BuiltInName {
        self.name
    }
}

#[derive(strum::EnumString, strum::AsRefStr, strum::Display, Debug, Clone, Copy, PartialEq, Eq)]
#[strum(serialize_all = "lowercase")]
pub enum BuiltInName {
    Exit,
    Echo,
    Type,
    Pwd,
    Cd,
}

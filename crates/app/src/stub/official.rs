//! Stand-ins for the official EPOS Gaming Suite checks (it only exists on Windows).

#[derive(Default)]
pub struct Status;

impl Status {
    pub fn summary(&self) -> String {
        String::new()
    }
}

pub fn check() -> Status {
    Status
}

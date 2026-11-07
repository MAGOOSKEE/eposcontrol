//! HID control of the EPOS GSX 300 (VID 0x1395, PID 0x0098).
//!
//! Protocol (from USB captures, see docs/RE_NOTES.md):
//! - Input report `02 01`: smart button pressed.
//! - Output report ID 4, 39 bytes: `04 40 01 00 01 XX 00..` with XX = 0xC8 or 0xE0.

use hidapi::{HidApi, HidDevice};

pub const VID: u16 = 0x1395;
pub const PID: u16 = 0x0098;
const USAGE_PAGE_CONSUMER: u16 = 0x000C;
const OUTPUT_REPORT_LEN: usize = 39;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    SmartButton,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Report value 0xC8.
    A,
    /// Report value 0xE0.
    B,
}

impl State {
    fn byte(self) -> u8 {
        match self {
            State::A => 0xC8,
            State::B => 0xE0,
        }
    }
}

pub struct Gsx300 {
    dev: HidDevice,
}

#[derive(Debug)]
pub enum Error {
    NotFound,
    Hid(hidapi::HidError),
}

impl From<hidapi::HidError> for Error {
    fn from(e: hidapi::HidError) -> Self {
        Error::Hid(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotFound => write!(f, "GSX 300 not found"),
            Error::Hid(e) => write!(f, "{e}"),
        }
    }
}

impl Gsx300 {
    /// Open the consumer control collection of the GSX 300 HID interface.
    pub fn open() -> Result<Self, Error> {
        let api = HidApi::new()?;
        let info = api
            .device_list()
            .find(|d| {
                d.vendor_id() == VID && d.product_id() == PID && d.usage_page() == USAGE_PAGE_CONSUMER
            })
            .ok_or(Error::NotFound)?;
        let dev = info.open_device(&api)?;
        Ok(Self { dev })
    }

    pub fn set_state(&self, state: State) -> Result<(), Error> {
        let mut r = [0u8; OUTPUT_REPORT_LEN];
        r[0] = 0x04;
        r[1] = 0x40;
        r[2] = 0x01;
        r[3] = 0x00;
        r[4] = 0x01;
        r[5] = state.byte();
        self.dev.write(&r)?;
        Ok(())
    }

    /// Block up to `timeout_ms` for an input report and return the raw bytes.
    pub fn read_raw(&self, timeout_ms: i32) -> Result<Vec<u8>, Error> {
        let mut buf = [0u8; 64];
        let n = self.dev.read_timeout(&mut buf, timeout_ms)?;
        Ok(buf[..n].to_vec())
    }

    pub fn parse(report: &[u8]) -> Option<Event> {
        match report {
            [0x02, 0x01, ..] => Some(Event::SmartButton),
            _ => None,
        }
    }
}

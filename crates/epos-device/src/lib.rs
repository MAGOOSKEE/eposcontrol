//! HID control of the EPOS GSX 300 (VID 0x1395, PID 0x0098).
//!
//! Protocol (from USB captures and manual tests, see docs/RE_NOTES.md):
//! - Input report `02 01` on the vendor collection: smart button pressed.
//! - Output report ID 4, 39 bytes, on the consumer collection:
//!   `04 40 01 00 01 XX 00..` where XX is a bitmask: 0x20 red, 0x08 blue, both purple.

use hidapi::{HidApi, HidDevice};

pub const VID: u16 = 0x1395;
pub const PID: u16 = 0x0098;
const USAGE_PAGE_CONSUMER: u16 = 0x000C;
const USAGE_PAGE_VENDOR: u16 = 0xFFFF;
const OUTPUT_REPORT_LEN: usize = 39;

const LED_RED: u8 = 0x20;
const LED_BLUE: u8 = 0x08;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    SmartButton,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Led {
    Off,
    #[default]
    Red,
    Blue,
    Purple,
}

impl Led {
    pub const ALL: [Led; 4] = [Led::Off, Led::Red, Led::Blue, Led::Purple];

    fn mask(self) -> u8 {
        match self {
            Led::Off => 0,
            Led::Red => LED_RED,
            Led::Blue => LED_BLUE,
            Led::Purple => LED_RED | LED_BLUE,
        }
    }

    pub fn from_index(i: i32) -> Led {
        Led::ALL.get(i as usize).copied().unwrap_or(Led::Off)
    }

    pub fn index(self) -> i32 {
        Led::ALL.iter().position(|l| *l == self).unwrap_or(0) as i32
    }
}

pub struct Gsx300 {
    /// Vendor collection (usage page 0xFFFF): button input reports.
    reader: HidDevice,
    /// Consumer collection (usage page 0x0C): LED output reports.
    writer: HidDevice,
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
    /// Open both HID collections of the GSX 300.
    pub fn open() -> Result<Self, Error> {
        let api = HidApi::new()?;
        let find = |page: u16| {
            api.device_list()
                .find(|d| d.vendor_id() == VID && d.product_id() == PID && d.usage_page() == page)
                .ok_or(Error::NotFound)
        };
        let reader = find(USAGE_PAGE_VENDOR)?.open_device(&api)?;
        let writer = find(USAGE_PAGE_CONSUMER)?.open_device(&api)?;
        Ok(Self { reader, writer })
    }

    pub fn set_led(&self, led: Led) -> Result<(), Error> {
        self.send_raw(&[0x04, 0x40, 0x01, 0x00, 0x01, led.mask()])
    }

    /// Send an arbitrary output report (report ID is the first byte) for protocol research.
    pub fn send_raw(&self, report: &[u8]) -> Result<(), Error> {
        let mut r = [0u8; OUTPUT_REPORT_LEN];
        let n = report.len().min(OUTPUT_REPORT_LEN);
        r[..n].copy_from_slice(&report[..n]);
        self.writer.write(&r)?;
        Ok(())
    }

    /// Block up to `timeout_ms` for an input report and return the raw bytes.
    pub fn read_raw(&self, timeout_ms: i32) -> Result<Vec<u8>, Error> {
        let mut buf = [0u8; 64];
        let n = self.reader.read_timeout(&mut buf, timeout_ms)?;
        Ok(buf[..n].to_vec())
    }

    pub fn parse(report: &[u8]) -> Option<Event> {
        match report {
            [0x02, 0x01, ..] => Some(Event::SmartButton),
            _ => None,
        }
    }
}

impl Gsx300 {
    /// USB serial number string of the device, if present.
    pub fn serial() -> Option<String> {
        let api = HidApi::new().ok()?;
        let serial = api
            .device_list()
            .find(|d| d.vendor_id() == VID && d.product_id() == PID)
            .and_then(|d| d.serial_number().map(|s| s.to_string()));
        serial
    }
}

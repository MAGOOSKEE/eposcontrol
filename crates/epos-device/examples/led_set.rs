//! Usage: led_set <hex bytes after report id 04>, e.g. `led_set 40 01 00 01 e0`
use epos_device::Gsx300;

fn main() {
    let mut report = vec![0x04u8];
    for a in std::env::args().skip(1) {
        report.push(u8::from_str_radix(&a, 16).expect("hex byte"));
    }
    let dev = Gsx300::open().expect("open GSX 300");
    dev.send_raw(&report).expect("write");
    println!("sent {:02x?}", report);
}

use epos_device::Gsx300;

fn main() {
    let dev = match Gsx300::open() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("open failed: {e}");
            return;
        }
    };
    println!("opened. Listening 15 s for reports (press the smart button).");
    let end = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while std::time::Instant::now() < end {
        if let Ok(r) = dev.read_raw(200) {
            if !r.is_empty() {
                println!("in: {:02x?} -> {:?}", r, Gsx300::parse(&r));
            }
        }
    }
}

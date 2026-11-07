use epos_device::{Gsx300, State};

fn main() {
    let dev = match Gsx300::open() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("open failed: {e}");
            return;
        }
    };
    println!("opened. Listening 10 s for reports (press the smart button).");
    let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < end {
        if let Ok(r) = dev.read_raw(200) {
            if !r.is_empty() {
                println!("in: {:02x?} -> {:?}", r, Gsx300::parse(&r));
            }
        }
    }
    for s in [State::A, State::B] {
        println!("write {:?}: {:?}", s, dev.set_state(s).map_err(|e| e.to_string()));
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

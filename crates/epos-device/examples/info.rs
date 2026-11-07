use hidapi::HidApi;

fn main() {
    let api = HidApi::new().unwrap();
    for d in api.device_list().filter(|d| d.vendor_id() == 0x1395) {
        println!(
            "pid={:04x} usage_page={:04x} usage={:04x} iface={} release=0x{:04x} product={:?} serial={:?}",
            d.product_id(),
            d.usage_page(),
            d.usage(),
            d.interface_number(),
            d.release_number(),
            d.product_string(),
            d.serial_number()
        );
    }
}

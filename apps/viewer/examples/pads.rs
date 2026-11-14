//! Lists the gamepads plugged in and what each is doing, for a few seconds: for finding out why
//! one is not read.
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use gilrs::{Axis, Button, Gilrs};
    let mut pads = Gilrs::new().expect("gamepads can be read");
    let seconds: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(4);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(seconds);
    let mut events = 0;
    while std::time::Instant::now() < until {
        while let Some(event) = pads.next_event() {
            events += 1;
            if events <= 40 {
                println!("event from {:?}: {:?}", event.id, event.event);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(8));
    }
    println!("{events} events");
    // What the HID layer itself lists, and what a PlayStation controller is sending.
    let api = hidapi::HidApi::new().expect("HID devices can be listed");
    for info in api.device_list() {
        if info.vendor_id() != 0x054C {
            continue;
        }
        println!(
            "hid: {:04x}:{:04x} {:?} usage {:x}/{:x} bus {:?}",
            info.vendor_id(),
            info.product_id(),
            info.product_string(),
            info.usage_page(),
            info.usage(),
            info.bus_type()
        );
        match info.open_device(&api) {
            Ok(device) => {
                let mut bytes = [0_u8; 128];
                for _ in 0..3 {
                    match device.read_timeout(&mut bytes, 500) {
                        Ok(count) => println!(
                            "  report of {count} bytes: {:02x?}",
                            &bytes[..count.min(12)]
                        ),
                        Err(error) => println!("  read failed: {error}"),
                    }
                }
            }
            Err(error) => println!("  can't be opened: {error}"),
        }
    }
    for (id, pad) in pads.gamepads() {
        println!(
            "pad {id:?}: {:?} (os name {:?}), mapping {:?}, uuid {:02x?}",
            pad.name(),
            pad.os_name(),
            pad.mapping_source(),
            pad.uuid()
        );
        for button in [
            Button::South,
            Button::East,
            Button::LeftTrigger2,
            Button::RightTrigger2,
        ] {
            println!(
                "  {button:?}: {:?}",
                pad.button_data(button).map(|d| d.value())
            );
        }
        for axis in [
            Axis::LeftStickX,
            Axis::LeftStickY,
            Axis::RightStickX,
            Axis::LeftZ,
            Axis::RightZ,
        ] {
            println!("  {axis:?}: {:?}", pad.axis_data(axis).map(|d| d.value()));
        }
    }
}
#[cfg(target_arch = "wasm32")]
fn main() {}

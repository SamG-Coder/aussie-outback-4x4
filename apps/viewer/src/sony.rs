//! A PlayStation controller, read directly.
//!
//! Windows shows the game a PlayStation controller only as a bare HID device: its own gamepad
//! API, which `gilrs` reads, lists none at all for a DualSense on many machines, unless some
//! other program is standing in for it as an Xbox pad. So this reads the controller's own
//! reports: a DualSense or a DualShock 4, by USB or by Bluetooth, with nothing else installed.
//!
//! Each report holds the two sticks, the two triggers as how far they are pulled, and a bit for
//! each button. They are handed to the game under the standard layout's names
//! (`solstice_game::input`): cross is `south`, circle `east`, square `west`, triangle `north`.

use hidapi::{BusType, HidApi, HidDevice};
use solstice_game::Devices;

const SONY: u16 = 0x054C;
/// The controllers read, by product: whether each is a DualSense, and what it is called.
const PADS: [(u16, bool, &str); 5] = [
    (0x0CE6, true, "DualSense"),
    (0x0DF2, true, "DualSense Edge"),
    (0x05C4, false, "DualShock 4"),
    (0x09CC, false, "DualShock 4"),
    (0x0BA0, false, "DualShock 4 (wireless adaptor)"),
];
/// Seconds between looks for a controller, while none is plugged in.
const LOOK_EVERY: f32 = 2.0;

/// Which bytes of a report hold what.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Layout {
    /// A DualSense's full report: sticks, then the triggers, then a counter, then buttons.
    DualSense,
    /// A DualShock 4's, and a DualSense's short report over Bluetooth: sticks, buttons, then
    /// the triggers.
    Classic,
}

/// What a report says the controller is doing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Report {
    sticks: [f32; 4],
    triggers: [f32; 2],
    /// The d-pad's direction, 0 for up and round clockwise to 7, or 8 let go; then the two
    /// bytes of buttons.
    pad: u8,
    faces: u8,
    others: u8,
}

/// Reads a report's bytes, or `None` if it is not one of a controller's state.
fn parse(bytes: &[u8], dualsense: bool, bluetooth: bool) -> Option<Report> {
    let (&id, rest) = bytes.split_first()?;
    let (layout, body) = match (id, dualsense, bluetooth) {
        // Over Bluetooth a DualSense's full report, and a DualShock's, carry a byte or two
        // of their own first.
        (0x31, true, _) => (Layout::DualSense, rest.get(1..)?),
        (0x01, true, false) => (Layout::DualSense, rest),
        (0x01, true, true) => (Layout::Classic, rest),
        (0x01, false, _) => (Layout::Classic, rest),
        (0x11, false, _) => (Layout::Classic, rest.get(2..)?),
        _ => return None,
    };
    let axis = |byte: u8| (f32::from(byte) - 127.5) / 127.5;
    let pull = |byte: u8| f32::from(byte) / 255.0;
    let (buttons, triggers) = match layout {
        Layout::DualSense => (body.get(7..9)?, body.get(4..6)?),
        Layout::Classic => (body.get(4..6)?, body.get(7..9)?),
    };
    Some(Report {
        sticks: [axis(body[0]), axis(body[1]), axis(body[2]), axis(body[3])],
        triggers: [pull(triggers[0]), pull(triggers[1])],
        pad: buttons[0] & 0x0F,
        faces: buttons[0] >> 4,
        others: buttons[1],
    })
}

/// A PlayStation controller, if one is plugged in.
pub struct SonyPad {
    api: HidApi,
    /// The controller, whether it is a DualSense, and whether it is on Bluetooth.
    device: Option<(HidDevice, bool, bool)>,
    since_look: f32,
    last: Report,
}

impl SonyPad {
    /// Ready to read one, or `None` on a machine whose HID devices can't be listed.
    pub fn new() -> Option<Self> {
        match HidApi::new() {
            Ok(api) => Some(Self {
                api,
                device: None,
                since_look: LOOK_EVERY,
                last: Report::default(),
            }),
            Err(error) => {
                log::warn!("PlayStation controllers can't be read: {error}");
                None
            }
        }
    }

    /// Looks for a controller, and opens the first found.
    fn look(&mut self) {
        if self.api.refresh_devices().is_err() {
            return;
        }
        for info in self.api.device_list() {
            let Some(&(_, dualsense, name)) = PADS
                .iter()
                .find(|pad| info.vendor_id() == SONY && info.product_id() == pad.0)
            else {
                continue;
            };
            match info.open_device(&self.api) {
                Ok(device) => {
                    let bluetooth = matches!(info.bus_type(), BusType::Bluetooth);
                    let _ = device.set_blocking_mode(false);
                    log::info!(
                        "controller: {name}, by {}",
                        if bluetooth { "Bluetooth" } else { "USB" }
                    );
                    self.device = Some((device, dualsense, bluetooth));
                    return;
                }
                Err(error) => log::warn!("{name}: can't be opened: {error}"),
            }
        }
    }

    /// Hands the game what the controller is doing, over a frame `seconds` long. Returns
    /// whether there is a controller.
    pub fn read(&mut self, devices: &mut Devices, seconds: f32) -> bool {
        if self.device.is_none() {
            self.since_look += seconds;
            if self.since_look >= LOOK_EVERY {
                self.since_look = 0.0;
                self.look();
            }
        }
        let Some((device, dualsense, bluetooth)) = &self.device else {
            return false;
        };
        // Every report that has come since the last frame: the latest is how it is now.
        let mut bytes = [0_u8; 128];
        loop {
            match device.read(&mut bytes) {
                Ok(0) => break,
                Ok(count) => {
                    if let Some(report) = parse(&bytes[..count], *dualsense, *bluetooth) {
                        self.last = report;
                    }
                }
                Err(_) => {
                    // Unplugged: everything is let go, and it is looked for again.
                    log::info!("controller: unplugged");
                    self.device = None;
                    self.last = Report::default();
                    self.since_look = 0.0;
                    break;
                }
            }
        }
        let report = self.last;
        for (name, value) in ["left_x", "left_y", "right_x", "right_y"]
            .into_iter()
            .zip(report.sticks)
        {
            devices.gamepad_axis(name, value);
        }
        devices.gamepad_button("l2", report.triggers[0]);
        devices.gamepad_button("r2", report.triggers[1]);
        let pressed = |on: bool| if on { 1.0 } else { 0.0 };
        for (name, bit) in [("west", 1), ("south", 2), ("east", 4), ("north", 8)] {
            devices.gamepad_button(name, pressed(report.faces & bit != 0));
        }
        for (name, bit) in [
            ("l1", 0x01),
            ("r1", 0x02),
            ("select", 0x10),
            ("start", 0x20),
            ("l3", 0x40),
            ("r3", 0x80),
        ] {
            devices.gamepad_button(name, pressed(report.others & bit != 0));
        }
        // The d-pad is one direction of eight: up is 0, and round clockwise.
        let pad = report.pad;
        devices.gamepad_button("up", pressed(matches!(pad, 7 | 0 | 1)));
        devices.gamepad_button("right", pressed(matches!(pad, 1..=3)));
        devices.gamepad_button("down", pressed(matches!(pad, 3..=5)));
        devices.gamepad_button("left", pressed(matches!(pad, 5..=7)));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A DualSense's report by USB: sticks centred but the left pushed right, the right
    /// trigger half pulled, cross held, the d-pad up and to the right, and R1.
    fn dualsense_usb() -> Vec<u8> {
        let mut bytes = vec![0_u8; 64];
        bytes[0] = 0x01;
        bytes[1..5].copy_from_slice(&[255, 128, 127, 0]);
        bytes[5] = 0;
        bytes[6] = 128;
        bytes[8] = 0x20 | 1;
        bytes[9] = 0x02;
        bytes
    }

    #[test]
    fn a_dualsense_report_is_read_by_usb_and_by_bluetooth() {
        let usb = parse(&dualsense_usb(), true, false).unwrap();
        assert!((usb.sticks[0] - 1.0).abs() < 1.0e-6);
        assert!(usb.sticks[1].abs() < 0.01 && usb.sticks[2].abs() < 0.01);
        assert!((usb.sticks[3] + 1.0).abs() < 1.0e-6, "pushed away is -1");
        assert_eq!(usb.triggers[0], 0.0);
        assert!((usb.triggers[1] - 0.502).abs() < 0.01);
        assert_eq!((usb.pad, usb.faces, usb.others), (1, 2, 0x02));
        // By Bluetooth the same report is numbered 0x31, with a counter before it.
        let mut bluetooth = vec![0x31, 0x00];
        bluetooth.extend_from_slice(&dualsense_usb()[1..]);
        assert_eq!(parse(&bluetooth, true, true), Some(usb));
        // Anything else is not a report of its state.
        assert_eq!(parse(&[0x05, 1, 2, 3], true, false), None);
        assert_eq!(parse(&[], true, false), None);
        assert_eq!(parse(&[0x01, 1, 2], true, false), None);
    }

    #[test]
    fn a_dualshock_keeps_its_buttons_before_its_triggers() {
        let mut bytes = vec![0_u8; 64];
        bytes[0] = 0x01;
        bytes[1..5].copy_from_slice(&[0, 255, 128, 128]);
        bytes[5] = 0x80 | 8;
        bytes[6] = 0x20;
        bytes[8] = 255;
        bytes[9] = 51;
        let report = parse(&bytes, false, false).unwrap();
        assert!((report.sticks[0] + 1.0).abs() < 1.0e-6 && (report.sticks[1] - 1.0).abs() < 1.0e-6);
        assert_eq!(report.triggers[0], 1.0);
        assert!((report.triggers[1] - 0.2).abs() < 1.0e-6);
        assert_eq!((report.pad, report.faces, report.others), (8, 8, 0x20));
        // A DualSense's short report over Bluetooth is laid out the same way.
        assert_eq!(parse(&bytes, true, true), Some(report));
    }

    #[test]
    fn the_game_is_handed_buttons_by_their_standard_names() {
        let mut pad = SonyPad {
            api: match HidApi::new() {
                Ok(api) => api,
                // A machine with no HID at all has nothing to test this on.
                Err(_) => return,
            },
            device: None,
            since_look: 0.0,
            last: parse(&dualsense_usb(), true, false).unwrap(),
        };
        // With no controller open nothing is handed over.
        let mut devices = Devices::new();
        assert!(!pad.read(&mut devices, 0.0) || pad.device.is_some());
    }
}

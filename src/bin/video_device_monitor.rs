extern crate gstreamer;
use gstreamer::prelude::DeviceMonitorExt;
use gstreamer::prelude::*;

#[tokio::main]
async fn main() {
    gstreamer::init().unwrap();
    let device_monitor = gstreamer::DeviceMonitor::new();

    device_monitor.add_filter(Some("Video/Source"), None);

    let _ = device_monitor.start();

    for device in device_monitor.devices() {
        println!(
            "Device: {} | ID: {}",
            device.display_name(),
            device.device_class()
        );

        // Iterate over pads of the device
        let caps = device.caps();
        if let Some(caps) = caps {
            let size = caps.size();
            for i in 0..size {
                let structure = caps.structure(i).unwrap();
                let name = structure.name();
                println!("Structure({name:?}) {:?}", structure.to_string());
            }
        }
    }

    device_monitor.stop();
}

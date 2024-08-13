use livekit_gstreamer::get_devices_info;

fn main() {
    gstreamer::init().unwrap();
    let devices = get_devices_info();
    for device_info in devices {
        println!(
            "============== Media Device Info ({:?}|{:?}) ==============",
            device_info.display_name, device_info.class
        );
        println!("Device path: {}", device_info.device_path);
        println!("Device name: {}", device_info.display_name);
        println!("Device class: {}", device_info.class);
        println!("Capabilities:");
        for capability in device_info.capabilities {
            println!("  {:?}", capability);
        }
        println!("============== End Media Device Info ==============");
    }
}

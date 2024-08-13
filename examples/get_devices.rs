use livekit_gstreamer::get_devices_info;

fn main() {
    gstreamer::init().unwrap();
    let devices = get_devices_info();
    for (device_path, capabilities) in devices {
        println!("{:?} -> {:?}", device_path, capabilities);
    }
}

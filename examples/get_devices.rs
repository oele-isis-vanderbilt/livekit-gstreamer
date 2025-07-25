use livekit_gstreamer::{get_devices_info, MediaDeviceInfo};

fn main() {
    gstreamer::init().unwrap();
    let devices = get_devices_info();

    let (video_devices, audio_devices, screen_devices): (
        Vec<MediaDeviceInfo>,
        Vec<MediaDeviceInfo>,
        Vec<MediaDeviceInfo>,
    ) = devices.into_iter().fold(
        (Vec::new(), Vec::new(), Vec::new()),
        |(mut video, mut audio, mut screen), device| {
            match device.device_class.as_str() {
                "Video/Source" => video.push(device),
                "Audio/Source" => audio.push(device),
                "Screen/Source" => screen.push(device),
                _ => (),
            }
            (video, audio, screen)
        },
    );

    println!("Video Devices:");
    for device_info in video_devices {
        println!(
            "============== Media Device Info ({:?}|{:?}) ==============",
            device_info.display_name, device_info.device_class
        );
        println!("Device path: {}", device_info.device_path);
        println!("Device name: {}", device_info.display_name);
        println!("Device class: {}", device_info.device_class);
        println!("Capabilities:");
        for capability in device_info.capabilities {
            println!("  {:?}", capability);
        }
        println!("============== End Media Device Info ==============");
    }
    println!("\n------------------------------------------------------\n");
    println!("Audio Devices:");
    for device_info in audio_devices {
        println!(
            "============== Media Device Info ({:?}|{:?}) ==============",
            device_info.display_name, device_info.device_class
        );
        println!("Device path: {}", device_info.device_path);
        println!("Device name: {}", device_info.display_name);
        println!("Device class: {}", device_info.device_class);
        println!("Capabilities:");
        for capability in device_info.capabilities {
            println!("  {:?}", capability);
        }
        println!("============== End Media Device Info ==============");
    }

    println!("\n------------------------------------------------------\n");
    println!("Screen Devices:");
    for device_info in screen_devices {
        println!(
            "============== Media Device Info ({:?}|{:?}) ==============",
            device_info.display_name, device_info.device_class
        );
        println!("Device path: {}", device_info.device_path);
        println!("Device name: {}", device_info.display_name);
        println!("Device class: {}", device_info.device_class);
        println!("Capabilities:");
        for capability in device_info.capabilities {
            println!("  {:?}", capability);
        }
        println!("============== End Media Device Info ==============");
    }
}

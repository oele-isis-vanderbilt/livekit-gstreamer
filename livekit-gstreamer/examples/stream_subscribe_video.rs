use dotenvy::dotenv;
use livekit_gstreamer::{
    GStreamerError, GstMediaStream, LocalFileSaveOptions, PublishOptions, VideoPublishOptions,
};

#[path = "./helper/wait.rs"]
mod wait;

#[tokio::main]
async fn main() -> Result<(), GStreamerError> {
    dotenv().ok();
    // Initialize gstreamer
    gstreamer::init().unwrap();
    if !(cfg!(any(target_os = "linux", target_os = "windows"))) {
        panic!("This example is only supported on Linux and Windows");
    }
    let mut stream = if cfg!(target_os = "linux") {
        // Note: Make sure to replace the device_id with the correct device and the codecs and resolutions are supported by the device
        // This can be checked by running `v4l2-ctl --list-formats-ext -d /dev/video0` for example or using gst-device-monitor-1.0 Video/Source
        GstMediaStream::new(PublishOptions::Video(VideoPublishOptions {
            codec: "image/jpeg".to_string(),
            width: 1920,
            height: 1080,
            framerate: 30,
            device_id: "/dev/video0".to_string(),
            local_file_save_options: Some(LocalFileSaveOptions {
                output_dir: "recordings".to_string(),
            }),
        }))
    } else {
        GstMediaStream::new(PublishOptions::Video(VideoPublishOptions {
            codec: "image/jpeg".to_string(),
            width: 1280,
            height: 720,
            framerate: 30,
            device_id: r"\\?\usb#vid_0c45&pid_6a10&mi_00#6&303dd63&0&0000#{e5323777-f976-4f5b-9b55-b94699c46e44}\global".to_string(),
            local_file_save_options: Some(LocalFileSaveOptions {
                output_dir: "recordings".to_string(),
            }),
        }))
    };

    stream.start().await.unwrap();

    let (frame_rx, close_rx) = stream.subscribe().unwrap();

    wait::wait_streams(&mut [stream], vec![frame_rx], vec![close_rx]).await
}

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

    // Note: Make sure to replace the device_id with the correct device and the codecs and resolutions are supported by the device
    // This can be checked by running `v4l2-ctl --list-formats-ext -d /dev/video0` for example or using gst-device-monitor-1.0 Video/Source
    let mut stream = GstMediaStream::new(PublishOptions::Video(VideoPublishOptions {
        codec: "image/jpeg".to_string(),
        width: 1920,
        height: 1080,
        framerate: 30,
        device_id: "/dev/video0".to_string(),
        local_file_save_options: Some(LocalFileSaveOptions {
            output_dir: "recordings".to_string(),
        }),
    }));

    stream.start().await.unwrap();

    let (frame_rx, close_rx) = stream.subscribe().unwrap();

    wait::wait_stream(&mut stream, frame_rx, close_rx).await
}

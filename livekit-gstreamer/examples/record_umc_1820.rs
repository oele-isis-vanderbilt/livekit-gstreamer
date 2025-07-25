use livekit_gstreamer::{
    AudioPublishOptions, GStreamerError, GstMediaStream, LocalFileSaveOptions, PublishOptions,
};

#[path = "./helper/wait.rs"]
mod wait;

#[tokio::main]
async fn main() -> Result<(), GStreamerError> {
    // Only run on Linux
    if !cfg!(target_os = "linux") {
        panic!("This example is only supported on Linux");
    }

    gstreamer::init().map_err(|e| {
        GStreamerError::PipelineError(format!("Failed to initialize gstreamer: {}", e))
    })?;

    let publish_options = AudioPublishOptions {
        codec: "audio/x-raw".to_string(),
        device_id: "hw:4".to_string(),
        framerate: 96000,
        channels: 10,
        selected_channel: Some(2),
        local_file_save_options: Some(LocalFileSaveOptions {
            output_dir: "recordings".to_string(),
        }),
    };

    let mut stream = GstMediaStream::new(PublishOptions::Audio(publish_options));

    stream.start().await?;

    let (frame_rx, close_rx) = stream.subscribe().unwrap();

    wait::wait_streams(&mut [stream], vec![frame_rx], vec![close_rx]).await
}

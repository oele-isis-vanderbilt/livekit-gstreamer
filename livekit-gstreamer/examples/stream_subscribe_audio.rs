use livekit_gstreamer::{
    AudioPublishOptions, GStreamerError, GstMediaStream, LocalFileSaveOptions, PublishOptions,
};

#[path = "./helper/wait.rs"]
mod wait;

#[tokio::main]
async fn main() -> Result<(), GStreamerError> {
    gstreamer::init().map_err(|e| {
        GStreamerError::PipelineError(format!("Failed to initialize gstreamer: {}", e))
    })?;

    if !(cfg!(any(target_os = "linux", target_os = "windows"))) {
        panic!("This example is only supported on Linux and Windows");
    }
    let mut stream = if cfg!(target_os = "linux") {
        GstMediaStream::new(PublishOptions::Audio(AudioPublishOptions {
            codec: "audio/x-raw".to_string(),
            device_id: "front:1".to_string(),
            framerate: 48000,
            channels: 1,
            selected_channel: None,
            local_file_save_options: Some(LocalFileSaveOptions {
                output_dir: "recordings".to_string(),
            }),
        }))
    } else {
        GstMediaStream::new(PublishOptions::Audio(AudioPublishOptions {
            codec: "audio/x-raw".to_string(),
            framerate: 48000,
            device_id: r"\\?\SWD#MMDEVAPI#{0.0.1.00000000}.{400ac096-5f57-4207-87c5-b9d208f12749}#{2eef81be-33fa-4800-9670-1cd474972c3f}".to_string(),
            local_file_save_options: Some(LocalFileSaveOptions {
                output_dir: "recordings".to_string(),
            }),
            channels: 2,
            selected_channel: None
        }))
    };

    stream.start().await?;

    let (frame_rx, close_rx) = stream.subscribe().unwrap();

    wait::wait_streams(&mut [stream], vec![frame_rx], vec![close_rx]).await
}

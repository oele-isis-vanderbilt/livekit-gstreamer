use livekit_gstreamer::{
    AudioPublishOptions, GStreamerError, GstMediaStream, LocalFileSaveOptions, PublishOptions,
    VideoPublishOptions,
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
    let mut audio_stream = if cfg!(target_os = "linux") {
        GstMediaStream::new(PublishOptions::Audio(AudioPublishOptions {
            codec: "audio/x-raw".to_string(),
            device_id: "hw:3".to_string(),
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

    let mut video_stream = if cfg!(target_os = "linux") {
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

    video_stream.start().await.unwrap();

    audio_stream.start().await?;

    let (audio_frame_rx, audio_close_rx) = audio_stream.subscribe().unwrap();
    let (video_frame_rx, video_close_rx) = video_stream.subscribe().unwrap();

    wait::wait_streams(
        &mut [audio_stream, video_stream],
        vec![audio_frame_rx, video_frame_rx],
        vec![audio_close_rx, video_close_rx],
    )
    .await
}

use livekit_gstreamer::{AudioPublishOptions, GStreamerError, GstMediaStream, PublishOptions};

#[tokio::main]
async fn main() -> Result<(), GStreamerError> {
    gstreamer::init().map_err(|e| {
        GStreamerError::PipelineError(format!("Failed to initialize gstreamer: {}", e))
    })?;

    let publish_options = AudioPublishOptions {
        codec: "audio/x-raw".to_string(),
        device_id: "hw:2".to_string(),
        framerate: 32000,
        channels: 1,
    };

    let mut stream = GstMediaStream::new(PublishOptions::Audio(publish_options));

    stream.start().await?;

    let (mut frame_rx, mut close_rx) = stream.subscribe().unwrap();

    loop {
        tokio::select! {
            _ = close_rx.recv() => {
                break;
            }
            frame = frame_rx.recv() => {
                if let Ok(frame) = frame {
                    // Do something with the frame
                    println!("Received frame at {:?} microseconds", frame.pts().unwrap_or_default().useconds());
                }
            }
        }
    }

    Ok(())
}

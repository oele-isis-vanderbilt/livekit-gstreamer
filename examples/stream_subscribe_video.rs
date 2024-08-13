use dotenvy::dotenv;
use livekit_gstreamer::{GStreamerError, GstMediaStream, PublishOptions, VideoPublishOptions};

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
    }));

    stream.start().await.unwrap();

    if let Some((mut frame_rx, mut close_rx)) = stream.subscribe() {
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
    }

    Ok(())
}

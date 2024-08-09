use dotenvy::dotenv;
use rust_livekit_streamer::{GStreamerError, GstVideoStream, VideoPublishOptions};

#[tokio::main]
async fn main() -> Result<(), GStreamerError> {
    dotenv().ok();
    // Initialize gstreamer
    gstreamer::init().unwrap();

    let mut stream = GstVideoStream::new(VideoPublishOptions {
        codec: "image/jpeg".to_string(),
        width: 1920,
        height: 1080,
        framerate: 30,
        device_id: "/dev/video0".to_string(),
    });

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

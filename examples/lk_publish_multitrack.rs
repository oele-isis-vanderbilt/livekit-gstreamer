use dotenvy::dotenv;
use livekit::{Room, RoomEvent, RoomOptions};

use livekit_api::access_token;
use livekit_gstreamer::{GstVideoStream, LKParticipant, LKParticipantError, VideoPublishOptions};
use std::{env, sync::Arc};

#[tokio::main]
async fn main() -> Result<(), LKParticipantError> {
    dotenv().ok();
    // Initialize gstreamer
    gstreamer::init().unwrap();
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    let url = env::var("LIVEKIT_URL").expect("LIVEKIT_URL is not set");
    let api_key = env::var("LIVEKIT_API_KEY").expect("LIVEKIT_API_KEY is not set");
    let api_secret = env::var("LIVEKIT_API_SECRET").expect("LIVEKIT_API_SECRET is not set");

    let token = access_token::AccessToken::with_api_key(&api_key, &api_secret)
        .with_identity("rust-bot-multivideo")
        .with_name("Rust Bot MultiVideo")
        .with_grants(access_token::VideoGrants {
            room_join: true,
            room: "DemoRoom".to_string(),
            ..Default::default()
        })
        .to_jwt()
        .unwrap();

    let (room, mut room_rx) = Room::connect(&url, &token, RoomOptions::default())
        .await
        .unwrap();

    let new_room = Arc::new(room);
    log::info!(
        "Connected to room: {} - {}",
        new_room.name(),
        String::from(new_room.sid().await)
    );

    // Note: Make sure to replace the device_id with the correct device and the codecs and resolutions are supported by the device
    // This can be checked by running `v4l2-ctl --list-formats-ext -d /dev/video0` for example or using gst-device-monitor-1.0 Video/Source
    let mut stream1 = GstVideoStream::new(VideoPublishOptions {
        codec: "image/jpeg".to_string(),
        width: 1920,
        height: 1080,
        framerate: 30,
        device_id: "/dev/video0".to_string(),
    });

    let mut stream2 = GstVideoStream::new(VideoPublishOptions {
        codec: "video/x-h264".to_string(),
        width: 1280,
        height: 720,
        framerate: 30,
        device_id: "/dev/video4".to_string(),
    });

    stream1.start().await.unwrap();

    stream2.start().await.unwrap();

    let mut participant = LKParticipant::new(new_room.clone());
    participant.publish_video_stream(&mut stream1, None).await?;
    log::info!(
        "Published stream 1 from device: {}",
        stream1.get_device_name().unwrap()
    );

    participant.publish_video_stream(&mut stream2, None).await?;
    log::info!(
        "Published stream 2 from device: {}",
        stream2.get_device_name().unwrap()
    );

    while let Some(msg) = room_rx.recv().await {
        match msg {
            RoomEvent::Disconnected { reason } => {
                log::info!("Disconnected from room: {:?}", reason);
                stream1.stop().await?;
                stream2.stop().await?;
                break;
            }
            _ => {
                log::info!("Received room event: {:?}", msg);
            }
        }
    }

    Ok(())
}
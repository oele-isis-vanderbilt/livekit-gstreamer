use dotenvy::dotenv;
use livekit::{Room, RoomEvent, RoomOptions};

use livekit_api::access_token;
use livekit_gstreamer::{
    AudioPublishOptions, GstMediaStream, LKParticipant, LKParticipantError, PublishOptions,
    VideoPublishOptions,
};
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
        .with_identity("rust-bot-multitrack")
        .with_name("Rust Bot Multitrack")
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
    let mut stream1 = GstMediaStream::new(PublishOptions::Video(VideoPublishOptions {
        codec: "image/jpeg".to_string(),
        width: 1920,
        height: 1080,
        framerate: 30,
        device_id: "/dev/video0".to_string(),
    }));

    let mut stream2 = GstMediaStream::new(PublishOptions::Video(VideoPublishOptions {
        codec: "video/x-h264".to_string(),
        width: 1280,
        height: 720,
        framerate: 30,
        device_id: "/dev/video4".to_string(),
    }));

    let mut stream3 = GstMediaStream::new(PublishOptions::Audio(AudioPublishOptions {
        codec: "audio/x-raw".to_string(),
        device_id: "front:3".to_string(),
        framerate: 32000,
        channels: 2,
    }));

    let mut stream4 = GstMediaStream::new(PublishOptions::Audio(AudioPublishOptions {
        codec: "audio/x-raw".to_string(),
        device_id: "hw:2".to_string(),
        framerate: 48000,
        channels: 1,
    }));

    stream1.start().await.unwrap();
    stream2.start().await.unwrap();
    stream3.start().await.unwrap();
    stream4.start().await.unwrap();

    let mut participant = LKParticipant::new(new_room.clone());
    participant.publish_stream(&mut stream1, None).await?;
    log::info!(
        "Published {} stream from device: {}",
        stream1.kind(),
        stream1.get_device_name().unwrap()
    );

    participant.publish_stream(&mut stream2, None).await?;
    log::info!(
        "Published {} stream from device: {}",
        stream2.kind(),
        stream2.get_device_name().unwrap()
    );
    participant.publish_stream(&mut stream3, None).await?;
    log::info!(
        "Published {} stream from device: {}",
        stream3.kind(),
        stream3.get_device_name().unwrap()
    );

    participant.publish_stream(&mut stream4, None).await?;
    log::info!(
        "Published {} stream from device: {}",
        stream4.kind(),
        stream4.get_device_name().unwrap()
    );

    while let Some(msg) = room_rx.recv().await {
        match msg {
            RoomEvent::Disconnected { reason } => {
                log::info!("Disconnected from room: {:?}", reason);
                stream1.stop().await?;
                stream2.stop().await?;
                stream3.stop().await?;
                stream4.stop().await?;
                break;
            }
            _ => {
                log::info!("Received room event: {:?}", msg);
            }
        }
    }

    Ok(())
}

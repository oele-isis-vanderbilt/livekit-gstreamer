use dotenvy::dotenv;
use livekit::{Room, RoomOptions};

use livekit_api::access_token;
use rust_livekit_streamer::{
    GstVideoStream, LKParticipant, LKParticipantError, VideoPublishOptions,
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
        .with_identity("rust-bot-h264")
        .with_name("Rust Bot h264")
        .with_grants(access_token::VideoGrants {
            room_join: true,
            room: "DemoRoom".to_string(),
            ..Default::default()
        })
        .to_jwt()
        .unwrap();

    let (room, _) = Room::connect(&url, &token, RoomOptions::default())
        .await
        .unwrap();

    let new_room = Arc::new(room);

    // Note: Make sure to replace the device_id with the correct device and the codecs and resolutions are supported by the device
    // This can be checked by running `v4l2-ctl --list-formats-ext -d /dev/video0` for example or using gst-device-monitor-1.0 Video/Source
    let mut stream = GstVideoStream::new(VideoPublishOptions {
        codec: "video/x-h264".to_string(),
        width: 1920,
        height: 1080,
        framerate: 30,
        device_id: "/dev/video4".to_string(),
    });

    stream.start().await.unwrap();

    let mut participant = LKParticipant::new(new_room.clone());

    let track_sid = participant.publish_video_stream(&mut stream, None).await?;

    log::info!(
        "Connected to room: {} - {}",
        new_room.name(),
        String::from(new_room.sid().await)
    );
    log::info!("Published track with SID for one minute: {}", track_sid);
    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    log::info!("Unpublishing track with SID: {}", track_sid);
    participant.unpublish_track(&track_sid).await?;

    Ok(())
}

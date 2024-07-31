use dotenvy::dotenv;
use livekit::prelude::*;
use livekit_api::access_token;
use std::{env, sync::Arc};

// Connect to a room using the specified env variables
// and print all incoming events

mod gst_camera_track;
mod logo_track;
mod video_track;

use gst_camera_track::{GSTCameraTrack, VideoPreset};

#[tokio::main]
async fn main() {
    dotenv().ok();
    // Initialize gstreamer
    gstreamer::init().unwrap();
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    let url = env::var("LIVEKIT_URL").expect("LIVEKIT_URL is not set");
    let api_key = env::var("LIVEKIT_API_KEY").expect("LIVEKIT_API_KEY is not set");
    let api_secret = env::var("LIVEKIT_API_SECRET").expect("LIVEKIT_API_SECRET is not set");

    let token = access_token::AccessToken::with_api_key(&api_key, &api_secret)
        .with_identity("rust-bot")
        .with_name("Rust Bot")
        .with_grants(access_token::VideoGrants {
            room_join: true,
            room: "DemoRoom".to_string(),
            ..Default::default()
        })
        .to_jwt()
        .unwrap();

    let (room, mut rx) = Room::connect(&url, &token, RoomOptions::default())
        .await
        .unwrap();
    let new_room = Arc::new(room);
    log::info!(
        "Connected to room: {} - {}",
        new_room.clone().name(),
        String::from(new_room.clone().sid().await)
    );

    let mut gstreamer_track = GSTCameraTrack::new(
        "/dev/video0",
        "I420",
        VideoPreset::H1080p,
        Some(new_room.clone()),
    );

    gstreamer_track.publish().await.unwrap();

    new_room
        .clone()
        .local_participant()
        .publish_data(DataPacket {
            payload: "Hello world I am about to publish some track to this room"
                .to_owned()
                .into_bytes(),
            reliable: true,
            ..Default::default()
        })
        .await
        .unwrap();

    while let Some(msg) = rx.recv().await {
        log::info!("Event: {:?}", msg);
    }
}

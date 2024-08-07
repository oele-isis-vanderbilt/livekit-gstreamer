mod livekit_track;
mod video_device;
use dotenvy::dotenv;

use livekit_api::access_token;
use livekit_track::{LivekitGSTTrackError, LivekitGSTVideoTrack, VideoPublishOptions};
use std::{env, sync::Arc};

#[tokio::main]
async fn main() -> Result<(), LivekitGSTTrackError> {
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

    let (room, mut room_rx) = Room::connect(&url, &token, RoomOptions::default())
        .await
        .unwrap();

    let new_room = Arc::new(room);

    let mut track = LivekitGSTVideoTrack::new(
        new_room.clone(),
        VideoPublishOptions {
            codec: "image/jpeg".to_string(),
            width: 1920,
            height: 1080,
            framerate: 30,
            device_id: "/dev/video0".to_string(),
        },
    );

    track.publish().await.unwrap();

    log::info!(
        "Connected to room: {} - {}",
        new_room.name(),
        String::from(new_room.sid().await)
    );

    while let Some(msg) = room_rx.recv().await {
        match msg {
            RoomEvent::Disconnected { reason } => {
                log::info!("Disconnected from room: {:?}", reason);
                track.unpublish().await.unwrap();
                break;
            }
            _ => {
                log::info!("Received room event: {:?}", msg);
            }
        }
    }

    Ok(())
}

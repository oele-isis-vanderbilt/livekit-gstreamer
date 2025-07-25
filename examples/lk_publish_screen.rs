use dotenvy::dotenv;
use livekit::{Room, RoomOptions};

use livekit_api::access_token;
use livekit_gstreamer::{
    GstMediaStream, LKParticipant, LKParticipantError, LocalFileSaveOptions, PublishOptions,
    ScreenPublishOptions,
};
use std::{env, sync::Arc};

#[path = "./helper/wait.rs"]
mod wait;

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
        .with_identity("rust-bot-screen-sharer")
        .with_name("Rust Bot Screen Sharer")
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
    let mut stream = if cfg!(target_os = "linux") {
        GstMediaStream::new(PublishOptions::Screen(ScreenPublishOptions {
            codec: "video/x-raw".to_string(),
            width: 1920,
            height: 1080,
            framerate: 30,
            screen_id_or_name: "DP-3-2".to_string(),
            local_file_save_options: Some(LocalFileSaveOptions {
                output_dir: "recordings".to_string(),
            }),
        }))
    } else {
        GstMediaStream::new(PublishOptions::Screen(ScreenPublishOptions {
            codec: "video/x-raw".to_string(),
            width: 1920,
            height: 1080,
            framerate: 30,
            screen_id_or_name: "65537".to_string(),
            local_file_save_options: Some(LocalFileSaveOptions {
                output_dir: "recordings".to_string(),
            }),
        }))
    };

    stream.start().await.unwrap();

    let mut participant = LKParticipant::new(new_room.clone());

    participant.publish_stream(&mut stream, None).await?;

    log::info!(
        "Connected to room: {} - {}",
        new_room.name(),
        String::from(new_room.sid().await)
    );

    wait::wait_lk(&mut [stream], new_room.clone(), &mut room_rx).await
}

use dotenvy::dotenv;
use livekit::{Room, RoomOptions};
use livekit_gstreamer::{
    AudioPublishOptions, GstMediaStream, LKParticipant, LKParticipantError, LocalFileSaveOptions,
    PublishOptions,
};

use livekit_api::access_token;
use std::{env, sync::Arc};

#[path = "./helper/wait.rs"]
mod wait;

#[tokio::main]
async fn main() -> Result<(), LKParticipantError> {
    // Only run on Linux
    if !cfg!(target_os = "linux") {
        panic!("This example is only supported on Linux");
    }
    dotenv().ok();

    // Initialize gstreamer
    gstreamer::init().unwrap();
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    let url = env::var("LIVEKIT_URL").expect("LIVEKIT_URL is not set");
    let api_key = env::var("LIVEKIT_API_KEY").expect("LIVEKIT_API_KEY is not set");
    let api_secret = env::var("LIVEKIT_API_SECRET").expect("LIVEKIT_API_SECRET is not set");

    let token = access_token::AccessToken::with_api_key(&api_key, &api_secret)
        .with_identity("rust-bot-microphone")
        .with_name("Rust Bot Microphone")
        .with_grants(access_token::VideoGrants {
            room_join: true,
            room: "demo-room".to_string(),
            ..Default::default()
        })
        .to_jwt()
        .unwrap();

    let (room, mut room_rx) = Room::connect(&url, &token, RoomOptions::default())
        .await
        .unwrap();

    let new_room = Arc::new(room);

    let publish_options1 = AudioPublishOptions {
        codec: "audio/x-raw".to_string(),
        device_id: "hw:4".to_string(),
        framerate: 96000,
        channels: 10,
        selected_channel: Some(1),
        local_file_save_options: Some(LocalFileSaveOptions {
            output_dir: "recordings".to_string(),
        }),
    };

    let publish_options2 = AudioPublishOptions {
        codec: "audio/x-raw".to_string(),
        device_id: "hw:4".to_string(),
        framerate: 96000,
        channels: 10,
        selected_channel: Some(2),
        local_file_save_options: Some(LocalFileSaveOptions {
            output_dir: "recordings".to_string(),
        }),
    };

    let mut stream1 = GstMediaStream::new(PublishOptions::Audio(publish_options1));

    let mut stream2 = GstMediaStream::new(PublishOptions::Audio(publish_options2));

    stream1.start().await?;
    stream2.start().await?;

    let mut participant = LKParticipant::new(new_room.clone());
    participant
        .publish_stream(&mut stream1, Some("UMC1820-Channel1".into()))
        .await?;
    participant
        .publish_stream(&mut stream2, Some("UMC1820-Channel2".into()))
        .await?;

    log::info!(
        "Connected to room: {} - {}",
        new_room.name(),
        String::from(new_room.sid().await)
    );

    wait::wait_lk(&mut [stream1, stream2], new_room.clone(), &mut room_rx).await
}

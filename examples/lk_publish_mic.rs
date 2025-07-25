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
    // Only run on windows and linux
    if !cfg!(any(target_os = "linux", target_os = "windows")) {
        panic!("This example is only supported on Linux and Windows");
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

    let mut stream = if cfg!(target_os = "linux") {
        GstMediaStream::new(PublishOptions::Audio(AudioPublishOptions {
            codec: "audio/x-raw".to_string(),
            device_id: "hw:3".to_string(),
            framerate: 48000,
            channels: 1,
            selected_channel: None,
            local_file_save_options: Some(LocalFileSaveOptions {
                output_dir: "recordings".to_string(),
            }),
        }))
    } else {
        GstMediaStream::new(PublishOptions::Audio(AudioPublishOptions {
            codec: "audio/x-raw".to_string(),
            framerate: 48000,
            device_id: r"\\?\SWD#MMDEVAPI#{0.0.1.00000000}.{400ac096-5f57-4207-87c5-b9d208f12749}#{2eef81be-33fa-4800-9670-1cd474972c3f}".to_string(),
            local_file_save_options: Some(LocalFileSaveOptions {
                output_dir: "recordings".to_string(),
            }),
            channels: 2,
            selected_channel: Some(1)
        }))
    };

    let (room, mut room_rx) = Room::connect(&url, &token, RoomOptions::default())
        .await
        .unwrap();

    let new_room = Arc::new(room);

    stream.start().await?;

    let mut participant = LKParticipant::new(new_room.clone());

    participant.publish_stream(&mut stream, None).await?;

    log::info!(
        "Connected to room: {} - {}",
        new_room.name(),
        String::from(new_room.sid().await)
    );

    wait::wait_lk(&mut [stream], new_room.clone(), &mut room_rx).await
}

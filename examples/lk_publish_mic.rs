use dotenvy::dotenv;
use livekit::{Room, RoomEvent, RoomOptions};
use livekit_gstreamer::{
    AudioPublishOptions, GstMediaStream, LKParticipant, LKParticipantError, LocalFileSaveOptions,
    PublishOptions,
};

use livekit_api::access_token;
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

    let publish_options = AudioPublishOptions {
        codec: "audio/x-raw".to_string(),
        device_id: "hw:1".to_string(),
        framerate: 48000,
        channels: 1,
        selected_channel: None,
        local_file_save_options: Some(LocalFileSaveOptions {
            output_dir: "recordings".to_string(),
        }),
        // local_file_save_options: None,
    };

    let mut stream = GstMediaStream::new(PublishOptions::Audio(publish_options));

    stream.start().await?;

    let mut participant = LKParticipant::new(new_room.clone());

    participant.publish_stream(&mut stream, None).await?;

    log::info!(
        "Connected to room: {} - {}",
        new_room.name(),
        String::from(new_room.sid().await)
    );

    loop {
        tokio::select! {
            msg = room_rx.recv() => {
                match msg {
                    Some(RoomEvent::Disconnected { reason }) => {
                        log::info!("Disconnected from room: {:?}", reason);
                        stream.stop().await?;
                        break;
                    }
                    Some(other_event) => {
                        log::info!("Received room event: {:?}", other_event);
                    }
                    None => {
                        log::info!("Room event channel closed");
                        stream.stop().await?;
                        break;
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                log::info!("Received Ctrl+C, stopping stream and disconnecting");
                stream.stop().await?;
                new_room.close().await?;
                log::info!("Disconnected from room");
                break;
            }
        }
    }

    Ok(())
}

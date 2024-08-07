mod video_device;
use gstreamer::Buffer;
use livekit::prelude::*;
use livekit_api::access_token;
use std::{env, sync::Arc};
use tokio::sync::broadcast::{channel, Receiver};
use tokio::sync::oneshot::channel as oneshot_channel;
use video_device::{GSTVideoDevice, GStreamerError};

use dotenvy::dotenv;

async fn publish_frames(
    mut frame_rx: Receiver<Arc<Buffer>>,
    room: Arc<Room>,
    mut room_rx: Receiver<RoomEvent>,
) {
    loop {
        let data = frame_rx.recv().await.unwrap();
        println!("Time: {:?}", data.pts());
    }
}

#[tokio::main]
async fn main() -> Result<(), GStreamerError> {
    dotenv().ok();
    // Initialize gstreamer
    gstreamer::init().unwrap();
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    let url = env::var("LIVEKIT_URL").expect("LIVEKIT_URL is not set");
    let api_key = env::var("LIVEKIT_API_KEY").expect("LIVEKIT_API_KEY is not set");
    let api_secret = env::var("LIVEKIT_API_SECRET").expect("LIVEKIT_API_SECRET is not set");

    let (tx, _) = channel::<Arc<Buffer>>(1);
    let tx_arc = Arc::new(tx);
    let mut frame_rx = tx_arc.subscribe();
    let device = GSTVideoDevice::from_device_path("/dev/video0")?;

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

    log::info!(
        "Connected to room: {} - {}",
        new_room.clone().name(),
        String::from(new_room.clone().sid().await)
    );

    tokio::spawn(async move {
        publish_frames(frame_rx, new_room.clone(), room_rx).await;
    });

    device.pipeline("image/jpeg", 1280, 720, 30, tx_arc)?;

    Ok(())
}

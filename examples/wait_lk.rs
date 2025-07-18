use std::sync::Arc;

use livekit::{Room, RoomEvent};
use livekit_gstreamer::{GstMediaStream, LKParticipantError};
use tokio::sync::mpsc::UnboundedReceiver;

pub async fn wait(
    streams: &mut [GstMediaStream],
    room: Arc<Room>,
    room_rx: &mut UnboundedReceiver<RoomEvent>,
) -> Result<(), LKParticipantError> {
    loop {
        tokio::select! {
            msg = room_rx.recv() => {
                match msg {
                    Some(RoomEvent::Disconnected { reason }) => {
                        log::info!("Disconnected from room: {:?}", reason);
                        for stream in streams.iter_mut() {
                            stream.stop().await?;
                        }
                        break;
                    }
                    Some(other_event) => {
                        log::info!("Received room event: {:?}", other_event);
                    }
                    None => {
                        log::info!("Room event channel closed");
                        for stream in streams.iter_mut() {
                            stream.stop().await?;
                        }
                        break;
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                log::info!("Received Ctrl+C, stopping stream and disconnecting");
                for stream in streams.iter_mut() {
                    stream.stop().await?;
                }
                room.close().await?;
                log::info!("Disconnected from room");
                break;
            }
        }
    }

    Ok(())
}

use std::sync::Arc;

use livekit::{Room, RoomEvent};
use livekit_gstreamer::{GStreamerError, GstMediaStream, LKParticipantError};
use tokio::sync::mpsc::UnboundedReceiver;

#[allow(dead_code)]
pub async fn wait_lk(
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

#[allow(dead_code)]
pub async fn wait_stream(
    stream: &mut GstMediaStream,
    mut frame_rx: tokio::sync::broadcast::Receiver<Arc<gstreamer::Buffer>>,
    mut close_rx: tokio::sync::broadcast::Receiver<()>,
) -> Result<(), GStreamerError> {
    loop {
        tokio::select! {

            _ = tokio::signal::ctrl_c() => {
                println!("Received Ctrl+C, stopping stream");
                break;
            }
            _ = close_rx.recv() => {
                println!("Stream closed");
                break;
            }
            frame = frame_rx.recv() => {
                match frame {
                    Ok(frame) => {
                        println!(
                            "Received frame at {:?} microseconds",
                            frame.pts().unwrap_or_default().useconds()
                        );
                    }
                    Err(err) => {
                        println!("Error receiving frame: {:?}", err);
                        break;
                    }
                }
            }
        }
    }

    stream.stop().await?;

    Ok(())
}

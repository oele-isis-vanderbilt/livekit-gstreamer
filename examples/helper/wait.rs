use gstreamer::Buffer;
use livekit::{Room, RoomEvent};
use livekit_gstreamer::{GStreamerError, GstMediaStream, LKParticipantError};
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::{broadcast, mpsc};

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
pub async fn wait_streams(
    streams: &mut [GstMediaStream],
    frame_rxs: Vec<broadcast::Receiver<Arc<Buffer>>>,
    close_rxs: Vec<broadcast::Receiver<()>>,
) -> Result<(), GStreamerError> {
    println!("Waiting for multiple streams...");

    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

    for ((mut frame_rx, mut close_rx), stream_index) in
        frame_rxs.into_iter().zip(close_rxs).zip(0..)
    {
        let shutdown_tx = shutdown_tx.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = close_rx.recv() => {
                        println!("Stream {stream_index} closed");
                        let _ = shutdown_tx.send(()).await;
                        break;
                    }
                    result = frame_rx.recv() => {
                        match result {
                            Ok(buffer) => {
                                println!(
                                    "Stream {stream_index}: Received frame at {:?} µs ({} bytes)",
                                    buffer.pts().unwrap_or_default().useconds(),
                                    buffer.size()
                                );
                            }
                            Err(RecvError::Lagged(_)) => {
                                println!("Stream {stream_index}: Frame lagged, dropping...");
                            }
                            Err(RecvError::Closed) => {
                                println!("Stream {stream_index}: Receiver closed");
                                let _ = shutdown_tx.send(()).await;
                                break;
                            }
                        }
                    }
                }
            }
        });
    }

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("Received Ctrl+C");
        }
        _ = shutdown_rx.recv() => {
            println!("One of the streams terminated");
        }
    }

    for stream in streams.iter_mut() {
        stream.stop().await?;
    }

    Ok(())
}

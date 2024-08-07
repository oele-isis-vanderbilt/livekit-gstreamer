use gstreamer::Buffer;
use livekit::options::TrackPublishOptions;
use livekit::track::LocalTrack;
use livekit::track::TrackSource;
use livekit::webrtc::prelude::I420Buffer;
use livekit::webrtc::prelude::RtcVideoSource;
use livekit::webrtc::prelude::VideoFrame;
use livekit::webrtc::prelude::VideoRotation;
use livekit::RoomError;
use livekit::{
    track::LocalVideoTrack,
    webrtc::{prelude::VideoResolution, video_source::native::NativeVideoSource},
    Room
};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use crate::video_device::{run_pipeline, GSTVideoDevice, GStreamerError};


#[derive(Debug, Error)]
pub enum LivekitGSTTrackError {
    #[error("GStreamer error: {0}")]
    GStreamerError(#[from] GStreamerError),
    #[error("Room error: {0}")]
    RoomError(#[from] RoomError),
}

pub struct TrackHandle {
    close_tx: mpsc::Sender<()>,
    frame_tx: broadcast::Sender<Arc<Buffer>>,
    track: LocalVideoTrack,
    task: tokio::task::JoinHandle<()>,
}

#[derive(Debug, Clone)]
pub struct VideoPublishOptions {
    pub codec: String,
    pub device_id: String,
    pub width: i32,
    pub height: i32,
    pub framerate: i32,
}

pub struct LivekitGSTVideoTrack {
    rtc_source: NativeVideoSource,
    room: Arc<Room>,
    handle: Option<TrackHandle>,
    publish_options: VideoPublishOptions,
}

impl LivekitGSTVideoTrack {
    pub fn new(room: Arc<Room>, publish_options: VideoPublishOptions) -> Self {
        Self {
            rtc_source: NativeVideoSource::new(VideoResolution {
                width: publish_options.width as u32,
                height: publish_options.height as u32,
            }),
            room,
            handle: None,
            publish_options,
        }
    }

    pub fn is_published(&self) -> bool {
        self.handle.is_some()
    }

    pub async fn unpublish(&mut self) -> Result<(), LivekitGSTTrackError> {
        if let Some(handle) = self.handle.take() {
            let _ = handle.close_tx.send(());
            let _ = handle.task.await;
            self.room
                .local_participant()
                .unpublish_track(&handle.track.sid())
                .await?;
        }
        Ok(())
    }

    pub async fn publish(&mut self) -> Result<(), LivekitGSTTrackError> {
        self.unpublish().await?;

        let (frame_tx, _) = broadcast::channel::<Arc<Buffer>>(1);
        let (close_tx, mut close_rx) = mpsc::channel::<()>(1);

        let device = GSTVideoDevice::from_device_path(&self.publish_options.device_id)?;

        let frame_tx_arc = Arc::new(frame_tx.clone());

        let pipeline = device.pipeline(
            &self.publish_options.codec,
            self.publish_options.width,
            self.publish_options.height,
            self.publish_options.framerate,
            frame_tx_arc.clone(),
        )?;

        let track = LocalVideoTrack::create_video_track(
            &device.display_name,
            RtcVideoSource::Native(self.rtc_source.clone()),
        );

        let frames_rx = frame_tx.subscribe();

        let task = tokio::spawn(Self::track_task(
            close_rx,
            close_tx.clone(),
            frames_rx,
            self.rtc_source.clone(),
            pipeline,
        ));
        
        self.room.local_participant()
            .publish_track(
                LocalTrack::Video(track.clone()),
                TrackPublishOptions {
                    source: TrackSource::Camera,
                    ..Default::default()
                },
            )
            .await?;

        let handle = TrackHandle {
            close_tx: close_tx.clone(),
            frame_tx,
            track,
            task,
        };
        self.handle = Some(handle);

        Ok(())
    }

    pub fn subscribe(&self) -> Option<broadcast::Receiver<Arc<Buffer>>> {
        self.handle.as_ref().map(|h| h.frame_tx.subscribe())
    }

    async fn track_task(
        mut close_rx: mpsc::Receiver<()>,
        close_tx: mpsc::Sender<()>,
        mut frames_rx: broadcast::Receiver<Arc<Buffer>>,
        rtc_source: NativeVideoSource,
        pipeline: gstreamer::Pipeline,
    ) {
        let cloned = pipeline.clone();

        let pipeline_task = tokio::spawn(run_pipeline(cloned, close_tx));

        loop {
            tokio::select! {
                _ = close_rx.recv() => {
                    break;
                },
                frame = frames_rx.recv() => {
                    if let Ok(frame) = frame {
                        let map = frame.map_readable().unwrap();
                        let data = map.as_slice();
                        let timestamp_us = frame.pts().unwrap_or_default().useconds() as i64;
                        let res = rtc_source.video_resolution();
                        let width = res.width as u32;
                        let height = res.height as u32;
                        let mut wrtc_video_buffer = I420Buffer::new(width as u32, height as u32);
                        let (data_y, data_u, data_v) = wrtc_video_buffer.data_mut();

                        let y_plane_size = (width * height) as usize;
                        let uv_plane_size = (width * height / 4) as usize;

                        data_y.copy_from_slice(&data[0..y_plane_size]);
                        data_u.copy_from_slice(&data[y_plane_size..y_plane_size + uv_plane_size]);
                        data_v.copy_from_slice(
                            &data[y_plane_size + uv_plane_size..y_plane_size + 2 * uv_plane_size],
                        );

                        let video_frame = VideoFrame {
                            buffer: wrtc_video_buffer,
                            rotation: VideoRotation::VideoRotation0,
                            timestamp_us: timestamp_us,
                        };
                        rtc_source.capture_frame(&video_frame);
                    }
                }
            }
        }

        let _ = pipeline_task.await;
    }
    
}

impl Drop for LivekitGSTVideoTrack {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.close_tx.send(());
        }
    }
}

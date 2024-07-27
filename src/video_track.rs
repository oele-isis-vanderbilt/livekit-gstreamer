use livekit::prelude::*;
use livekit::webrtc::prelude::RtcVideoSource;
use livekit::webrtc::video_source::VideoResolution;
use livekit::options::TrackPublishOptions;
use livekit::webrtc::{
    video_frame::{I420Buffer, VideoFrame, VideoRotation},
    video_source::native::NativeVideoSource,
};

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use opencv::{core::Size, imgproc, prelude::*, videoio, Result as CvResult};

const FRAME_RATE: u64 = 30;
const FB_WIDTH: usize = 1280;
const FB_HEIGHT: usize = 720;

struct TrackHandle {
    close_tx: oneshot::Sender<()>,
    track: LocalVideoTrack,
    task: JoinHandle<std::result::Result<(), opencv::Error>>,
}

pub struct VideoTrack {
    rtc_source: NativeVideoSource,
    room: Arc<Room>,
    handle: Option<TrackHandle>,
}

impl VideoTrack {
    pub fn new(room: Arc<Room>) -> Self {
        Self {
            rtc_source: NativeVideoSource::new(VideoResolution {
                width: 1280,
                height: 720,
            }),
            room,
            handle: None,
        }
    }

    pub fn is_published(&self) -> bool {
        self.handle.is_some()
    }

    pub async fn publish(&mut self) -> Result<(), RoomError> {
        self.unpublish().await?;
        let (close_tx, close_rx) = oneshot::channel();

        let track = LocalVideoTrack::create_video_track("video-camera", RtcVideoSource::Native(self.rtc_source.clone()));
        let task = tokio::spawn(Self::track_task(close_rx, self.rtc_source.clone()));

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
            close_tx,
            track,
            task,
        };

        self.handle = Some(handle);
        Ok(())
    }

    pub async fn unpublish(&mut self) -> Result<(), RoomError> {
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

    async fn track_task(mut close_rx: oneshot::Receiver<()>, rtc_source: NativeVideoSource) -> CvResult<()> {
        let mut interval = tokio::time::interval(Duration::from_millis(1000 / FRAME_RATE));
        let mut cam = videoio::VideoCapture::new(4, videoio::CAP_ANY)?; // 0 is the default camera
        let opened = videoio::VideoCapture::is_opened(&cam)?;
        let mut timestamp_us = 0;

        if !opened {
            panic!("Unable to open default camera!");
        }

        loop {
            tokio::select! {
                _ = &mut close_rx => {
                    break
                }
                _ = interval.tick() => {}
            }

            let mut frame = Mat::default();
            cam.read(&mut frame)?;

            if frame.size()?.width > 0 && frame.size()?.height > 0 {
                let mut resized_frame = Mat::default();
                imgproc::resize(&frame, &mut resized_frame, Size::new(FB_WIDTH as i32, FB_HEIGHT as i32), 0.0, 0.0, imgproc::INTER_LINEAR)?;

                let mut yuv_frame = Mat::default();
                imgproc::cvt_color(&resized_frame, &mut yuv_frame, imgproc::COLOR_BGR2YUV_I420, 0)?;

                let yuv_data = yuv_frame.data_bytes()?;
                let (width, height) = (FB_WIDTH as i32, FB_HEIGHT as i32);

                let mut buffer = I420Buffer::new(width as u32, height as u32);
                let (data_y, data_u, data_v) = buffer.data_mut();

                let y_plane_size = (width * height) as usize;
                let uv_plane_size = (width * height / 4) as usize;

                data_y.copy_from_slice(&yuv_data[0..y_plane_size]);
                data_u.copy_from_slice(&yuv_data[y_plane_size..y_plane_size + uv_plane_size]);
                data_v.copy_from_slice(&yuv_data[y_plane_size + uv_plane_size..y_plane_size + 2 * uv_plane_size]);

                let video_frame = VideoFrame {
                    buffer: buffer,
                    rotation: VideoRotation::VideoRotation0,
                    timestamp_us: 0,
                };

                rtc_source.capture_frame(&video_frame);
            }
        }

        Ok(())
    }
}
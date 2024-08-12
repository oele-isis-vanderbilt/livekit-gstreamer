use crate::media_device::GStreamerError;
use crate::utils::random_string;
use crate::video_stream::GstVideoStream;
use gstreamer::Buffer;
use livekit::options::TrackPublishOptions;
use livekit::track::{LocalTrack, LocalVideoTrack, TrackSource};
use livekit::webrtc::prelude::{
    I420Buffer, RtcVideoSource, VideoFrame, VideoResolution, VideoRotation,
};
use livekit::webrtc::video_source::native::NativeVideoSource;
use livekit::{Room, RoomError};
use std::collections::HashMap;
use std::sync::Arc;

use thiserror::Error;
use tokio::sync::broadcast;

#[derive(Error, Debug)]
pub enum LKParticipantError {
    #[error("GStreamer error: {0}")]
    GStreamerError(#[from] GStreamerError),
    #[error("Livekit error: {0}")]
    LivekitError(#[from] RoomError),
}

pub struct LKParticipant {
    room: Arc<Room>,
    published_tracks: HashMap<String, TrackHandle>,
}

struct TrackHandle {
    track: LocalVideoTrack,
    task: tokio::task::JoinHandle<()>,
}

impl LKParticipant {
    pub fn new(room: Arc<Room>) -> Self {
        Self {
            room,
            published_tracks: HashMap::new(),
        }
    }

    pub async fn publish_video_stream(
        &mut self,
        stream: &mut GstVideoStream,
        track_name: Option<String>,
    ) -> Result<String, LKParticipantError> {
        if !stream.has_started() {
            stream.start().await?;
        }
        // This unwrap is safe because we know the stream has started
        let (frames_rx, close_rx) = stream.subscribe().unwrap();
        let details = stream.details().unwrap();
        let track_name = track_name.unwrap_or(stream.get_device_name().unwrap());
        let rtc_source = NativeVideoSource::new(VideoResolution {
            width: details.width as u32,
            height: details.height as u32,
        });

        let track = LocalVideoTrack::create_video_track(
            &track_name,
            RtcVideoSource::Native(rtc_source.clone()),
        );

        let track_sid = random_string("track");

        let task = tokio::spawn(Self::track_task(close_rx, frames_rx, rtc_source.clone()));

        self.room
            .local_participant()
            .publish_track(
                LocalTrack::Video(track.clone()),
                TrackPublishOptions {
                    source: TrackSource::Camera,
                    ..Default::default()
                },
            )
            .await?;

        self.published_tracks
            .insert(track_sid.clone(), TrackHandle { track, task });

        Ok(track_sid)
    }

    async fn track_task(
        mut close_rx: broadcast::Receiver<()>,
        mut frames_rx: broadcast::Receiver<Arc<Buffer>>,
        rtc_source: NativeVideoSource,
    ) {
        loop {
            tokio::select! {
                _ = close_rx.recv() => {
                    break;
                }
                frame = frames_rx.recv() => {
                    if let Ok(frame) = frame {
                        let map = frame.map_readable().unwrap();
                        let data = map.as_slice();
                        let timestamp_us = frame.pts().unwrap_or_default().useconds() as i64;
                        let res = rtc_source.video_resolution();
                        let width = res.width;
                        let height = res.height;
                        let mut wrtc_video_buffer = I420Buffer::new(width, height);
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
                            timestamp_us,
                        };
                        rtc_source.capture_frame(&video_frame);
                    }
                }
            }
        }
    }

    pub async fn unpublish_track(&mut self, track_sid: &str) -> Result<(), LKParticipantError> {
        if let Some(handle) = self.published_tracks.get(track_sid) {
            self.room
                .local_participant()
                .unpublish_track(&handle.track.sid())
                .await?;
            handle.task.abort();
        }
        Ok(())
    }
}

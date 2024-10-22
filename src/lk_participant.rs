use crate::media_device::GStreamerError;
use crate::media_stream::{GstMediaStream, PublishOptions};
use crate::utils::random_string;
use gstreamer::Buffer;
use livekit::options::TrackPublishOptions;
use livekit::track::{LocalAudioTrack, LocalTrack, LocalVideoTrack, TrackSource};
use livekit::webrtc::audio_source::native::NativeAudioSource;
use livekit::webrtc::prelude::{
    AudioFrame, I420Buffer, RtcAudioSource, RtcVideoSource, VideoFrame, VideoResolution,
    VideoRotation,
};
use livekit::webrtc::video_source::native::NativeVideoSource;
use livekit::{Room, RoomError};
use std::borrow::Cow;
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
    #[error("Streaming error: {0}")]
    StreamingError(String),
}

pub struct LKParticipant {
    room: Arc<Room>,
    published_tracks: HashMap<String, TrackHandle>,
}

struct TrackHandle {
    track: LocalTrack,
    task: tokio::task::JoinHandle<()>,
}

impl LKParticipant {
    pub fn new(room: Arc<Room>) -> Self {
        Self {
            room,
            published_tracks: HashMap::new(),
        }
    }

    pub async fn publish_stream(
        &mut self,
        stream: &mut GstMediaStream,
        track_name: Option<String>,
    ) -> Result<String, LKParticipantError> {
        if !stream.has_started() {
            stream.start().await?;
        }
        // This unwrap is safe because we know the stream has started
        let (frames_rx, close_rx) = stream.subscribe().unwrap();
        let details = stream.details().unwrap();
        let track_name = track_name.unwrap_or(stream.get_device_name().unwrap());

        match details {
            PublishOptions::Video(details) => {
                let rtc_source = NativeVideoSource::new(VideoResolution {
                    width: details.width as u32,
                    height: details.height as u32,
                });

                let track = LocalVideoTrack::create_video_track(
                    &track_name,
                    RtcVideoSource::Native(rtc_source.clone()),
                );

                let track_sid = random_string("video-track");

                let task = tokio::spawn(Self::video_track_task(
                    close_rx,
                    frames_rx,
                    rtc_source.clone(),
                ));

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

                self.published_tracks.insert(
                    track_sid.clone(),
                    TrackHandle {
                        track: LocalTrack::Video(track),
                        task,
                    },
                );

                Ok(track_sid)
            }
            PublishOptions::Audio(details) => {
                let rtc_source =
                    NativeAudioSource::new(Default::default(), details.framerate as u32, 1, 2000);

                let track = LocalAudioTrack::create_audio_track(
                    &track_name,
                    RtcAudioSource::Native(rtc_source.clone()),
                );

                let track_sid = random_string("audio-track");

                let task = tokio::spawn(Self::audio_track_task(
                    close_rx,
                    frames_rx,
                    rtc_source.clone(),
                ));

                self.room
                    .local_participant()
                    .publish_track(
                        LocalTrack::Audio(track.clone()),
                        TrackPublishOptions {
                            source: TrackSource::Microphone,
                            ..Default::default()
                        },
                    )
                    .await?;

                self.published_tracks.insert(
                    track_sid.clone(),
                    TrackHandle {
                        track: LocalTrack::Audio(track),
                        task,
                    },
                );

                Ok(track_sid)
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

    async fn video_track_task(
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

    async fn audio_track_task(
        mut close_rx: broadcast::Receiver<()>,
        mut frames_rx: broadcast::Receiver<Arc<Buffer>>,
        rtc_source: NativeAudioSource,
    ) {
        loop {
            tokio::select! {
                    _ = close_rx.recv() => {
                        break;
                    }
                    frame = frames_rx.recv() => {
                        if let Ok(frame) = frame {
                            let map = frame.map_readable().unwrap();
                            let audio_data: &[i16] = unsafe {
                                std::slice::from_raw_parts(map.as_ptr() as *const i16, map.size() / 2)
                            };
                            let samples_per_channel = audio_data.len() as u32 / rtc_source.num_channels();
                            let audio_frame = AudioFrame {
                                data: Cow::Borrowed(audio_data),
                                sample_rate: rtc_source.sample_rate(),
                                num_channels: rtc_source.num_channels(),
                                samples_per_channel,
                            };
                            rtc_source.capture_frame(&audio_frame).await.unwrap();
                    }
                }
            }
        }
    }
}

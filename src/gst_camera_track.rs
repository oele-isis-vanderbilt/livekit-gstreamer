use gstreamer::{prelude::*, ElementFactory};
use gstreamer_app::AppSink;
use livekit::options::TrackPublishOptions;
use livekit::prelude::*;
use livekit::webrtc::prelude::RtcVideoSource;
use livekit::webrtc::video_source::VideoResolution;
use livekit::webrtc::{
    video_frame::{I420Buffer, VideoFrame, VideoRotation},
    video_source::native::NativeVideoSource,
};

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

#[allow(dead_code)]
pub enum VideoPreset {
    H1080p,
    H720p,
    H480p,
    H360p,
    H240p,
}

struct TrackHandle {
    close_tx: oneshot::Sender<()>,
    track: LocalVideoTrack,
    task: JoinHandle<()>,
}

impl VideoPreset {
    pub fn resolution(&self) -> VideoResolution {
        match self {
            VideoPreset::H1080p => VideoResolution {
                width: 1920,
                height: 1080,
            },
            VideoPreset::H720p => VideoResolution {
                width: 1280,
                height: 720,
            },
            VideoPreset::H480p => VideoResolution {
                width: 854,
                height: 480,
            },
            VideoPreset::H360p => VideoResolution {
                width: 640,
                height: 360,
            },
            VideoPreset::H240p => VideoResolution {
                width: 426,
                height: 240,
            },
        }
    }
}

pub struct GSTCameraTrack {
    rtc_source: NativeVideoSource,
    room: Option<Arc<Room>>,
    device: String,
    frame_format: String,
    preset: VideoPreset,
    handle: Option<TrackHandle>,
}

impl GSTCameraTrack {
    pub fn new(
        device: &str,
        frame_format: &str,
        preset: VideoPreset,
        room: Option<Arc<Room>>,
    ) -> Self {
        Self {
            rtc_source: NativeVideoSource::new(preset.resolution()),
            room,
            device: device.to_string(),
            frame_format: frame_format.to_string(),
            preset,
            handle: None,
        }
    }

    fn get_show_pipeline(&self) -> gstreamer::Pipeline {
        let src = gstreamer::ElementFactory::make("v4l2src")
            .name("source")
            .build()
            .expect("Failed to create source element");

        // Set the device
        src.set_property("device", &self.device);

        let capsfilter = ElementFactory::make("capsfilter")
            .name("filter")
            .build()
            .expect("Failed to create capsfilter element");

        let resolution = self.preset.resolution();

        // Create the caps for image/jpeg
        let caps = gstreamer::Caps::builder("image/jpeg")
            .field("width", resolution.width as i32)
            .field("height", resolution.height as i32)
            .field("framerate", gstreamer::Fraction::new(30, 1))
            .build();
        capsfilter.set_property("caps", &caps);

        let jpeg_dec = gstreamer::ElementFactory::make("jpegdec")
            .name("jpegdec")
            .build()
            .expect("Failed to create jpegdec element");

        let raw_filter = ElementFactory::make("capsfilter")
            .name("raw_filter")
            .build()
            .expect("Failed to create raw_filter element");

        let raw_caps = gstreamer::Caps::builder("video/x-raw")
            .field("format", &self.frame_format)
            .build();

        raw_filter.set_property("caps", &raw_caps);

        let sink = gstreamer::ElementFactory::make("autovideosink")
            .name("sink")
            .build()
            .expect("Failed to create sink element");

        let pipeline = gstreamer::Pipeline::with_name("camera-pipeline");
        pipeline
            .add_many([&src, &capsfilter, &jpeg_dec, &raw_filter, &sink])
            .unwrap();
        gstreamer::Element::link_many([&src, &capsfilter, &jpeg_dec, &raw_filter, &sink]).unwrap();

        pipeline
    }

    pub fn get_sink_pipeline(&self) -> (gstreamer::Pipeline, gstreamer::Element) {
        let src = gstreamer::ElementFactory::make("v4l2src")
            .name("source")
            .build()
            .expect("Failed to create source element");

        // Set the device
        src.set_property("device", &self.device);

        let capsfilter = ElementFactory::make("capsfilter")
            .name("filter")
            .build()
            .expect("Failed to create capsfilter element");

        let resolution = self.preset.resolution();

        // Create the caps for image/jpeg
        let caps = gstreamer::Caps::builder("image/jpeg")
            .field("width", resolution.width as i32)
            .field("height", resolution.height as i32)
            .field("framerate", gstreamer::Fraction::new(30, 1))
            .build();
        capsfilter.set_property("caps", &caps);

        let jpeg_dec = gstreamer::ElementFactory::make("jpegdec")
            .name("jpegdec")
            .build()
            .expect("Failed to create jpegdec element");

        let raw_filter = ElementFactory::make("capsfilter")
            .name("raw_filter")
            .build()
            .expect("Failed to create raw_filter element");

        let raw_caps = gstreamer::Caps::builder("video/x-raw")
            .field("format", &self.frame_format)
            .build();

        raw_filter.set_property("caps", &raw_caps);

        let sink = gstreamer::ElementFactory::make("appsink")
            .name("sink")
            .build()
            .expect("Failed to create sink element");

        let pipeline = gstreamer::Pipeline::with_name("camera-pipeline");
        pipeline
            .add_many([&src, &capsfilter, &jpeg_dec, &raw_filter, &sink])
            .unwrap();
        gstreamer::Element::link_many([&src, &capsfilter, &jpeg_dec, &raw_filter, &sink]).unwrap();
        (pipeline, sink)
    }

    pub fn show(&self) {
        let pipeline = self.get_show_pipeline();
        pipeline.set_state(gstreamer::State::Playing).unwrap();
        let bus = pipeline.bus().unwrap();

        for msg in bus.iter_timed(gstreamer::ClockTime::NONE) {
            match msg.view() {
                gstreamer::MessageView::Eos(..) => {
                    break;
                }
                gstreamer::MessageView::Error(err) => {
                    eprintln!(
                        "Error from element {}: {}",
                        msg.src()
                            .map(|s| s.path_string())
                            .as_deref()
                            .unwrap_or("None"),
                        err.error()
                    );
                    eprintln!("Debugging information: {:?}", err.debug());
                    break;
                }
                _ => (),
            }
        }

        pipeline.set_state(gstreamer::State::Null).unwrap();
    }

    pub async fn publish(&mut self) -> Result<(), RoomError> {
        self.unpublish().await?;

        let (close_tx, close_rx) = oneshot::channel();

        let track = LocalVideoTrack::create_video_track(
            "video-camera",
            RtcVideoSource::Native(self.rtc_source.clone()),
        );

        let (pipeline, sink) = self.get_sink_pipeline();

        let task = tokio::spawn(Self::track_task(
            close_rx,
            pipeline,
            sink,
            self.rtc_source.clone(),
        ));

        let room = self.room.as_ref().ok_or(RoomError::AlreadyClosed)?;
        room.local_participant()
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
        let room = self.room.as_ref().ok_or(RoomError::AlreadyClosed)?;

        if let Some(handle) = self.handle.take() {
            let _ = handle.close_tx.send(());
            let _ = handle.task.await;
            room.local_participant()
                .unpublish_track(&handle.track.sid())
                .await?;
        }

        Ok(())
    }

    pub fn is_published(&self) -> bool {
        self.handle.is_some()
    }

    async fn track_task(
        mut close_rx: oneshot::Receiver<()>,
        pipeline: gstreamer::Pipeline,
        sink: gstreamer::Element,
        rtc_source: NativeVideoSource,
    ) {
        let mut interval = tokio::time::interval(Duration::from_millis(1000 / 30));
        pipeline.set_state(gstreamer::State::Playing).unwrap();
        let appsink = sink.dynamic_cast::<AppSink>().unwrap();
        loop {
            tokio::select! {
                _ = &mut close_rx => {
                    pipeline.set_state(gstreamer::State::Null).unwrap();
                    break;
                }
                _ = interval.tick() => {}
            }

            let sample = appsink.pull_sample().unwrap();
            let buffer = sample.buffer().unwrap();
            let map = buffer.map_readable().unwrap();

            // Process the I420 frame data
            let data = map.as_slice();
            let width = 1920;
            let height = 1080;
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
                timestamp_us: 0,
            };

            rtc_source.capture_frame(&video_frame);
        }
    }
}

impl Drop for GSTCameraTrack {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.close_tx.send(());
        }
    }
}

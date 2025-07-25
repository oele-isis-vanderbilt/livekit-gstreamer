use gstreamer::{prelude::*, Buffer};
use gstreamer_app::AppSink;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use thiserror::Error;
use tokio::sync::broadcast;

use crate::get_device_capabilities;
use crate::utils::random_string;
use crate::utils::system_time_nanos;
use crate::{get_gst_device, get_monitor};

const SUPPORTED_VIDEO_CODECS: [&str; 2] = ["video/x-h264", "image/jpeg"];
const SUPPORTED_AUDIO_CODECS: [&str; 1] = ["audio/x-raw"];
const VIDEO_FRAME_FORMAT: &str = "I420";

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
struct FileSinkTiming {
    start_time: Option<i64>,
    end_time: Option<i64>,
}

/// A struct representing a GStreamer device
/// This implementation assumes that GStreamer is initialized elsewhere
#[derive(Debug, Clone)]
pub struct GstMediaDevice {
    pub display_name: String,
    #[allow(dead_code)]
    pub device_class: String,
    pub device_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingMetadata {
    pub filename: String,
    pub parent_dir: String,
    pub source: String,
    pub media_type: String,
    start_time: Option<i64>,
    end_time: Option<i64>,
    pub codec: String,
    pub audio_channel: Option<i32>,
}

impl RecordingMetadata {
    pub fn new(
        filename: String,
        parent_dir: String,
        source: String,
        media_type: String,
        codec: String,
        audio_channel: Option<i32>,
    ) -> Self {
        RecordingMetadata {
            filename,
            parent_dir,
            source,
            media_type,
            start_time: None,
            end_time: None,
            codec,
            audio_channel,
        }
    }

    pub fn set_start_time(&mut self, time: i64) {
        self.start_time = Some(time);
    }

    pub fn set_end_time(&mut self, time: i64) {
        self.end_time = Some(time);
    }

    pub fn start_time(&self) -> Option<i64> {
        self.start_time
    }

    pub fn end_time(&self) -> Option<i64> {
        self.end_time
    }

    pub fn write_success(&self) -> Result<bool, GStreamerError> {
        let parent_dir = PathBuf::from(&self.parent_dir);

        let string_content = serde_json::to_string(&self).map_err(|e| {
            GStreamerError::PipelineError(format!("Failed to serialize metadata: {}", e))
        })?;

        let metadata_file = format!("{}.json", self.filename);

        std::fs::write(parent_dir.join(metadata_file), string_content).map_err(|e| {
            GStreamerError::PipelineError(format!("Failed to write metadata: {}", e))
        })?;

        Ok(true)
    }

    pub fn write_error(&self, error: &str) -> Result<bool, GStreamerError> {
        let parent_dir = PathBuf::from(&self.parent_dir);

        let error_object = serde_json::json!({
            "error": error,
            "filename": self.filename,
            "parent_dir": self.parent_dir,
            "source": self.source,
            "media_type": self.media_type,
            "codec": self.codec,
            "audio_channel": self.audio_channel,
        });

        let string_content = serde_json::to_string(&error_object).map_err(|e| {
            GStreamerError::PipelineError(format!("Failed to serialize error metadata: {}", e))
        })?;

        let metadata_file = format!("{}.error.json", self.filename);

        std::fs::write(parent_dir.join(metadata_file), string_content).map_err(|e| {
            GStreamerError::PipelineError(format!("Failed to write metadata: {}", e))
        })?;

        Ok(true)
    }
}

pub async fn run_pipeline(
    pipeline: gstreamer::Pipeline,
    tx: broadcast::Sender<()>,
    mut recording_metadata: Option<RecordingMetadata>,
) -> Result<(), GStreamerError> {
    let timing = Arc::new(Mutex::new(FileSinkTiming::default()));

    if recording_metadata.is_some() {
        let filesink = pipeline.iterate_elements().find(|e| {
            let factory = e.factory();
            factory.map(|f| f.name() == *"filesink").unwrap_or(false)
        });

        if let Some(filesink) = filesink {
            let timing_clone = timing.clone();
            if let Some(sink_pad) = filesink.static_pad("sink") {
                sink_pad.add_probe(gstreamer::PadProbeType::BUFFER, move |_, info| {
                    if let Some(gstreamer::PadProbeData::Buffer(ref buffer)) = info.data {
                        if buffer.pts().is_some() {
                            let mut timing = timing_clone.lock().unwrap();
                            if timing.start_time.is_none() {
                                timing.start_time = Some(system_time_nanos());
                            }
                            timing.end_time = Some(system_time_nanos());
                        }
                    }
                    gstreamer::PadProbeReturn::Ok
                });
            }
        }
    }

    pipeline
        .set_state(gstreamer::State::Playing)
        .map_err(|err| {
            println!(
                "Failed to set pipeline to Playing state: {:?}",
                err.to_string()
            );
            GStreamerError::PipelineError("Failed to set pipeline to Playing state".to_string())
        })?;
    let bus = pipeline.bus().unwrap();
    for msg in bus.iter_timed(gstreamer::ClockTime::NONE) {
        use gstreamer::MessageView;
        match msg.view() {
            MessageView::Eos(..) => {
                if let Some(metadata) = recording_metadata.as_mut() {
                    metadata.set_end_time(system_time_nanos());
                    // Get more reliable timestamps from the Filesink
                    if let Some(start_time) = timing.lock().unwrap().start_time {
                        metadata.set_start_time(start_time);
                    }
                    if let Some(end_time) = timing.lock().unwrap().end_time {
                        metadata.set_end_time(end_time);
                    }
                    let _ = metadata.write_success();
                }
                break;
            }
            MessageView::Error(err) => {
                if let Some(metadata) = recording_metadata.as_mut() {
                    let _ =
                        metadata.write_error(&format!("Pipeline error: {}", err.error().message()));
                }
                break;
            }
            MessageView::StateChanged(e) => {
                if let Some(metadata) = recording_metadata.as_mut() {
                    if e.current() == gstreamer::State::Playing {
                        metadata.set_start_time(system_time_nanos());
                    }
                }
                if e.current() == gstreamer::State::Null {
                    break;
                }
            }
            _ => (),
        }
    }
    pipeline.set_state(gstreamer::State::Null).map_err(|_| {
        GStreamerError::PipelineError("Failed to set pipeline to Null state".to_string())
    })?;
    tx.send(())
        .map_err(|_| GStreamerError::PipelineError("Failed to send signal".to_string()))?;
    Ok(())
}

impl GstMediaDevice {
    pub fn from_device_path(path: &str) -> Result<Self, GStreamerError> {
        let device = get_gst_device(path);
        let device =
            device.ok_or_else(|| GStreamerError::DeviceError("No device found".to_string()))?;
        let display_name: String = device.display_name().into();

        let device = GstMediaDevice {
            display_name,
            device_class: device.device_class().into(),
            device_path: path.into(),
        };
        Ok(device)
    }

    pub fn from_screen_id_or_name(screen_id_or_name: &str) -> Result<Self, GStreamerError> {
        let (monitor, _) = get_monitor(screen_id_or_name)
            .ok_or_else(|| GStreamerError::DeviceError("No screen found".to_string()))?;

        let device = GstMediaDevice {
            display_name: monitor.display_name,
            device_class: "Screen/Source".to_string(),
            device_path: monitor.device_path,
        };

        Ok(device)
    }

    pub fn capabilities(&self) -> Vec<MediaCapability> {
        if self.device_class == "Screen/Source" {
            if cfg!(target_os = "windows") {
                if let Some((monitor, _)) = get_monitor(&self.device_path) {
                    return monitor.capabilities;
                } else {
                    return vec![];
                }
            } else {
                return get_monitor(&self.device_path).map_or(vec![], |(m, _)| m.capabilities);
            }
        }
        let device = get_gst_device(&self.device_path).unwrap();
        get_device_capabilities(&device)
    }

    pub fn screen_share_pipeline(
        &self,
        codec: &str,
        width: i32,
        height: i32,
        framerate: i32,
        tx: Arc<broadcast::Sender<Arc<Buffer>>>,
        filename: Option<String>,
    ) -> Result<gstreamer::Pipeline, GStreamerError> {
        if self.device_class != "Screen/Source" {
            return Err(GStreamerError::PipelineError(
                "Device is not a screen source".to_string(),
            ));
        }

        let can_support = self.supports_screen_share(codec, width, height, framerate);

        if !can_support {
            return Err(GStreamerError::PipelineError(
                "Device does not support requested configuration".to_string(),
            ));
        }

        let element = self.get_screen_element()?;

        let video_convert = gstreamer::ElementFactory::make("videoconvert")
            .name(random_string("videoconvert"))
            .build()
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to create videoconvert".to_string())
            })?;

        let video_scale = gstreamer::ElementFactory::make("videoscale")
            .name(random_string("videoscale"))
            .build()
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to create videoscale".to_string())
            })?;

        let caps = gstreamer::Caps::builder("video/x-raw")
            .field("format", VIDEO_FRAME_FORMAT)
            .field("width", width)
            .field("height", height)
            .field("framerate", gstreamer::Fraction::new(framerate, 1))
            .field("pixel-aspect-ratio", gstreamer::Fraction::new(1, 1))
            .build();

        let caps_filter = gstreamer::ElementFactory::make("capsfilter")
            .name(random_string("capsfilter"))
            .build()
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to create capsfilter".to_string())
            })?;

        caps_filter.set_property("caps", &caps);

        let tee = gstreamer::ElementFactory::make("tee")
            .name(random_string("tee"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("Failed to create tee".to_string()))?;

        let queue_appsink = gstreamer::ElementFactory::make("queue")
            .name(random_string("queue-appsink"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("Failed to create queue".to_string()))?;

        let broadcast_appsink = self.broadcast_appsink(tx, Some(&caps))?;

        let pipeline = gstreamer::Pipeline::with_name(&random_string("stream-screen-share"));

        pipeline
            .add_many([
                &element,
                &video_convert,
                &video_scale,
                &caps_filter,
                &tee,
                &queue_appsink,
                broadcast_appsink.upcast_ref(),
            ])
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to add elements to pipeline".to_string())
            })?;

        gstreamer::Element::link_many([&element, &video_convert, &video_scale, &caps_filter, &tee])
            .map_err(|e| GStreamerError::PipelineError(e.to_string()))?;

        let tee_appsink_pad = tee.request_pad_simple("src_%u").ok_or_else(|| {
            GStreamerError::PipelineError("Failed to request tee pad for appsink".into())
        })?;

        let queue_appsink_pad = queue_appsink
            .static_pad("sink")
            .ok_or_else(|| GStreamerError::PipelineError("Appsink queue has no sink pad".into()))?;

        tee_appsink_pad.link(&queue_appsink_pad).map_err(|_| {
            GStreamerError::PipelineError("Failed to link tee to appsink queue".into())
        })?;

        gstreamer::Element::link_many([&queue_appsink, broadcast_appsink.upcast_ref()])
            .map_err(|_| GStreamerError::PipelineError("Failed to link appsink".to_string()))?;

        if let Some(ref path) = filename {
            self.add_video_file_branch(&pipeline, &tee, path)?;
        }

        pipeline
            .iterate_elements()
            .foreach(|e| {
                let _ = e.sync_state_with_parent();
            })
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to sync state with parent".to_string())
            })?;

        Ok(pipeline)
    }

    pub fn video_pipeline(
        &self,
        codec: &str,
        width: i32,
        height: i32,
        framerate: i32,
        tx: Arc<broadcast::Sender<Arc<Buffer>>>,
        filename: Option<String>,
    ) -> Result<gstreamer::Pipeline, GStreamerError> {
        if self.device_class == "Audio/Source" {
            return Err(GStreamerError::PipelineError(
                "Device is an audio source".to_string(),
            ));
        }

        if !SUPPORTED_VIDEO_CODECS.contains(&codec) {
            return Err(GStreamerError::PipelineError(format!(
                "Unsupported codec {}",
                codec
            )));
        }

        let can_support = self.supports_video(codec, width, height, framerate);
        if !can_support {
            return Err(GStreamerError::PipelineError(
                "Device does not support requested configuration".to_string(),
            ));
        }
        if codec == "video/x-raw" {
            return self.video_xraw_pipeline(width, height, framerate, tx, filename);
        } else if codec == "video/x-h264" {
            return self.video_xh264_pipeline(width, height, framerate, tx, filename);
        } else if codec == "image/jpeg" {
            return self.image_jpeg_pipeline(width, height, framerate, tx, filename);
        }

        Err(GStreamerError::PipelineError(
            "Failed to create pipeline".to_string(),
        ))
    }

    pub fn audio_pipeline(
        &self,
        codec: &str,
        channels: i32,
        framerate: i32,
        tx: Arc<broadcast::Sender<Arc<Buffer>>>,
        filename: Option<String>,
    ) -> Result<gstreamer::Pipeline, GStreamerError> {
        if self.device_class == "Video/Source" {
            return Err(GStreamerError::PipelineError(
                "Device is a video source".to_string(),
            ));
        }

        if !SUPPORTED_AUDIO_CODECS.contains(&codec) {
            return Err(GStreamerError::PipelineError(format!(
                "Unsupported codec {}",
                codec
            )));
        }

        let can_support = self.supports_audio(codec, channels, framerate);
        if !can_support {
            return Err(GStreamerError::PipelineError(
                "Device does not support requested configuration".to_string(),
            ));
        }
        self.audio_xraw_pipeline(channels, framerate, tx, filename)
    }

    pub fn deinterleaved_audio_pipeline(
        &self,
        codec: &str,
        channels: i32,
        selected_channel: i32,
        framerate: i32,
        tx: Arc<broadcast::Sender<Arc<Buffer>>>,
        filename: Option<String>,
    ) -> Result<gstreamer::Pipeline, GStreamerError> {
        if self.device_class == "Video/Source" {
            return Err(GStreamerError::PipelineError(
                "Device is a video source".to_string(),
            ));
        }

        if !SUPPORTED_AUDIO_CODECS.contains(&codec) {
            return Err(GStreamerError::PipelineError(format!(
                "Unsupported codec {}",
                codec
            )));
        }

        let can_support = self.supports_audio(codec, channels, framerate);
        if !can_support {
            return Err(GStreamerError::PipelineError(
                "Device does not support requested configuration".to_string(),
            ));
        }

        self.audio_deinterleaved_pipeline(selected_channel, channels, framerate, tx, filename)
    }

    fn audio_deinterleaved_pipeline(
        &self,
        selected_channel: i32,
        channels: i32,
        framerate: i32,
        tx: Arc<broadcast::Sender<Arc<Buffer>>>,
        filename: Option<String>,
    ) -> Result<gstreamer::Pipeline, GStreamerError> {
        let audio_el = self.get_audio_element()?;
        let convert = gstreamer::ElementFactory::make("audioconvert")
            .name(random_string("audioconvert"))
            .build()
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to create audioconvert".to_string())
            })?;

        let caps = gstreamer::Caps::builder("audio/x-raw")
            .field("format", "S16LE")
            .field("channels", channels)
            .field("rate", framerate)
            .field("channel-mask", gstreamer::Bitmask::new((1 << channels) - 1))
            .build();

        let caps_element = gstreamer::ElementFactory::make("capsfilter")
            .name(random_string("capsfilter"))
            .build()
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to create capsfilter".to_string())
            })?;

        caps_element.set_property("caps", caps);

        let deinterleave_element = gstreamer::ElementFactory::make("deinterleave")
            .name(random_string("deinterleave"))
            .build()
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to create deinterleave".to_string())
            })?;

        let queue = gstreamer::ElementFactory::make("queue")
            .name(random_string("queue"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("Failed to create queue".to_string()))?;

        let tee = gstreamer::ElementFactory::make("tee")
            .name(random_string("tee"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("Failed to create tee".to_string()))?;

        let queue_appsink = gstreamer::ElementFactory::make("queue")
            .name(random_string("queue-appsink"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("Failed to create queue".to_string()))?;

        let broadcast_appsink = self.broadcast_appsink(tx, None)?;

        let pipeline = gstreamer::Pipeline::with_name(&random_string("deinterleaved-audio-xraw"));

        pipeline
            .add_many([
                &audio_el,
                &convert,
                &caps_element,
                &deinterleave_element,
                &queue,
                &tee,
                &queue_appsink,
                (broadcast_appsink.upcast_ref()),
            ])
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to add elements to pipeline".to_string())
            })?;

        gstreamer::Element::link_many([&audio_el, &convert, &caps_element, &deinterleave_element])
            .map_err(|_| GStreamerError::PipelineError("Failed to link elements".to_string()))?;

        let cloned = queue.clone();

        deinterleave_element.connect_pad_added(move |_, src_pad| {
            let pad_name = src_pad.name();
            if pad_name == format!("src_{}", selected_channel - 1) {
                let queue_sink_pad = cloned.static_pad("sink").unwrap();
                if queue_sink_pad.is_linked() {
                    return;
                }
                src_pad.link(&queue_sink_pad).unwrap();
            }
        });

        gstreamer::Element::link_many([&queue, &tee]).map_err(|_| {
            GStreamerError::PipelineError("Failed to link queue and tee".to_string())
        })?;

        let tee_appsink_pad = tee.request_pad_simple("src_%u").ok_or_else(|| {
            GStreamerError::PipelineError("Failed to request tee pad for appsink".into())
        })?;

        let queue_appsink_pad = queue_appsink
            .static_pad("sink")
            .ok_or_else(|| GStreamerError::PipelineError("Appsink queue has no sink pad".into()))?;

        tee_appsink_pad.link(&queue_appsink_pad).map_err(|_| {
            GStreamerError::PipelineError("Failed to link tee to appsink queue".into())
        })?;

        gstreamer::Element::link_many([&queue_appsink, broadcast_appsink.upcast_ref()])
            .map_err(|_| GStreamerError::PipelineError("Failed to link appsink".to_string()))?;

        if let Some(ref path) = filename {
            self.add_audio_file_branch(&pipeline, &tee, path)?;
        }

        Ok(pipeline)
    }

    fn audio_xraw_pipeline(
        &self,
        channels: i32,
        framerate: i32,
        tx: Arc<broadcast::Sender<Arc<Buffer>>>,
        filename: Option<String>,
    ) -> Result<gstreamer::Pipeline, GStreamerError> {
        let audio_el = self.get_audio_element()?;
        let convert = gstreamer::ElementFactory::make("audioconvert")
            .name(random_string("audioconvert"))
            .build()
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to create audioconvert".to_string())
            })?;

        let caps = gstreamer::Caps::builder("audio/x-raw")
            .field("format", "S16LE")
            .field("channels", channels)
            .field("rate", framerate)
            .build();

        let caps_element = gstreamer::ElementFactory::make("capsfilter")
            .name(random_string("capsfilter"))
            .build()
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to create capsfilter".to_string())
            })?;

        caps_element.set_property("caps", caps);

        let tee = gstreamer::ElementFactory::make("tee")
            .name(random_string("tee"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("Failed to create tee".to_string()))?;

        let queue_appsink = gstreamer::ElementFactory::make("queue")
            .name(random_string("queue-appsink"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("Failed to create queue".to_string()))?;

        let broadcast_appsink = self.broadcast_appsink(tx, None)?;

        let pipeline = gstreamer::Pipeline::with_name(&random_string("stream-audio-xraw"));

        pipeline
            .add_many([&audio_el, &convert, &caps_element, &tee])
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to add elements to pipeline".to_string())
            })?;

        gstreamer::Element::link_many([&audio_el, &convert, &caps_element, &tee])
            .map_err(|_| GStreamerError::PipelineError("Failed to link elements".to_string()))?;

        pipeline
            .add_many([&queue_appsink, broadcast_appsink.upcast_ref()])
            .map_err(|_| GStreamerError::PipelineError("Failed to add appsink".to_string()))?;
        gstreamer::Element::link_many([&queue_appsink, broadcast_appsink.upcast_ref()])
            .map_err(|_| GStreamerError::PipelineError("Failed to link appsink".to_string()))?;

        let tee_appsink_pad = tee.request_pad_simple("src_%u").ok_or_else(|| {
            GStreamerError::PipelineError("Failed to request tee pad for appsink".into())
        })?;

        let queue_appsink_pad = queue_appsink
            .static_pad("sink")
            .ok_or_else(|| GStreamerError::PipelineError("Appsink queue has no sink pad".into()))?;

        tee_appsink_pad.link(&queue_appsink_pad).map_err(|_| {
            GStreamerError::PipelineError("Failed to link tee to appsink queue".into())
        })?;

        if let Some(ref path) = filename {
            self.add_audio_file_branch(&pipeline, &tee, path)?;
        }

        pipeline
            .iterate_elements()
            .foreach(|e| {
                let _ = e.sync_state_with_parent();
            })
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to sync state with parent".to_string())
            })?;

        Ok(pipeline)
    }

    pub fn supports_video(&self, codec: &str, width: i32, height: i32, framerate: i32) -> bool {
        let caps = self.capabilities();
        if self.device_class == "Audio/Source" {
            return false;
        }
        let caps = caps
            .iter()
            .filter_map(|c| match c {
                MediaCapability::Video(c) => Some(c),
                _ => None,
            })
            .collect::<Vec<_>>();

        caps.iter().any(|c| {
            c.codec == codec
                && c.width == width
                && c.height == height
                && c.framerates.contains(&framerate)
        })
    }

    pub fn supports_audio(&self, codec: &str, channels: i32, framerate: i32) -> bool {
        let caps = self.capabilities();
        if self.device_class == "Video/Source" {
            return false;
        }
        let caps = caps
            .iter()
            .filter_map(|c| match c {
                MediaCapability::Audio(c) => Some(c),
                _ => None,
            })
            .collect::<Vec<_>>();

        caps.iter().any(|c| {
            c.codec == codec
                && c.channels == channels
                && c.framerates.0 <= framerate
                && c.framerates.1 >= framerate
        })
    }

    pub fn supports_screen_share(
        &self,
        codec: &str,
        width: i32,
        height: i32,
        framerate: i32,
    ) -> bool {
        if self.device_class != "Screen/Source" {
            return false;
        }
        let caps = self.capabilities();
        let caps = caps
            .iter()
            .filter_map(|c| match c {
                MediaCapability::Screen(c) => Some(c),
                _ => None,
            })
            .collect::<Vec<_>>();

        caps.iter().any(|c| {
            c.codec == codec
                && c.width >= width
                && c.height >= height
                && c.framerates.contains(&framerate)
        })
    }

    //FixMe: This Pipeline doesn't work for all devices
    fn video_xraw_pipeline(
        &self,
        width: i32,
        height: i32,
        framerate: i32,
        tx: Arc<broadcast::Sender<Arc<Buffer>>>,
        filename: Option<String>,
    ) -> Result<gstreamer::Pipeline, GStreamerError> {
        if filename.is_some() {
            return Err(GStreamerError::PipelineError(
                "Filename not supported for xraw pipeline".to_string(),
            ));
        }

        let input = self.get_video_element()?;
        let caps_element = gstreamer::ElementFactory::make("capsfilter")
            .name(random_string("capsfilter"))
            .build()
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to create capsfilter".to_string())
            })?;
        let caps = gstreamer::Caps::builder("video/x-raw")
            .field("width", width)
            .field("height", height)
            .field("format", VIDEO_FRAME_FORMAT)
            .field("framerate", gstreamer::Fraction::new(framerate, 1))
            .build();
        caps_element.set_property("caps", caps);

        let i420_caps = gstreamer::Caps::builder("video/x-raw")
            .field("format", "I420")
            .build();

        let sink = self.broadcast_appsink(tx, Some(&i420_caps))?;

        let pipeline = gstreamer::Pipeline::with_name(&random_string("stream-xraw"));
        pipeline
            .add_many([&input, &caps_element, sink.upcast_ref()])
            .unwrap();
        gstreamer::Element::link_many([&input, &caps_element, sink.upcast_ref()]).unwrap();

        Ok(pipeline)
    }

    fn video_xh264_pipeline(
        &self,
        width: i32,
        height: i32,
        framerate: i32,
        tx: Arc<broadcast::Sender<Arc<Buffer>>>,
        filename: Option<String>,
    ) -> Result<gstreamer::Pipeline, GStreamerError> {
        if filename.is_some() {
            return Err(GStreamerError::PipelineError(
                "Filename not supported for H264 pipeline".to_string(),
            ));
        }

        let input = self.get_video_element()?;
        let caps_element = gstreamer::ElementFactory::make("capsfilter")
            .name(random_string("capsfilter"))
            .build()
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to create capsfilter".to_string())
            })?;
        let caps = gstreamer::Caps::builder("video/x-h264")
            .field("width", width)
            .field("height", height)
            .field("framerate", gstreamer::Fraction::new(framerate, 1))
            .build();
        caps_element.set_property("caps", caps);

        let h264parse = gstreamer::ElementFactory::make("h264parse")
            .name(random_string("h264parse"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("Failed to create h264parse".to_string()))?;

        let avdec_h264 = gstreamer::ElementFactory::make("avdec_h264")
            .name(random_string("avdec_h264"))
            .build()
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to create avdec_h264".to_string())
            })?;

        let i420_caps = gstreamer::Caps::builder("video/x-raw")
            .field("format", "I420")
            .build();
        let appsink = self.broadcast_appsink(tx, Some(&i420_caps))?;

        let pipeline = gstreamer::Pipeline::with_name(&random_string("stream-h264"));

        pipeline
            .add_many([
                &input,
                &caps_element,
                &h264parse,
                &avdec_h264,
                appsink.upcast_ref(),
            ])
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to add elements to pipeline".to_string())
            })?;

        gstreamer::Element::link_many([
            &input,
            &caps_element,
            &h264parse,
            &avdec_h264,
            appsink.upcast_ref(),
        ])
        .map_err(|_| GStreamerError::PipelineError("Failed to link elements".to_string()))?;

        Ok(pipeline)
    }

    fn image_jpeg_pipeline(
        &self,
        width: i32,
        height: i32,
        framerate: i32,
        tx: Arc<broadcast::Sender<Arc<Buffer>>>,
        filename: Option<String>,
    ) -> Result<gstreamer::Pipeline, GStreamerError> {
        let input = self.get_video_element()?;
        let caps_element = gstreamer::ElementFactory::make("capsfilter")
            .name(random_string("capsfilter"))
            .build()
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to create capsfilter".to_string())
            })?;
        let caps = gstreamer::Caps::builder("image/jpeg")
            .field("width", width)
            .field("height", height)
            .field("framerate", gstreamer::Fraction::new(framerate, 1))
            .build();
        caps_element.set_property("caps", caps);

        let jpegdec = gstreamer::ElementFactory::make("jpegdec")
            .name(random_string("jpegdec"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("Failed to create jpegdec".to_string()))?;

        let convert = gstreamer::ElementFactory::make("videoconvert")
            .name(random_string("videoconvert"))
            .build()
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to create videoconvert".to_string())
            })?;

        let i420_caps = gstreamer::Caps::builder("video/x-raw")
            .field("format", "I420")
            .build();

        let caps_filter = gstreamer::ElementFactory::make("capsfilter")
            .name(random_string("capsfilter"))
            .build()
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to create capsfilter".to_string())
            })?;

        caps_filter.set_property("caps", &i420_caps);

        let tee = gstreamer::ElementFactory::make("tee")
            .name(random_string("tee"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("Failed to create tee".to_string()))?;

        let queue_appsink = gstreamer::ElementFactory::make("queue")
            .name(random_string("queue-appsink"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("Failed to create queue".to_string()))?;

        let appsink = self.broadcast_appsink(tx, Some(&i420_caps))?;

        let pipeline = gstreamer::Pipeline::with_name(&random_string("stream-jpeg"));

        pipeline
            .add_many([
                &input,
                &caps_element,
                &jpegdec,
                &convert,
                &caps_filter,
                &tee,
                &queue_appsink,
                appsink.upcast_ref(),
            ])
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to add elements to pipeline".to_string())
            })?;
        gstreamer::Element::link_many([
            &input,
            &caps_element,
            &jpegdec,
            &convert,
            &caps_filter,
            &tee,
            &queue_appsink,
            appsink.upcast_ref(),
        ])
        .map_err(|_| GStreamerError::PipelineError("Failed to link elements".to_string()))?;

        if let Some(ref path) = filename {
            self.add_video_file_branch(&pipeline, &tee, path)?;
        }

        pipeline
            .iterate_elements()
            .foreach(|e| {
                let _ = e.sync_state_with_parent();
            })
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to sync state with parent".to_string())
            })?;

        Ok(pipeline)
    }

    #[cfg(target_os = "windows")]
    fn get_screen_element(&self) -> Result<gstreamer::Element, GStreamerError> {
        let (_, idx) = get_monitor(&self.device_path)
            .ok_or_else(|| GStreamerError::DeviceError("No screen found".to_string()))?;

        // Try the dxgiscreencapsrc if available
        if gstreamer::ElementFactory::find("dx9screencapsrc").is_some() {
            let element = gstreamer::ElementFactory::make("dx9screencapsrc")
                .name(random_string("screen-source"))
                .build()
                .map_err(|_| {
                    GStreamerError::PipelineError("Failed to create dxgiscreencapsrc".to_string())
                })?;

            element.set_property("monitor", idx);
            element.set_property("cursor", true);

            Ok(element)
        } else {
            Err(GStreamerError::PipelineError(
                "dx9screencapsrc not found".to_string(),
            ))
        }
    }

    #[cfg(target_os = "linux")]
    fn get_screen_element(&self) -> Result<gstreamer::Element, GStreamerError> {
        let monitor = get_monitor(&self.device_path)
            .ok_or_else(|| GStreamerError::DeviceError("No screen found".to_string()))?;

        let element = gstreamer::ElementFactory::make("ximagesrc")
            .name(random_string("screen-source"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("Failed to create ximagesrc".to_string()))?;

        if let Some(MediaCapability::Screen(cap)) = monitor.capabilities.first() {
            element.set_property("use-damage", false);
            element.set_property("show-pointer", true);
            element.set_property("startx", cap.startx as u32);
            element.set_property("starty", cap.starty as u32);
            element.set_property("endx", cap.endx as u32 - 1);
            element.set_property("endy", cap.endy as u32 - 1);
        } else {
            return Err(GStreamerError::PipelineError(
                "No screen capability found".to_string(),
            ));
        }

        Ok(element)
    }

    fn get_video_element(&self) -> Result<gstreamer::Element, GStreamerError> {
        let device = get_gst_device(&self.device_path).unwrap();
        let random_source_name = random_string("source");
        let element = device
            .create_element(Some(random_source_name.as_str()))
            .unwrap();
        Ok(element)
    }

    fn get_audio_element(&self) -> Result<gstreamer::Element, GStreamerError> {
        let device = get_gst_device(&self.device_path).unwrap();
        println!("Device: {:?}", device);
        println!("Device props: {:?}", device.caps());
        let random_source_name = random_string("source");
        let element = device
            .create_element(Some(random_source_name.as_str()))
            .unwrap();
        Ok(element)
    }

    fn broadcast_appsink(
        &self,
        tx: Arc<broadcast::Sender<Arc<Buffer>>>,
        caps: Option<&gstreamer::Caps>,
    ) -> Result<AppSink, GStreamerError> {
        let appsink = gstreamer::ElementFactory::make("appsink")
            .name(random_string("xraw-appsink"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("Failed to create appsink".to_string()))?;
        let appsink = appsink
            .dynamic_cast::<AppSink>()
            .map_err(|_| GStreamerError::PipelineError("Failed to cast appsink".to_string()))?;

        // appsink.set_property("sync", &false);
        appsink.set_property("emit-signals", true);
        appsink.set_property("drop", true);
        appsink.set_property("max-buffers", 1u32);

        appsink.set_callbacks(
            gstreamer_app::AppSinkCallbacks::builder()
                .new_sample(move |sink| {
                    let sample = match sink.pull_sample() {
                        Ok(s) => s,
                        Err(_) => return Err(gstreamer::FlowError::Eos),
                    };

                    let buffer = sample.buffer().ok_or(gstreamer::FlowError::Error)?;

                    if tx.receiver_count() > 0 {
                        let _ = tx.send(Arc::new(buffer.copy()));
                    }
                    Ok(gstreamer::FlowSuccess::Ok)
                })
                .build(),
        );
        if caps.is_some() {
            appsink.set_caps(caps);
        }

        Ok(appsink)
    }

    fn add_video_file_branch(
        &self,
        pipeline: &gstreamer::Pipeline,
        tee: &gstreamer::Element,
        path: &str,
    ) -> Result<(), GStreamerError> {
        let queue_file = gstreamer::ElementFactory::make("queue")
            .name(random_string("file-queue"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("queue".into()))?;

        let convert = gstreamer::ElementFactory::make("videoconvert")
            .name(random_string("file-videoconvert"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("videoconvert".into()))?;

        let format_filter = gstreamer::ElementFactory::make("capsfilter")
            .name(random_string("file-capsfilter"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("capsfilter".into()))?;
        let caps = gstreamer::Caps::builder("video/x-raw")
            .field("format", "I420")
            .build();
        format_filter.set_property("caps", &caps);

        let encoder = gstreamer::ElementFactory::make("x264enc")
            .name(random_string("file-x264enc"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("x264enc".into()))?;
        encoder.set_property("bitrate", 3000u32);
        encoder.set_property_from_str("tune", "zerolatency");

        let parser = gstreamer::ElementFactory::make("h264parse")
            .name(random_string("file-h264parse"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("h264parse".into()))?;

        let muxer = gstreamer::ElementFactory::make("mp4mux")
            .name(random_string("file-mp4mux"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("mp4mux".into()))?;

        let filesink = gstreamer::ElementFactory::make("filesink")
            .name(random_string("file-filesink"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("filesink".into()))?;
        filesink.set_property("location", path);
        filesink.set_property("sync", false);

        pipeline
            .add_many([
                &queue_file,
                &convert,
                &format_filter,
                &encoder,
                &parser,
                &muxer,
                &filesink,
            ])
            .map_err(|_| GStreamerError::PipelineError("Failed to add file branch".into()))?;

        gstreamer::Element::link_many([
            &queue_file,
            &convert,
            &format_filter,
            &encoder,
            &parser,
            &muxer,
            &filesink,
        ])
        .map_err(|_| GStreamerError::PipelineError("Failed to link file branch".into()))?;

        let tee_src_pad = tee
            .request_pad_simple("src_%u")
            .ok_or_else(|| GStreamerError::PipelineError("Failed to request tee pad".into()))?;
        let queue_sink_pad = queue_file
            .static_pad("sink")
            .ok_or_else(|| GStreamerError::PipelineError("Queue has no sink pad".into()))?;

        tee_src_pad.link(&queue_sink_pad).map_err(|_| {
            GStreamerError::PipelineError("Failed to link tee to file branch".into())
        })?;

        Ok(())
    }

    fn add_audio_file_branch(
        &self,
        pipeline: &gstreamer::Pipeline,
        tee: &gstreamer::Element,
        path: &str,
    ) -> Result<(), GStreamerError> {
        let queue_file = gstreamer::ElementFactory::make("queue")
            .name(random_string("file-queue"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("queue".into()))?;

        let convert = gstreamer::ElementFactory::make("audioconvert")
            .name(random_string("file-audioconvert"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("audioconvert".into()))?;

        let resample = gstreamer::ElementFactory::make("audioresample")
            .name(random_string("file-audioresample"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("audioresample".into()))?;

        let encoder = gstreamer::ElementFactory::make("avenc_aac")
            .name(random_string("file-avenc_aac"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("avenc_aac".into()))?;
        encoder.set_property("bitrate", 128000i32);

        let parser = gstreamer::ElementFactory::make("aacparse")
            .name(random_string("file-aacparse"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("aacparse".into()))?;

        let muxer = gstreamer::ElementFactory::make("mp4mux")
            .name(random_string("file-mp4mux"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("mp4mux".into()))?;

        let filesink = gstreamer::ElementFactory::make("filesink")
            .name(random_string("file-filesink"))
            .build()
            .map_err(|_| GStreamerError::PipelineError("filesink".into()))?;
        filesink.set_property("location", path);
        filesink.set_property("sync", false);

        pipeline
            .add_many([
                &queue_file,
                &convert,
                &resample,
                &encoder,
                &parser,
                &muxer,
                &filesink,
            ])
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to ad elements to the file branch".into())
            })?;

        gstreamer::Element::link_many([
            &queue_file,
            &convert,
            &resample,
            &encoder,
            &parser,
            &muxer,
            &filesink,
        ])
        .map_err(|_| {
            GStreamerError::PipelineError("Failed to link elements in file branch".into())
        })?;

        let tee_src_pad = tee
            .request_pad_simple("src_%u")
            .ok_or_else(|| GStreamerError::PipelineError("Failed to request tee pad".into()))?;
        let queue_sink_pad = queue_file
            .static_pad("sink")
            .ok_or_else(|| GStreamerError::PipelineError("Queue has no sink pad".into()))?;

        tee_src_pad.link(&queue_sink_pad).map_err(|_| {
            GStreamerError::PipelineError("Failed to link tee to file branch".into())
        })?;

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoCapability {
    pub width: i32,
    pub height: i32,
    pub framerates: Vec<i32>,
    pub codec: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioCapability {
    pub channels: i32,
    pub framerates: (i32, i32),
    pub codec: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenCapability {
    pub width: i32,
    pub height: i32,
    pub framerates: Vec<i32>,
    pub codec: String,
    pub startx: i32,
    pub starty: i32,
    pub endx: i32,
    pub endy: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaDeviceInfo {
    pub device_path: String,
    pub display_name: String,
    pub capabilities: Vec<MediaCapability>,
    pub device_class: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MediaCapability {
    Video(VideoCapability),
    Audio(AudioCapability),
    Screen(ScreenCapability), // For screen capture capabilities
}

#[derive(Debug, Clone, Error)]
pub enum GStreamerError {
    #[error("Failed to create pipeline: {0}")]
    PipelineError(String),
    #[error("Devices: {0}")]
    DeviceError(String),
}

mod tests {
    #[cfg(test)]
    use super::*;

    #[test]
    fn test_from_path() {
        gstreamer::init().unwrap();
        let path = "/dev/video4";
        let device = GstMediaDevice::from_device_path(path);
        assert!(device.is_ok());
        let device = device.unwrap();
        assert_eq!(device.device_path, path);
    }
}

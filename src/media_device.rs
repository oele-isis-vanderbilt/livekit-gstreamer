use gstreamer::{prelude::*, Buffer};
use gstreamer::{Device, DeviceMonitor};
use gstreamer_app::AppSink;
use once_cell::sync::Lazy;
use std::sync::Arc;
use std::sync::Mutex;
use thiserror::Error;
use tokio::sync::broadcast;

use crate::utils::random_string;

const SUPPORTED_VIDEO_CODECS: [&str; 2] = ["video/x-h264", "image/jpeg"];
const SUPPORTED_AUDIO_CODECS: [&str; 1] = ["audio/x-raw"];
const VIDEO_FRAME_FORMAT: &str = "I420";

static GLOBAL_DEVICE_MONITOR: Lazy<Arc<Mutex<DeviceMonitor>>> = Lazy::new(|| {
    let monitor = DeviceMonitor::new();
    monitor.add_filter(Some("Video/Source"), None);
    monitor.add_filter(Some("Audio/Source"), None);
    if let Err(err) = monitor.start() {
        eprintln!("Failed to start global device monitor: {:?}", err);
    }
    Arc::new(Mutex::new(monitor))
});

pub fn get_gst_device(path: &str) -> Option<Device> {
    let device_monitor = GLOBAL_DEVICE_MONITOR.clone();
    let device_monitor = device_monitor.lock().unwrap();
    let device = device_monitor.devices().into_iter().find(|d| {
        let props = d.properties();

        match props {
            // FixMe: This only works for v4l2 devices
            Some(props) => {
                let path_prop = props.get::<Option<String>>("object.path");
                path_prop
                    .is_ok_and(|path_prop| path_prop.is_some() && path_prop.unwrap().contains(path))
            }
            None => false,
        }
    });

    device
}

/// A struct representing a GStreamer device
/// This implementation assumes that GStreamer is initialized elsewhere
#[derive(Debug, Clone)]
pub struct GstMediaDevice {
    pub display_name: String,
    #[allow(dead_code)]
    pub device_class: String,
    pub device_id: String,
}

pub async fn run_pipeline(
    pipeline: gstreamer::Pipeline,
    tx: broadcast::Sender<()>,
) -> Result<(), GStreamerError> {
    pipeline.set_state(gstreamer::State::Playing).unwrap();
    let bus = pipeline.bus().unwrap();
    for msg in bus.iter_timed(gstreamer::ClockTime::NONE) {
        use gstreamer::MessageView;
        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                eprintln!("Error: {:?}", err.error());
                break;
            }
            MessageView::StateChanged(e) => {
                // Check if we need to stop the pipeline
                if e.current() == gstreamer::State::Null {
                    break;
                }
            }
            _ => (),
        }
    }
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
            device_id: path.into(),
        };
        Ok(device)
    }

    pub fn capabilities(&self) -> Vec<MediaCapability> {
        let device = get_gst_device(&self.device_id).unwrap();

        let caps = device.caps().unwrap();
        if self.device_class == "Video/Source" {
            caps.iter()
                .map(|s| {
                    let structure = s;
                    let width = structure.get::<i32>("width").unwrap();
                    let height = structure.get::<i32>("height").unwrap();
                    let mut framerates = vec![];
                    if let Ok(framerate_fields) = structure.get::<gstreamer::List>("framerate") {
                        let frates: Vec<i32> = framerate_fields
                            .iter()
                            .map(|f| {
                                let f = f.get::<gstreamer::Fraction>();
                                match f {
                                    Ok(f) => f.numer() / f.denom(),
                                    Err(_) => 0,
                                }
                            })
                            .collect();
                        framerates.extend(frates);
                    } else if let Ok(framerate) = structure.get::<gstreamer::Fraction>("framerate")
                    {
                        framerates.push(framerate.numer() / framerate.denom());
                    }

                    let codec = structure.name().to_string();

                    MediaCapability::Video(VideoCapability {
                        width,
                        height,
                        framerates,
                        codec,
                    })
                })
                .collect()
        } else {
            caps.iter()
                .map(|s| {
                    let structure = s;
                    println!("{:?}", structure);
                    let channels = structure.get::<i32>("channels").unwrap();
                    if let Ok(framerate_fields) = structure.get::<gstreamer::IntRange<i32>>("rate")
                    {
                        let codec = structure.name().to_string();

                        MediaCapability::Audio(AudioCapability {
                            channels,
                            framerate: (framerate_fields.min(), framerate_fields.max()),
                            codec,
                        })
                    } else {
                        MediaCapability::Audio(AudioCapability {
                            channels,
                            framerate: (0, 0),
                            codec: "audio/x-raw".to_string(),
                        })
                    }
                })
                .collect()
        }
    }

    pub fn video_pipeline(
        &self,
        codec: &str,
        width: i32,
        height: i32,
        framerate: i32,
        tx: Arc<broadcast::Sender<Arc<Buffer>>>,
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
            return self.video_xraw_pipeline(width, height, framerate, tx);
        } else if codec == "video/x-h264" {
            return self.video_xh264_pipeline(width, height, framerate, tx);
        } else if codec == "image/jpeg" {
            return self.image_jpeg_pipeline(width, height, framerate, tx);
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

        self.audio_xraw_pipeline(channels, framerate, tx)
    }

    fn audio_xraw_pipeline(
        &self,
        channels: i32,
        framerate: i32,
        tx: Arc<broadcast::Sender<Arc<Buffer>>>,
    ) -> Result<gstreamer::Pipeline, GStreamerError> {
        let audio_el = self.get_audio_element()?;

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

        let broadcast_appsink = self.broadcast_appsink(tx, None)?;

        let pipeline = gstreamer::Pipeline::with_name(&random_string("stream-audio-xraw"));

        pipeline
            .add_many([&audio_el, &caps_element, (broadcast_appsink.upcast_ref())])
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to add elements to pipeline".to_string())
            })?;

        gstreamer::Element::link_many([&audio_el, &caps_element, (broadcast_appsink.upcast_ref())])
            .map_err(|_| GStreamerError::PipelineError("Failed to link elements".to_string()))?;

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
                && c.framerate.0 <= framerate
                && c.framerate.1 >= framerate
        })
    }

    //FixMe: This Pipeline doesn't work for all devices
    fn video_xraw_pipeline(
        &self,
        width: i32,
        height: i32,
        framerate: i32,
        tx: Arc<broadcast::Sender<Arc<Buffer>>>,
    ) -> Result<gstreamer::Pipeline, GStreamerError> {
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
    ) -> Result<gstreamer::Pipeline, GStreamerError> {
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

        let i420_caps = gstreamer::Caps::builder("video/x-raw")
            .field("format", "I420")
            .build();

        let appsink = self.broadcast_appsink(tx, Some(&i420_caps))?;

        let pipeline = gstreamer::Pipeline::with_name(&random_string("stream-jpeg"));

        pipeline
            .add_many([&input, &caps_element, &jpegdec, appsink.upcast_ref()])
            .map_err(|_| {
                GStreamerError::PipelineError("Failed to add elements to pipeline".to_string())
            })?;
        gstreamer::Element::link_many([&input, &caps_element, &jpegdec, appsink.upcast_ref()])
            .map_err(|_| GStreamerError::PipelineError("Failed to link elements".to_string()))?;

        Ok(pipeline)
    }

    fn get_video_element(&self) -> Result<gstreamer::Element, GStreamerError> {
        let device = get_gst_device(&self.device_id).unwrap();
        let random_source_name = random_string("source");
        let element = device
            .create_element(Some(random_source_name.as_str()))
            .unwrap();
        Ok(element)
    }

    fn get_audio_element(&self) -> Result<gstreamer::Element, GStreamerError> {
        let device = get_gst_device(&self.device_id).unwrap();
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

        appsink.set_callbacks(
            gstreamer_app::AppSinkCallbacks::builder()
                .new_sample(move |sink| {
                    let sample = match sink.pull_sample() {
                        Ok(sample) => sample,
                        Err(_) => return Err(gstreamer::FlowError::Eos),
                    };

                    // Send the sample to the broadcast channel without awaiting
                    let buffer = sample.buffer().ok_or(gstreamer::FlowError::Error)?;
                    if tx.send(Arc::new(buffer.copy())).is_err() {
                        return Err(gstreamer::FlowError::Error);
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
}

#[derive(Debug, Clone)]
pub struct VideoCapability {
    pub width: i32,
    pub height: i32,
    pub framerates: Vec<i32>,
    pub codec: String,
}

#[derive(Debug, Clone)]
pub struct AudioCapability {
    pub channels: i32,
    pub framerate: (i32, i32),
    pub codec: String,
}

pub enum AudioFormas {
    S16LE,
    S32LE,
}

#[derive(Debug, Clone)]
pub enum MediaCapability {
    Video(VideoCapability),
    Audio(AudioCapability),
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
        println!("Device: {:?}", device);
        assert_eq!(device.device_id, path);
        println!("{:?}", device.capabilities());
    }
}

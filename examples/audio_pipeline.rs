use gstreamer::prelude::*;
use livekit_gstreamer::{GSTMediaDevice, GStreamerError};

fn main() -> Result<(), GStreamerError> {
    gstreamer::init().map_err(|e| {
        GStreamerError::PipelineError(format!("Failed to initialize gstreamer: {}", e))
    })?;

    let device = GSTMediaDevice::from_device_path("hw:2")?;
    println!("Device Capabilities: {:?}", device.capabilities());

    // let audio_el = audio_dev
    //     .create_element(Some("src"))
    //     .map_err(|e| GStreamerError::DeviceError(format!("Failed to create element: {}", e)))?;

    // let audio_convert = gstreamer::ElementFactory::make("audioconvert")
    //     .name("convert")
    //     .build()
    //     .map_err(|e| {
    //         GStreamerError::DeviceError(format!("Failed to create audioconvert element: {}", e))
    //     })?;

    // let audio_sink = gstreamer::ElementFactory::make("autoaudiosink")
    //     .name("sink")
    //     .build()
    //     .map_err(|e| {
    //         GStreamerError::DeviceError(format!("Failed to create autoaudiosink element: {}", e))
    //     })?;

    // let pipeline = gstreamer::Pipeline::with_name("audio-pipeline");

    // pipeline
    //     .add_many(&[&audio_el, &audio_convert, &audio_sink])
    //     .map_err(|e| {
    //         GStreamerError::PipelineError(format!("Failed to add elements to pipeline: {}", e))
    //     })?;
    // gstreamer::Element::link_many(&[&audio_el, &audio_convert, &audio_sink])
    //     .map_err(|e| GStreamerError::PipelineError(format!("Failed to link elements: {}", e)))?;

    // pipeline.set_state(gstreamer::State::Playing).map_err(|e| {
    //     GStreamerError::PipelineError(format!("Failed to set pipeline state to playing: {}", e))
    // })?;

    // let bus = pipeline.bus().unwrap();

    // for msg in bus.iter_timed(gstreamer::ClockTime::NONE) {
    //     match msg.view() {
    //         gstreamer::MessageView::Eos(..) => {
    //             break;
    //         }
    //         gstreamer::MessageView::Error(err) => {
    //             eprintln!(
    //                 "Error from element {}: {}",
    //                 msg.src()
    //                     .map(|s| s.path_string())
    //                     .as_deref()
    //                     .unwrap_or("None"),
    //                 err.error()
    //             );
    //             eprintln!("Debugging information: {:?}", err.debug());
    //             break;
    //         }
    //         _ => (),
    //     }
    // }

    Ok(())
}

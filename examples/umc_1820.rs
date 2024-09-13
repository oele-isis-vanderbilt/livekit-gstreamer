extern crate gstreamer;
use gstreamer::prelude::*;

fn main() {
    // Initialize GStreamer
    gstreamer::init().unwrap();

    // Create the pipeline
    let pipeline = gstreamer::Pipeline::with_name("my-pipeline");

    // Create elements for the pipeline
    let source = gstreamer::ElementFactory::make("alsasrc")
        .name("src")
        .build()
        .unwrap(); // Input source (e.g., ALSA)
    let deinterleave = gstreamer::ElementFactory::make("deinterleave")
        .name("dint")
        .build()
        .unwrap(); // Split channels
    let audio_convert = gstreamer::ElementFactory::make("audioconvert")
        .name("audio")
        .build()
        .unwrap(); // Convert format
    let audio_resample = gstreamer::ElementFactory::make("audioresample")
        .name("res")
        .build()
        .unwrap(); // Resample if needed
    let sink = gstreamer::ElementFactory::make("autoaudiosink")
        .name("abb")
        .build()
        .unwrap(); // Output sink

    // Configure the source to capture audio from the specific device (if needed)
    source.set_property("device", &"hw:4"); // Set device if necessary

    // Add elements to the pipeline
    pipeline
        .add_many(&[
            &source,
            &deinterleave,
            &audio_convert,
            &audio_resample,
            &sink,
        ])
        .unwrap();

    // Link source to deinterleave
    gstreamer::Element::link_many(&[&source, &deinterleave]).unwrap();
    let cloned = audio_convert.clone();

    // Link deinterleave to audio_convert, audio_resample, and sink
    // We only connect the first channel (channel 0) of the interleaved audio
    deinterleave.connect_pad_added(move |deinterleave, src_pad| {
        // Check if the newly added pad is `src_0` (channel 0)
        let pad_name = src_pad.name();
        if pad_name == "src_1" {
            println!("Linking {} to the audio_convert sink pad", pad_name);

            let audio_convert_sink_pad = audio_convert.static_pad("sink").unwrap();
            if audio_convert_sink_pad.is_linked() {
                println!("Already linked, skipping pad {}", pad_name);
                return;
            }

            // Link the channel 0 pad from deinterleave to the audio_convert element
            src_pad.link(&audio_convert_sink_pad).unwrap();
        }
    });

    // Link the remaining elements
    gstreamer::Element::link_many(&[&cloned, &audio_resample, &sink]).unwrap();

    // Start playing
    pipeline.set_state(gstreamer::State::Playing).unwrap();

    // Wait until error or EOS
    let bus = pipeline.bus().unwrap();
    for msg in bus.iter_timed(gstreamer::ClockTime::NONE) {
        match msg.view() {
            gstreamer::MessageView::Eos(..) => break,
            gstreamer::MessageView::Error(err) => {
                eprintln!(
                    "Error received from element {:?}: {}",
                    err.src().map(|s| s.path_string()),
                    err.error()
                );
                break;
            }
            _ => (),
        }
    }

    // Cleanup
    pipeline.set_state(gstreamer::State::Null).unwrap();
}

extern crate gstreamer;

use rust_livekit_streamer::gst_camera_track::{GSTCameraTrack, VideoPreset};

#[tokio::main]
async fn main() {
    gstreamer::init().unwrap();

    let track = GSTCameraTrack::new("/dev/video0", "I420", VideoPreset::H1080p, None);

    track.show();
}

# LIVEKIT-GSTREAMER

Uses gstreamer to stream devices from Rust to Livekit rooms. This crate provides necessary functionalities to streaming audio and video and video from local devices using `gstreamer` and `livekit` client sdks.


## Installation 
This crate is yet to be published to [crates.io](https://crates.io).

```toml
[dependencies]
livekit-gstreamer = { git = "https://github.com/oele-isis-vanderbilt/livekit-gstreamer.git" }
```


## System Dependencies
> [!WARNING]  
> This crate has only been currently tested with gstreamer in Ubuntu 24.

Install [`gstreamer`](https://gstreamer.freedesktop.org/) in your system before using this crate (Ubuntu/Debian instructions below) or use the following [link](https://gstreamer.freedesktop.org/documentation/installing) to install it in your system.

```
$ sudo apt-get install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libgstreamer-plugins-bad1.0-dev gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav gstreamer1.0-tools gstreamer1.0-x gstreamer1.0-alsa gstreamer1.0-gl gstreamer1.0-gtk3 gstreamer1.0-qt5 gstreamer1.0-pulseaudio
```

## Usage
See the [examples directory](./examples/) for detailed usage examples:


1. [`lk_publish_cam_h264.rs`](examples/lk_publish_cam_h264.rs): Captures a camera stream using GStreamer, encodes it in H.264, and publishes it in I420 format to LiveKit. 

2. [`lk_publish_image_jpeg.rs`](examples/lk_publish_image_jpeg.rs): Streams a sequence of JPEG images, converting them to I420 format for publication to LiveKit.

3. [`lk_publish_multitrack.rs`](examples/lk_publish_multitrack.rs): Publishes 2 video tracks to LiveKit, with the final video stream in I420 format and 2 audio tracks from microphones to the livekit room.

4. [`lk_publish_one_minute.rs`](examples/lk_publish_one_minute.rs): Streams video for one minute, converting to I420 format before publishing to LiveKit.

5. [`stream_subscribe_video.rs`](examples/stream_subscribe_video.rs): Subscribes to a local GStreamer media stream in I420 format, do anything with it that you want.

6. [`stream_subscribe_audio.rs`](examples/stream_subscribe_audio.rs): Subscribes to a local GStreamer media audio stream, do anything with it that you want.

7. [`lk_publish_mic.rs`](examples/lk_publish_mic.rs): Streams audio from a local microphone to the livekit room.

8. [`get_devices.rs`](examples/get_devices.rs): Get all the devices, by path and their capabilities to the livekit room.


## Funding Info
This work is supported by the National Science Foundation under Grant No. DRL-2112635.

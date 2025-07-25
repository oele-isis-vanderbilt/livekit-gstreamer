# syncflow-app
This is an attempt to acompany syncflow with a desktop application that can do the following cross platform(Still in intial stages):

1. List Devices (Cameras, Mics and Screens) from rust (Uses gstreamer bindings).
2. Publish a plethora of devices to syncflow sessions (On session start)
3. Record devices locally (No streaming) and upon session completion, upload the recordings to the same s3 bucket as the syncflow session.

There are two crates in this workspace:

1. [`livekit-gstreamer`](./livekit-gstreamer/): Uses gstreamer to publish devices to livekit rooms.
2. [`syncflow-publisher`](./syncflow-publisher/): A tauri app with UI Controls to listen to sessions, select devices etc...

## Roadmap:
1. Registration to SyncFlow as an IOT device.
2. Configuration and Device Discovery.
3. Streaming/Local Recording based on when a session is started.
4. Upload to SyncFlow S3 bucket upon session complete.
5. Cleanup utilities.

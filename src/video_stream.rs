use crate::media_device::{run_pipeline, GSTMediaDevice, GStreamerError};
use gstreamer::{prelude::*, Buffer, Pipeline};
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Debug)]
struct StreamHandle {
    close_tx: broadcast::Sender<()>,
    frame_tx: broadcast::Sender<Arc<Buffer>>,
    task: tokio::task::JoinHandle<Result<(), GStreamerError>>,
    pipeline: Pipeline,
    device: GSTMediaDevice,
}

#[derive(Debug, Clone)]
pub struct VideoPublishOptions {
    pub codec: String,
    pub device_id: String,
    pub width: i32,
    pub height: i32,
    pub framerate: i32,
}

#[derive(Debug)]
pub struct GstVideoStream {
    handle: Option<StreamHandle>,
    publish_options: VideoPublishOptions,
}

impl GstVideoStream {
    pub fn new(publish_options: VideoPublishOptions) -> Self {
        Self {
            handle: None,
            publish_options,
        }
    }

    pub fn has_started(&self) -> bool {
        self.handle.is_some()
    }

    pub async fn stop(&mut self) -> Result<(), GStreamerError> {
        if let Some(handle) = self.handle.take() {
            handle
                .pipeline
                .set_state(gstreamer::State::Null)
                .map_err(|_| GStreamerError::PipelineError("Failed to stop pipeline".into()))?;
            let _ = handle.task.await;
        }
        Ok(())
    }

    pub async fn start(&mut self) -> Result<(), GStreamerError> {
        self.stop().await?;

        let (frame_tx, _) = broadcast::channel::<Arc<Buffer>>(1);
        let (close_tx, _) = broadcast::channel::<()>(1);

        let device = GSTMediaDevice::from_device_path(self.publish_options.device_id.as_str())?;

        let frame_tx_arc = Arc::new(frame_tx.clone());
        let pipeline = device.video_pipeline(
            &self.publish_options.codec,
            self.publish_options.width,
            self.publish_options.height,
            self.publish_options.framerate,
            frame_tx_arc.clone(),
        )?;

        let pipline_task = tokio::spawn(run_pipeline(pipeline.clone(), close_tx.clone()));

        let handle = StreamHandle {
            close_tx,
            frame_tx,
            task: pipline_task,
            pipeline,
            device,
        };
        self.handle = Some(handle);

        Ok(())
    }

    pub fn subscribe(&self) -> Option<(broadcast::Receiver<Arc<Buffer>>, broadcast::Receiver<()>)> {
        self.handle
            .as_ref()
            .map(|h| (h.frame_tx.subscribe(), h.close_tx.subscribe()))
    }

    pub fn details(&self) -> Option<VideoPublishOptions> {
        self.handle.as_ref().map(|_| self.publish_options.clone())
    }

    pub fn get_device_name(&self) -> Option<String> {
        self.handle.as_ref().map(|h| h.device.display_name.clone())
    }
}

impl Drop for GstVideoStream {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle
                .pipeline
                .set_state(gstreamer::State::Null)
                .map_err(|_| GStreamerError::PipelineError("Failed to stop pipeline".into()));
        }
    }
}

use crate::{
    media_device::{run_pipeline, GStreamerError, GstMediaDevice},
    RecordingMetadata,
};
use gstreamer::{prelude::*, Buffer, Pipeline};
use serde::{Deserialize, Serialize};
use std::{
    path::{self, PathBuf},
    sync::Arc,
};
use tokio::{fs, sync::broadcast};

#[derive(Debug)]
struct StreamHandle {
    close_tx: broadcast::Sender<()>,
    frame_tx: broadcast::Sender<Arc<Buffer>>,
    task: tokio::task::JoinHandle<Result<(), GStreamerError>>,
    pipeline: Pipeline,
    device: GstMediaDevice,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalFileSaveOptions {
    pub output_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalSaveFileMetadata {
    pub file_name: String,
    pub codec: String,
    pub started_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoPublishOptions {
    pub codec: String,
    pub device_id: String,
    pub width: i32,
    pub height: i32,
    pub framerate: i32,
    pub local_file_save_options: Option<LocalFileSaveOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioPublishOptions {
    pub codec: String,
    pub device_id: String,
    pub framerate: i32,
    pub channels: i32,
    pub selected_channel: Option<i32>,
    pub local_file_save_options: Option<LocalFileSaveOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PublishOptions {
    Video(VideoPublishOptions),
    Audio(AudioPublishOptions),
}

#[derive(Debug)]
pub struct GstMediaStream {
    handle: Option<StreamHandle>,
    publish_options: PublishOptions,
}

pub async fn create_dir(options: &LocalFileSaveOptions) -> Result<PathBuf, GStreamerError> {
    let output_dir = PathBuf::from(&options.output_dir);
    fs::create_dir_all(&output_dir)
        .await
        .map_err(|e| GStreamerError::PipelineError(format!("Failed to create directory: {}", e)))?;
    Ok(output_dir)
}

impl GstMediaStream {
    pub fn new(publish_options: PublishOptions) -> Self {
        Self {
            handle: None,
            publish_options,
        }
    }

    pub fn has_started(&self) -> bool {
        self.handle.is_some()
    }

    pub fn kind(&self) -> &str {
        match &self.publish_options {
            PublishOptions::Video(_) => "Video",
            PublishOptions::Audio(_) => "Audio",
        }
    }

    pub async fn stop(&mut self) -> Result<(), GStreamerError> {
        if let Some(handle) = self.handle.take() {
            handle.pipeline.send_event(gstreamer::event::Eos::new());
            let _ = handle.task.await;
        }
        self.handle = None;
        Ok(())
    }

    pub async fn start(&mut self) -> Result<(), GStreamerError> {
        self.stop().await?;

        let (frame_tx, _) = broadcast::channel::<Arc<Buffer>>(1);
        let (close_tx, _) = broadcast::channel::<()>(1);

        let device = match &self.publish_options {
            PublishOptions::Video(video_options) => {
                GstMediaDevice::from_device_path(video_options.device_id.as_str())?
            }
            PublishOptions::Audio(audio_options) => {
                GstMediaDevice::from_device_path(audio_options.device_id.as_str())?
            }
        };

        let frame_tx_arc = Arc::new(frame_tx.clone());
        let mut metadata = None;

        let pipeline = match &self.publish_options {
            PublishOptions::Video(video_options) => {
                let mut filename = None;
                if let Some(local_file_save_options) = &video_options.local_file_save_options {
                    let op_dir = create_dir(local_file_save_options).await?;
                    let filename_str = format!(
                        "{}-{}-{}-{}.mp4",
                        "video",
                        device.display_name.replace(" ", "_"),
                        video_options.device_id.replace(" ", "_").replace("/", "_"),
                        chrono::Local::now().format("%Y-%m-%d-%H-%M-%S")
                    );

                    metadata = Some(RecordingMetadata::new(
                        filename_str.clone(),
                        path::absolute(&op_dir)
                            .unwrap()
                            .to_string_lossy()
                            .to_string(),
                        "camera".into(),
                        "video".into(),
                        video_options.codec.clone(),
                        None, // No audio channel for video
                    ));

                    filename = Some(op_dir.join(filename_str).to_string_lossy().to_string());
                }
                device.video_pipeline(
                    &video_options.codec,
                    video_options.width,
                    video_options.height,
                    video_options.framerate,
                    frame_tx_arc.clone(),
                    filename,
                )?
            }
            PublishOptions::Audio(audio_options) => {
                let mut filename = None;
                if let Some(local_file_save_options) = &audio_options.local_file_save_options {
                    let op_dir = create_dir(local_file_save_options).await?;
                    let filename_str = format!(
                        "{}-{}-{}-{}-{}.m4a",
                        "audio",
                        match audio_options.selected_channel {
                            Some(channel) => format!(
                                "{}-channel-{}",
                                device.display_name.replace(" ", "_"),
                                channel
                            ),
                            None => device.display_name.replace(" ", "_"),
                        },
                        audio_options.device_id.replace(" ", "_"),
                        audio_options.device_id.replace(" ", "_").replace("/", "_"),
                        chrono::Local::now().format("%Y-%m-%d-%H-%M-%S")
                    );

                    metadata = Some(RecordingMetadata::new(
                        filename_str.clone(),
                        path::absolute(&op_dir)
                            .unwrap()
                            .to_string_lossy()
                            .to_string(),
                        "microphone".into(),
                        "audio".into(),
                        audio_options.codec.clone(),
                        audio_options.selected_channel.clone(),
                    ));

                    filename = Some(op_dir.join(filename_str).to_string_lossy().to_string());
                }
                match audio_options.selected_channel {
                    Some(selected_channel) => device.deinterleaved_audio_pipeline(
                        &audio_options.codec,
                        audio_options.channels,
                        selected_channel,
                        audio_options.framerate,
                        frame_tx_arc.clone(),
                    )?,
                    None => device.audio_pipeline(
                        &audio_options.codec,
                        audio_options.channels,
                        audio_options.framerate,
                        frame_tx_arc.clone(),
                        filename,
                    )?,
                }
            }
        };

        let pipline_task = tokio::spawn(run_pipeline(
            pipeline.clone(),
            close_tx.clone(),
            metadata.clone(),
        ));

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

    pub fn details(&self) -> Option<PublishOptions> {
        self.handle.as_ref().map(|_| self.publish_options.clone())
    }

    pub fn get_device_name(&self) -> Option<String> {
        self.handle.as_ref().map(|h| h.device.display_name.clone())
    }
}

impl Drop for GstMediaStream {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle
                .pipeline
                .set_state(gstreamer::State::Null)
                .map_err(|_| GStreamerError::PipelineError("Failed to stop pipeline".into()));
        }
    }
}

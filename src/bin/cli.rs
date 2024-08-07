use clap::{Parser, Subcommand};
use rust_livekit_streamer::devices::list_video_devices;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct LKRustStreamerCLI {
    #[clap(subcommand)]
    subcmd: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    ListDevices(ListDevices),
}

#[derive(Parser)]
pub struct ListDevices {
    #[clap(short, long, default_value_t = true)]
    pub audio: bool,
    #[clap(short, long, default_value_t = true)]
    pub video: bool,
    #[clap(short, long, default_value = "all")]
    pub device_id: String,

    #[clap(short, long, default_value_t = true)]
    pub input_only: bool,
}

fn main() {
    let args = LKRustStreamerCLI::parse();
    match args.subcmd {
        Commands::ListDevices(list_devices) => {
            if list_devices.audio {
                let devices = list_video_devices();
                println!("Audio Devices: {:?}", devices);
            }
        }
    }
}

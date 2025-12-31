use std::num::NonZeroUsize;
use std::path::PathBuf;
use clap::Parser;
use crate::model::VersionIdentifier;

#[derive(Parser)]
pub struct Args {
    #[clap(long)]
    pub version: VersionIdentifier,
    #[clap(short = 'j', long)]
    pub threads: Option<NonZeroUsize>,
    #[clap(short = 'd', long = "minecraft-directory")]
    pub dot_minecraft: PathBuf,
    #[clap(short, long)]
    pub re_download: bool,
    #[clap(long)]
    pub unsafe_danger_skip_validation_hash_and_size: bool,
}

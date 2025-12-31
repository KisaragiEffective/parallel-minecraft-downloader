#![deny(clippy::all)]
#![warn(clippy::nursery)]

mod model;
mod args;
mod console_message;
mod hash;

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;
use base64::alphabet::STANDARD;
use base64::engine::GeneralPurposeConfig;
use clap::Parser;
use rayon::iter::{IntoParallelIterator, ParallelIterator, IndexedParallelIterator};
use reqwest::blocking::Client;
use reqwest::Url;
use reqwest::tls::Version;

use sha1_smol::Sha1;
use model::{AssetMappingRoot, DetailedVersionMetadata, PartialVersionManifestRoot};
use crate::model::AssetMappingValue;
use crate::args::Args;
use crate::console_message::{ConsoleMessage, CorruptedMetadataAction, State};
use crate::hash::Sha1Hash;

fn create_necessary_folders(base: &Path) {
    let asset_dir = base.join("assets");
    if !asset_dir.exists() {
        std::fs::create_dir(&asset_dir).expect("auto-mkdir of asset dir failed");

        let objects_dir = asset_dir.join("objects");

        if !objects_dir.exists() {
            std::fs::create_dir(&objects_dir).expect("auto-mkdir of asset/objects dir failed");
        }

        for hash_prefix_value in 0..256 {
            let hash_prefix = format!("{hash_prefix_value:02x}");
            let hash_prefix_dir = objects_dir.join(hash_prefix);

            if !hash_prefix_dir.exists() {
                std::fs::create_dir(&hash_prefix_dir).expect("auto-mkdir of asset/object/__ dir failed");
            }
        }
    }
}

const APPLICATION_JSON: &str = "application/json";

fn print_console_message(assets_num: usize, console_message: ConsoleMessage) {
    let ConsoleMessage { item_index, hash, state } = console_message;

    let s: Box<str> = match state {
        State::Checking => "checking".into(),
        State::Cached => "cached; skipping".into(),
        State::Processing => "writing".into(),
        State::Done => "done".into(),
        State::CorruptedMetadata(CorruptedMetadataAction::Skipped { actual_hash }) => format!("hash mismatch. actual hash is {actual_hash}; skipping").into_boxed_str(),
        State::CorruptedMetadata(CorruptedMetadataAction::ForciblyContinued) => "actual size or hash did not match, but it will be SAVED!! Please be aware that this feature is NOT SUPPORTED. USE AT YOUR OWN PERIL.".into()
    };

    eprintln!("[{item_index:04}/{assets_num:04}] {h}: {s}", h = hash.human_readable());
}

fn main() {
    let args = Args::parse();
    if !args.dot_minecraft.exists() || !args.dot_minecraft.is_dir() {
        eprintln!("--minecraft-directory must point existing directory");
        exit(1);
    }

    // prepare
    create_necessary_folders(&args.dot_minecraft);

    let requested_version = &args.version;

    // TLS 1.2未満は安全ではないので禁止
    let client = reqwest::blocking::ClientBuilder::new().https_only(true).min_tls_version(Version::TLS_1_2)
        .tcp_keepalive(Duration::from_secs(300))
        .build()
        .expect("failed to initialize HTTP client");

    let version_manifest = client.get("https://launchermeta.mojang.com/mc/game/version_manifest.json")
        .header("Accept", APPLICATION_JSON)
        .send().expect("failed to list up versions")
        .json::<PartialVersionManifestRoot>().expect("invalid or non-conformed JSON was returned")
        .versions.into_iter().find(|x| &x.id == requested_version);

    let Some(version_manifest) = version_manifest else {
        eprintln!("version {requested_version} could not be found in the remote.", requested_version = &requested_version.0);
        exit(1)
    };

    println!("downloading detailed version metadata for {requested_version} = {url}", requested_version = &requested_version.0, url = &version_manifest.url);

    let url_to_asset_index = client.get(version_manifest.url)
        .header("Accept", APPLICATION_JSON)
        .send().expect("failed to fetch version metadata")
        .json::<DetailedVersionMetadata>().expect("invalid or non-conformed JSON was returned")
        .asset_index.url;

    println!("downloading asset list for {requested_version} = {url}", requested_version = &requested_version.0, url = &url_to_asset_index);

    let assets = client.get(url_to_asset_index)
        .header("Accept", APPLICATION_JSON)
        .send().expect("failed to fetch asset list")
        .json::<AssetMappingRoot>().expect("invalid or non-conformed JSON was returned").objects;

    let assets_num = assets.len();

    if let Some(threads) = args.threads {
        rayon::ThreadPoolBuilder::new().num_threads(threads.get()).build_global().unwrap();
    }

    let (sender, receiver) = std::sync::mpsc::channel::<ConsoleMessage>();
    let (please_exit, exit_rx) = std::sync::mpsc::channel();

    let console_writer = std::thread::spawn(move || {
        while matches!(exit_rx.recv_timeout(Duration::ZERO), Err(RecvTimeoutError::Timeout)) {
            match receiver.recv_timeout(Duration::ZERO) {
                Ok(console_message) => print_console_message(assets_num, console_message),
                Err(_) => {
                    std::thread::yield_now();
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }

        while let Ok(console_message) = receiver.recv_timeout(Duration::ZERO) {
            print_console_message(assets_num, console_message);
        }
    });

    // Base64エンジンを作るのはゼロコストではないので使いまわす
    let base64_engine = base64::engine::GeneralPurpose::new(&STANDARD, GeneralPurposeConfig::new());
    // HTTPクライアントを作るのは少なくともゼロコストではないので使いまわす
    let Args { dot_minecraft, re_download, unsafe_danger_skip_validation_hash_and_size, .. } = args;
    assets
        .into_values().collect::<Vec<_>>()
        .into_par_iter()
        .enumerate()
        .for_each(|(i, k)| process(k, &client, &base64_engine, &sender, i + 1, &dot_minecraft, unsafe_danger_skip_validation_hash_and_size, re_download));

    please_exit.send(()).expect("error send");
    console_writer.join().expect("error join");
}

fn process<BE: base64::Engine>(
    meta: AssetMappingValue,
    client: &Client,
    base64: &BE,
    sender: &std::sync::mpsc::Sender<ConsoleMessage>,
    item_index: usize,
    dot_minecraft: &Path,
    unsafe_danger_skip_validation_hash_and_size: bool,
    force: bool,
) {
    let size = meta.size;
    let hash = meta.hash;

    sender.send(ConsoleMessage {
        item_index,
        hash,
        state: State::Checking,
    }).unwrap_or_default();

    let (url, path) = create_channel(&hash, dot_minecraft);

    let is_cached = || {
        if force { return false }
        let Ok(res) = client.head(url.clone()).send() else { return false };
        let headers = res.headers();
        let Some(azure_md5_header) = headers.get("content-md5") else { return false };
        let Ok(fd) = File::open(&path) else { return false };
        let mut br = BufReader::with_capacity(size, fd);
        let mut buf = vec![];
        let Ok(read_size) = br.read_to_end(&mut buf) else { return false };
        if read_size != size { return false }
        let actual = md5::compute(&buf).0;
        let base64_engine = base64;
        let Ok(decoded_azure_md5_header) = base64_engine.decode(azure_md5_header.as_bytes()) else {
            return false
        };

        if decoded_azure_md5_header == actual {
            return true
        }

        false
    };

    if is_cached() {
        sender.send(ConsoleMessage {
            item_index,
            hash,
            state: State::Cached,
        }).unwrap_or_default();

        return
    }

    let bytes = client.get(url).send().expect("failed to get")
        .bytes().expect("failed to read response");

    let actual_hash = sha1_hash(&bytes);
    if unsafe_danger_skip_validation_hash_and_size || bytes.len() == size && actual_hash == hash {
        if unsafe_danger_skip_validation_hash_and_size {
            sender.send(ConsoleMessage {
                item_index,
                hash,
                state: State::CorruptedMetadata(CorruptedMetadataAction::ForciblyContinued),
            }).unwrap_or_default();
        }

        sender.send(ConsoleMessage {
            item_index,
            hash,
            state: State::Processing,
        }).unwrap_or_default();

        let f = match File::options().create(true).truncate(true).write(true).open(&path) {
            Ok(f) => f,
            Err(e) => panic!("failed to open {path:?}: {e:?}")
        };
        let mut buf_writer = BufWriter::with_capacity(size, f);
        buf_writer.write_all(&bytes).expect("failed to write to file");

        sender.send(ConsoleMessage {
            item_index,
            hash,
            state: State::Done,
        }).unwrap_or_default();
    } else {
        sender.send(ConsoleMessage {
            item_index,
            hash,
            state: State::CorruptedMetadata(CorruptedMetadataAction::Skipped { actual_hash })
        }).unwrap_or_default()
    }
}

fn create_asset_url(hash: &Sha1Hash) -> Url {
    let hash2 = hash.human_readable();
    let head = unsafe { std::str::from_utf8_unchecked(hash2.as_bytes().get_unchecked(0..2)) };
    let hash = hash.human_readable();

    const M: &str = "https://resources.download.minecraft.net/";
    let mut raw = String::with_capacity(M.len() + 1 + head.len() + 1 + hash.len());
    raw += M;
    raw += "/";
    raw += head;
    raw += "/";
    raw += &hash;

    // this never panics.
    Url::parse(&raw).unwrap()
}

fn create_channel(hash: &Sha1Hash, base_dir: &Path) -> (Url, PathBuf) {
    // this never panics.
    let url = create_asset_url(hash);
    let hash2 = hash.human_readable();
    let head = unsafe { std::str::from_utf8_unchecked(hash2.as_bytes().get_unchecked(0..2)) };
    let hash = hash.human_readable();
    let hash = &hash;

    let path = base_dir.join("assets").join("objects").join(head).join(hash);

    (url, path)
}

fn sha1_hash(bytes: &[u8]) -> Sha1Hash {
    Sha1Hash(Sha1::from(bytes).digest())
}

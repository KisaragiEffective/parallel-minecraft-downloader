#![deny(clippy::all)]
#![warn(clippy::nursery)]

mod model;

use std::fs::File;
use std::hash::{BuildHasher, Hasher};
use std::io::{BufReader, BufWriter, Read, Write};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;
use base64::alphabet::STANDARD;
use base64::engine::GeneralPurposeConfig;
use clap::Parser;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reqwest::blocking::Client;
use reqwest::Url;
use reqwest::tls::Version;

use sha1_smol::Sha1;
use model::{AssetMappingRoot, DetailedVersionMetadata, PartialVersionManifestRoot};
use crate::model::{AssetMappingValue, Sha1Hash, VersionIdentifier};

#[derive(Parser)]
struct Args {
    #[clap(long)]
    version: VersionIdentifier,
    #[clap(short = 'j', long)]
    threads: Option<NonZeroUsize>,
    #[clap(short = 'd', long = "minecraft-directory")]
    dot_minecraft: PathBuf,
    #[clap(short, long)]
    re_download: bool,
    #[clap(long)]
    unsafe_danger_skip_validation_hash_and_size: bool,
}

pub struct JunkHasher {
    x: u64
}

impl Hasher for JunkHasher {
    fn finish(&self) -> u64 {
        self.x
    }

    fn write(&mut self, bytes: &[u8]) {
        // we can simply pass, with some trick; we know that there are "paths", so removing file extension may decrease conflict.
        let end = bytes.iter().enumerate().rfind(|(_, x)| **x == b'.').map_or(bytes.len() - 1, |(pos, _)| pos - 1);
        let bytes = if end >= 8 {
            &bytes[(end - 8)..end]
        } else {
            bytes
        };
        for i in 1..=8 {
            self.x |= bytes.get(i - 1).map_or(0, |x| (*x as u64) << ((8 - i) * 8))
        }

        self.x = self.x.wrapping_mul(65535);
        // haha that's just random
        self.x = self.x.wrapping_add(0x231a_0edc_d310_77a2);
    }
}

impl BuildHasher for JunkHasher {
    type Hasher = Self;

    fn build_hasher(&self) -> Self::Hasher {
        Self { x: 0xc71b_58d7_e3ff_62ab }
    }
}

impl Default for JunkHasher {
    fn default() -> Self {
        Self { x: 0xc71b_58d7_e3ff_62ab }
    }
}

fn main() {
    let args = Args::parse();
    if !args.dot_minecraft.exists() || !args.dot_minecraft.is_dir() {
        eprintln!("--minecraft-directory must point existing directory");
        exit(1);
    }

    // prepare

    {
        let r = args.dot_minecraft.join("assets");
        if !r.exists() {
            std::fs::create_dir(&r).expect("auto-mkdir of asset dir failed");

            let o = r.join("objects");

            if !o.exists() {
                std::fs::create_dir(&o).expect("auto-mkdir of asset/objects dir failed");

                for i in 0..256 {
                    let s = format!("{i:02x}");
                    let p = o.join(s);

                    if !p.exists() {
                        std::fs::create_dir(&p).expect("auto-mkdir of asset/object/__ dir failed");
                    }
                }
            }
        }
    }
    let requested_version = &args.version;

    // TLS 1.2未満は安全ではないので禁止
    let client = reqwest::blocking::ClientBuilder::new().https_only(true).min_tls_version(Version::TLS_1_2)
        .tcp_keepalive(Duration::from_secs(300))
        .build()
        .expect("failed to initialize HTTP client");

    let version_manifest = client.get("https://launchermeta.mojang.com/mc/game/version_manifest.json")
        .header("Accept", "application/json")
        .send().expect("failed to list up versions")
        .json::<PartialVersionManifestRoot>().expect("invalid or non-conformed JSON was returned")
        .versions.into_iter().find(|x| &x.id == requested_version);

    let Some(version_manifest) = version_manifest else {
        eprintln!("version {requested_version} could not be found in the remote.", requested_version = &requested_version.0);
        exit(1)
    };

    println!("downloading detailed version metadata for {requested_version} = {url}", requested_version = &requested_version.0, url = &version_manifest.url);

    let url_to_asset_index = client.get(version_manifest.url)
        .header("Accept", "application/json")
        .send().expect("failed to fetch version metadata")
        .json::<DetailedVersionMetadata>().expect("invalid or non-conformed JSON was returned")
        .asset_index.url;

    println!("downloading asset list for {requested_version} = {url}", requested_version = &requested_version.0, url = &url_to_asset_index);

    let assets = client.get(url_to_asset_index)
        .header("Accept", "application/json")
        .send().expect("failed to fetch asset list")
        .json::<AssetMappingRoot>().expect("invalid or non-conformed JSON was returned").objects;

    if let Some(threads) = args.threads {
        rayon::ThreadPoolBuilder::new().num_threads(threads.get()).build_global().unwrap();
    }

    let force = args.re_download;
    let (sender, receiver) = std::sync::mpsc::channel::<String>();
    let (please_exit, exit_rx) = std::sync::mpsc::channel();

    let console_writer = std::thread::spawn(move || {
        {
            let exit_rx = exit_rx;
            let run = || {
                match exit_rx.recv_timeout(Duration::ZERO) { Ok(()) => false, Err(e) => match e {
                    RecvTimeoutError::Timeout => true,
                    RecvTimeoutError::Disconnected => false,
                } }
            };

            // println!("{}", run());
            while run() {
                // eprintln!("status emit 1");
                match receiver.recv_timeout(Duration::ZERO) {
                    Ok(v) => eprintln!("{v}"),
                    Err(_) => {
                        std::thread::yield_now();
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
            }

            // eprintln!("exit asked");

            while let Ok(r) = receiver.recv_timeout(Duration::ZERO) {
                eprintln!("{r}");
            }
        }
    });

    // Base64エンジンを作るのはゼロコストではないので使いまわす
    let base64_engine = base64::engine::GeneralPurpose::new(&STANDARD, GeneralPurposeConfig::new());
    // HTTPクライアントを作るのは少なくともゼロコストではないので使いまわす
    assets.into_par_iter().for_each(|kv| process(kv.1, &args, &client, force, &sender, &base64_engine));
    // eprintln!("ended!!");
    please_exit.send(()).expect("error send");
    // eprintln!("signal sended, waiting join");
    console_writer.join().expect("error join");
    // eprintln!("joined");
}

fn process<BE: base64::Engine>(
    meta: AssetMappingValue,
    args: &Args,
    client: &Client,
    force: bool,
    sender: &std::sync::mpsc::Sender<String>,
    base64: &BE
) {
    let size = meta.size;
    let hash = &meta.hash;
    {
        sender.send(format!("{hash}: processing")).unwrap_or_default();
    }

    let (url, path) = create_channel(hash, &args.dot_minecraft);

    'download: {
        'check_cached: {
            if force { break 'check_cached }
            // make sure that
            let Ok(res) = client.head(url.clone()).send() else { break 'check_cached };
            let headers = res.headers();
            let Some(azure_md5_header) = headers.get("content-md5") else { break 'check_cached };
            let Ok(fd) = File::open(&path) else { break 'check_cached };
            let mut br = BufReader::with_capacity(size, fd);
            let mut buf = vec![];
            let Ok(read_size) = br.read_to_end(&mut buf) else { break 'check_cached };
            if read_size != size { break 'check_cached }
            let actual = md5::compute(&buf).0;
            let base64_engine = base64;
            let Ok(decoded_azure_md5_header) = base64_engine.decode(azure_md5_header.as_bytes()) else {
                break 'check_cached
            };

            if decoded_azure_md5_header == actual {
                sender.send(format!("{hash}: cached; skipping")).unwrap_or_default();
                break 'download
            }
        }

        let bytes = client.get(url).send().expect("failed to get")
            .bytes().expect("failed to read response");

        let actual_hash = sha1_hash(&bytes);
        if args.unsafe_danger_skip_validation_hash_and_size || bytes.len() == size && &actual_hash == hash {
            if args.unsafe_danger_skip_validation_hash_and_size {
                sender.send(format!(
                "{hash}: actual size or hash did not match, but it will be SAVED!! Please be aware that this feature is NOT SUPPORTED. USE AT YOUR OWN PERIL."
                )).unwrap_or_default();
            }
            sender.send(format!("try: {:?}", &path)).unwrap_or_default();
            let f = match File::options().create(true).truncate(true).write(true).open(&path) {
                Ok(f) => f,
                Err(e) => panic!("failed to open {path:?}: {e:?}")
            };
            let mut buf_writer = BufWriter::with_capacity(size, f);
            buf_writer.write_all(&bytes).expect("failed to write to file");
        } else {
            sender.send(
                format!("{hash}: hash mismatch. actual hash is {actual_hash}; skipping")
            ).unwrap_or_default();
        }
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

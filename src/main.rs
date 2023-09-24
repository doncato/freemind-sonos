mod speaker;
pub use crate::speaker::box_controller::SpeakerBox;

mod freemind;
pub use crate::freemind::freemind_handler::FreemindConfig;

mod content;
pub use crate::content::speech::{get_date, get_speech_voicerss};
pub use crate::content::music::{JellyfinConfig, get_random_jellyfin_track};

use freemind::freemind_handler::FreemindState;
use sonor::{args, rupnp::ssdp::URN, Speaker};
use clap::{Arg, Command};
use confy;
use env_logger::{self, Builder};
use log::LevelFilter;
use pnet::datalink::interfaces;
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::time::sleep_until;
use std::fmt;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::fs::File;
use std::io::Write;
use tokio::time::{Duration, Instant};
use tokio;


const AV_TRANSPORT: &URN = &URN::service("schemas-upnp-org", "AVTransport", 1);

#[derive(Serialize, Deserialize)]
struct Config {
    username: String,
    timezone: i8,
    local_server: String,
    path: PathBuf,
    tts_api_key: String,
    freemind: FreemindConfig,
    jellyfin: JellyfinConfig,
    speaker: SpeakerBox,
}
impl ::std::default::Default for Config {
    fn default() -> Self {
        Self {
            username: "doncato".to_string(),
            timezone: 2,
            local_server: "http://192.168.0.1/media".to_string(),
            path: PathBuf::from("./media"),
            tts_api_key: "YOUR TTS API KEY".to_string(),
            freemind: FreemindConfig::default(),
            jellyfin: JellyfinConfig::default(),
            speaker: SpeakerBox::default(),
        }
    }
}
impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", serde_json::to_string(self).unwrap())
    }
}
impl Config {
    /// Iterates over every speaker configuration in the config and converts them
    /// into a Vector of sonor::Speaker objects.
    async fn to_speaker(&self) -> Option<Speaker> {
        let mut result = None;
        log::debug!("Connecting to {} . . .", self.speaker.ip());
            if let Some(spk) = self.speaker.to_speaker().await {
                result = Some(spk);
                log::debug!("Successfully connected to {}.", self.speaker.ip());
            } else {
                log::debug!("Ignoring {}: Connection failed.", self.speaker.ip());
            }
        result
    }
}

#[derive(Debug, Clone)]
struct AppState {
    username: String,
    server: String,
    path: PathBuf,
    spk: Speaker,
    tts_api_key: String,
    fmstate: FreemindState,
    jellyfin: JellyfinConfig,
}
impl AppState {
    fn new(username: String, server: String, path: PathBuf, spk: Speaker, tts_api_key: String, fmconf: FreemindConfig, jellyfin: JellyfinConfig) -> Self {
        Self {
            username,
            server,
            path,
            spk,
            tts_api_key,
            fmstate: FreemindState::new(fmconf),
            jellyfin,
        }
    }

    async fn fetch_tts_and_save(&self, txt: String) -> Result<(), reqwest::Error> {
        let result = get_speech_voicerss(&txt, &self.tts_api_key).await?;

        let mut f = File::create(&self.path.join("tts.mp3")).unwrap();
        f.write_all(&result).unwrap_or(());

        Ok(())
    }

    async fn play(&self) {
        self.spk.play().await.unwrap();
    }

    async fn play_uri(&self, uri: String, play: bool) {
        let result = self.spk.action(
            AV_TRANSPORT,
            "SetAVTransportURI",
            args! {"InstanceID": "0", "CurrentURI": uri.as_str(), "CurrentURIMetaData": ""},
        ).await;

        if play && result.is_ok() && (!self.spk.is_playing().await.unwrap_or(false)) {
            self.spk.play().await.unwrap()
        }
    }

    async fn fade_in(&self) {
        let current_volume = self.spk.volume().await.unwrap();
        self.spk.set_volume(0).await.unwrap();
        while self.spk.set_volume_relative(3).await.unwrap_or(current_volume) < current_volume-3 {
            sleep_until(Instant::now() + Duration::from_millis(500)).await;
        }
        self.spk.set_volume(current_volume).await.unwrap();
    }

    async fn fade_out(&self) {
        let current_volume = self.spk.volume().await.unwrap();
        while self.spk.set_volume_relative(-3).await.unwrap_or(1) > 3 {
            sleep_until(Instant::now() + Duration::from_millis(500)).await;
        }
        self.spk.pause().await.unwrap();
        self.spk.set_volume(current_volume).await.unwrap();
    }

    async fn skip_to(&self, seconds: u32) {
        self.spk.skip_to(seconds).await.unwrap();
    }

    async fn skip_by(&self, seconds: i32) {
        self.spk.skip_by(seconds).await.unwrap();
    }

    async fn wait_for_end(&self) {
        while self.spk.is_playing().await.unwrap() {
            sleep_until(Instant::now() + Duration::from_millis(500)).await;
        }
    }

    async fn play_file(&self, file: String) {
        let uri = format!("{}{}", self.server, file).replace(" ", "%20");
        let result = self.spk.action(
            AV_TRANSPORT,
            "SetAVTransportURI",
            args! {"InstanceID": "0", "CurrentURI": uri.as_str(), "CurrentURIMetaData": ""},
        ).await;

        if result.is_ok() && (!self.spk.is_playing().await.unwrap_or(false)) {
            self.spk.play().await.unwrap()
        }
    }
}

async fn init<'a>(log_level: LevelFilter) -> AppState {
    Builder::new().filter(None, log_level).init();
    log::info!("Initializing . . .");

    log::debug!("Loading Config . . .");
    let cfg: Config = confy::load_path("./FreemindSonos.config").expect(
        "Failed to start because the config file could not be created or could not be read!",
    );
    let path = cfg.path.clone().into_boxed_path();
    if !path.exists() {
        panic!("Provided path in the config does not exist!")
    } else if !path.is_dir() {
        panic!("Provided path in the config is not an directory!")
    }
    log::debug!("Getting IP Addresses of the machine");
    let mut addrs: Vec<Ipv4Addr> = Vec::new();
    for iface in interfaces()
        .iter()
        .filter(|e| e.is_up() && !e.is_loopback() && !e.ips.is_empty())
    {
        let mut ips: Vec<Ipv4Addr> = Vec::new();
        for ip in iface.ips.iter() {
            if let ipnetwork::IpNetwork::V4(addr) = ip {
                ips.push(addr.ip())
            }
        }
        addrs.append(&mut ips)
    }
    if addrs.is_empty() {
        panic!("This machine does not have any IPv4 Address. Please make sure that all desired network-interfaces are connected to a network, have a valid IPv4 address and are accessible by this program");
    } else {
        log::info!("Found {} IP addresses", addrs.len());
        log::debug!("These IP addresses were found:\n{:#?}", addrs);
    }

    log::debug!("Trying to connect to configured speaker . . .");
    AppState::new(
        cfg.username.clone(),
        cfg.local_server.clone(),
        path.to_path_buf(),
        cfg.to_speaker().await.unwrap(),
        cfg.tts_api_key,
        cfg.freemind,
        cfg.jellyfin,
    )
}

#[tokio::main]
async fn main() {
    let args = Command::new("Sonos Controller")
        .version("0.1.0")
        .author("doncato, https://github.com/doncato")
        .about("Control one Sonos Speaker")
        .arg(
            Arg::new("debug")
                .long("debug")
                .help("Change log level to debug"),
        )
        .get_matches();

    let llvl = if args.is_present("debug") {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };
    let mut op: AppState = init(llvl).await;
    log::info!("Initialized.");
    log::debug!("Connected to {:#?} Speaker", op.spk);

    let track = get_random_jellyfin_track(&op.jellyfin).await.unwrap();//_or(None)

    if let Some(title) = &track {
        op.play_uri(format!("https://venture.zossennews.de/media/Audio/{}/stream.mp3", title.id).to_string(), true).await;
    };

    op.fmstate.fetch().await.unwrap();

    let elements = op.fmstate.get_today();
    let count = elements.len();

    let mut i: u8 = 0;
    let mut event_list = String::new();
    elements.iter().for_each(|e| {
        i+=1;
        event_list.push_str(format!("Number {}: {}.\n ", i, e.description()).as_str());
        if e.location() != "" || e.timedelta().is_some() {
            event_list.push_str("Taking place");
            if e.location() != "" {
                event_list.push_str(format!(" at {}", e.location()).as_str());
            }
            if e.timedelta().is_some() {
                event_list.push_str(format!(" in {} minutes", e.timedelta().unwrap()).as_str());
            }
        }
        event_list.push_str(".\n ");
    });

    let message = format!(
        "Hey {}! You have {} events due today.\n {}",
        op.username,
        count,
        event_list,
    );

    op.fetch_tts_and_save(message).await.unwrap();
    sleep_until(Instant::now() + Duration::from_secs(120)).await;
    //op.wait_for_end().await;

    op.fade_out().await;

    op.play_file("tts.mp3".to_string()).await;

    /*
    sleep_until(Instant::now() + Duration::from_millis(500)).await;
    op.wait_for_end().await;

    if let Some(title) = &track {
        op.play_uri(format!("https://venture.zossennews.de/media/Audio/{}/stream.mp3?startTimeTicks=1200000000", title.id).to_string(), false).await;
        op.play().await;
        op.fade_in().await;
    };
    */
}

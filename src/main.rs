use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::net::{IpAddr, TcpStream};
use std::path::Path;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, sleep};
use std::time::Duration;

use discord_rich_presence::{DiscordIpc, DiscordIpcClient, activity};
use serde_json::from_str;
use zerocopy::{FromBytes, Immutable, KnownLayout};

#[derive(FromBytes, KnownLayout, Immutable)]
#[repr(C, packed)]
struct GameBytes {
    magic: u32,
    index: i32,
    titleid: [u8; 10],
    title: [u8; 128],
}

struct Game {
    titleid: String,
    title: String,
    image_url: String,
}

#[derive(serde::Deserialize)]
struct Config {
    ip: String,
    client_id: String,
    default_image: Option<String>,
    show_live_area: bool,
    refresh_interval: u64,
}

fn get_config(config_path: &Path) -> Config {
    if !config_path.exists() {
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::write(
            config_path,
            r#"{
    "ip": "YOUR_VITA_IP",
    "client_id": "YOUR_DISCORD_APP_ID",
    "default_image": "https://gmedia.playstation.com/is/image/SIEPDC/ps-logo-favicon?$icon-196-196--t$",
    "show_live_area": false,
    "refresh_interval": 5
}"#,
        )
        .unwrap();
        log::error!(
            "No config found, a default one has been created at {:?}",
            config_path
        );
        log::error!("Please fill it in and restart the program.");
        std::process::exit(0);
    }

    let json = fs::read_to_string(config_path).unwrap();
    from_str::<Config>(&json).unwrap()
}

fn get_tsv(cache_path: &Path, url: &str) -> HashMap<String, (String, String)> {
    let tsv = if cache_path.exists() {
        fs::read_to_string(cache_path).unwrap()
    } else {
        log::info!("Fetching game database...");
        let text = match reqwest::blocking::get(url) {
            Ok(r) => match r.text() {
                Ok(t) => t,
                Err(e) => {
                    log::error!("Failed to read TSV response: {}", e);
                    std::process::exit(1);
                }
            },
            Err(e) => {
                log::error!("Failed to fetch game database: {}", e);
                std::process::exit(1);
            }
        };

        fs::create_dir_all(cache_path.parent().unwrap()).unwrap();
        fs::write(cache_path, &text).unwrap();
        text
    };

    parse_tsv(&tsv)
}

fn parse_tsv(tsv: &str) -> HashMap<String, (String, String)> {
    let mut map = HashMap::new();

    for line in tsv.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 6 {
            continue;
        }
        let titleid = fields[0].to_string();
        let region = fields[1].to_string();
        let content_id = fields[5].to_string();
        map.insert(titleid, (region, content_id));
    }
    map
}

fn is_ps1_id(titleid: &str) -> bool {
    matches!(
        titleid.get(..4).map(|s| s.to_uppercase()).as_deref(),
        Some(
            "CPCS"
                | "ESPM"
                | "HPS1"
                | "HPS9"
                | "LSP0"
                | "LSP1"
                | "LSP2"
                | "SCAJ"
                | "SCES"
                | "SCPS"
                | "SCUS"
                | "SIPS"
                | "SLES"
                | "SLKA"
                | "SLPM"
                | "SLPS"
                | "SLUS"
        )
    )
}

fn is_psp_id(titleid: &str) -> bool {
    matches!(
        titleid.get(..4).map(|s| s.to_uppercase()).as_deref(),
        Some(
            "NPEG"
                | "NPEH"
                | "NPEX"
                | "NPEZ"
                | "NPHG"
                | "NPHH"
                | "NPHZ"
                | "NPJG"
                | "NPJH"
                | "NPJJ"
                | "NPUF"
                | "NPUG"
                | "NPUH"
                | "NPUX"
                | "NPUZ"
                | "UCAS"
                | "UCES"
                | "UCJB"
                | "UCJM"
                | "UCJS"
                | "UCKS"
                | "UCUS"
                | "ULAS"
                | "ULES"
                | "ULJM"
                | "ULJS"
                | "ULKS"
                | "ULUS"
        )
    )
}

fn get_image_url(
    titleid: &str,
    tsv: &HashMap<String, (String, String)>,
    default_img: &str,
) -> String {
    if is_ps1_id(titleid) {
        return format!(
            "https://raw.githubusercontent.com/Andiweli/HexFlow-Covers/main/Covers/PS1/{}.png",
            titleid
        );
    }

    if is_psp_id(titleid) {
        return format!(
            "https://raw.githubusercontent.com/Andiweli/HexFlow-Covers/main/Covers/PSP/{}.png",
            titleid
        );
    }

    if let Some((region, content_id)) = tsv.get(titleid) {
        let lang = if region == "JP" { "ja" } else { "en" };
        return format!(
            "https://store.playstation.com/store/api/chihiro/00_09_000/container/{}/{}/19/{}/image?w=248&h=248",
            region, lang, content_id
        );
    }

    default_img.to_string()
}

fn vita_client(
    ip: IpAddr,
    game_data: Arc<Mutex<Option<Game>>>,
    tsv: HashMap<String, (String, String)>,
    refresh: Duration,
    default_img: String,
    running: Arc<AtomicBool>,
) {
    let mut fail_count = 0u32;
    const MAX_FAILS: u32 = 5;
    let get_str = |data: &[u8]| {
        let len = data.iter().position(|&x| x == 0).unwrap_or(data.len());
        String::from_utf8_lossy(&data[..len]).to_string()
    };

    while running.load(Ordering::SeqCst) {
        let mut stream = match TcpStream::connect((ip, 0xCAFE)) {
            Ok(s) => {
                fail_count = 0;
                s
            }
            Err(e) => {
                fail_count += 1;
                log::warn!("Connection failed: {}", e);
                if fail_count >= MAX_FAILS {
                    *game_data.lock().unwrap() = None;
                    log::warn!("[VITA] Unable to reach Vita, cleared activity");
                    fail_count = 0;
                }
                thread::sleep(refresh);
                continue;
            }
        };

        if let Err(e) = stream.set_read_timeout(Some(Duration::new(5, 0))) {
            log::warn!("[VITA] Error when setting read time out: {}", e);
            thread::sleep(refresh);
            continue;
        }

        let mut buffer = [0u8; std::mem::size_of::<GameBytes>()];

        match stream.read_exact(&mut buffer) {
            Ok(_) => {}
            Err(e) => {
                log::warn!("[VITA] Read failed: {}", e);
                thread::sleep(refresh);
                continue;
            }
        }
        let game_raw = GameBytes::ref_from_bytes(&buffer).unwrap();

        if game_raw.magic != 0xCAFECAFE {
            log::warn!("[VITA] Invalid packet");
            continue;
        }

        let titleid = match get_str(&game_raw.titleid) {
            s if s.is_empty() => "livearea".to_string(),
            s => s,
        };
        let title = match get_str(&game_raw.title) {
            s if s.is_empty() => "Live Area".to_string(),
            s => s,
        };

        let mut game_lock = game_data.lock().unwrap();

        let image_url = match game_lock.as_ref() {
            Some(g) if g.titleid == titleid => g.image_url.clone(),
            _ => get_image_url(&titleid.to_uppercase(), &tsv, &default_img),
        };

        let game: Game = Game {
            titleid,
            title,
            image_url,
        };

        log::info!("[VITA] Game ID: {}", game.titleid);
        log::info!("[VITA] Game: {}", game.title);
        log::info!("[VITA] Image: {}", game.image_url);

        *game_lock = Some(game);

        thread::sleep(refresh);
    }
}

fn discord_client(
    client_id: &str,
    game_discord: Arc<Mutex<Option<Game>>>,
    show_live_area: bool,
    refresh: Duration,
    running: Arc<AtomicBool>,
) {
    let mut client = DiscordIpcClient::new(client_id);
    while running.load(Ordering::SeqCst) {
        match client.connect() {
            Ok(_) => {
                log::info!("[DISCORD] Connected to Discord");
                break;
            }
            Err(e) => {
                log::warn!("[DISCORD] Failed to connect to discord client {}", e);
                thread::sleep(Duration::from_millis(5000));
                continue;
            }
        }
    }

    while running.load(Ordering::SeqCst) {
        let activity_data = {
            let game = game_discord.lock().unwrap();
            game.as_ref()
                .map(|g| (g.title.clone(), g.image_url.clone()))
        };

        if let Some((title, image_url)) = activity_data {
            if title != "Live Area" || show_live_area {
                let mut activity = activity::Activity::new().state(title);
                if !image_url.is_empty() {
                    activity = activity.assets(activity::Assets::new().large_image(image_url));
                }
                if let Err(e) = client.set_activity(activity) {
                    log::warn!("[DISCORD] Failed to set activity: {}", e);
                    let _ = client.reconnect();
                }
            } else {
                if let Err(e) = client.clear_activity() {
                    log::warn!("[DISCORD] Failed to clear activity: {}", e);
                }
            }
        }
        sleep(refresh);
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config_path = dirs::config_dir()
        .unwrap()
        .join("vita-presence-rs")
        .join("config.json");
    let config: Config = get_config(&config_path);

    let ip = match IpAddr::from_str(&config.ip) {
        Ok(ip) => ip,
        Err(_) => {
            log::error!(
                "IP Address is invalid, please fix it at {}",
                &config_path.display()
            );
            std::process::exit(1)
        }
    };

    let client_id = &config.client_id;
    let show_live_area = config.show_live_area;
    let default_img = config.default_image.unwrap_or_default();
    let refresh_interval = Duration::from_secs(config.refresh_interval);

    let cache_path = dirs::cache_dir().unwrap().join("vita-presence-rs");
    let tsv = get_tsv(
        &cache_path.join("PSV_GAMES.tsv"),
        "http://nopaystation.com/tsv/PSV_GAMES.tsv",
    );

    let game: Arc<Mutex<Option<Game>>> = Arc::new(Mutex::new(None));

    let game_vita = Arc::clone(&game);
    let game_discord = Arc::clone(&game);

    let running = Arc::new(AtomicBool::new(true));
    let r_ctrlc = Arc::clone(&running);
    let r_vita = Arc::clone(&running);
    let r_discord = Arc::clone(&running);

    ctrlc::set_handler(move || {
        r_ctrlc.store(false, Ordering::SeqCst);
    })
    .unwrap();

    log::info!("Use Ctrl+C to quit");
    let handle = thread::spawn(move || {
        vita_client(ip, game_vita, tsv, refresh_interval, default_img, r_vita)
    });
    discord_client(
        client_id,
        game_discord,
        show_live_area,
        refresh_interval,
        r_discord,
    );
    handle.join().unwrap();
}

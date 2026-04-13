mod discord;
mod images;
mod vita;

use std::fs;
use std::net::IpAddr;
use std::path::Path;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self};
use std::time::Duration;

use serde_json::from_str;
use zerocopy::{FromBytes, Immutable, KnownLayout};

use crate::discord::discord_client;
use crate::vita::{vita_client, vita_client_http};

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
    use_iruzzarcana_fork: Option<bool>,
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
    "refresh_interval": 5,
    "use_iruzzarcana_fork": false
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
    let http_fork = config.use_iruzzarcana_fork.unwrap_or(false);

    let cache_path = dirs::cache_dir().unwrap().join("vita-presence-rs");
    let image_cache_path = cache_path.join("image_cache.json");
    let tsv = crate::images::get_tsv(
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
        if !http_fork {
            vita_client(ip, game_vita, tsv, refresh_interval, default_img, r_vita);
        } else {
            vita_client_http(
                ip,
                game_vita,
                tsv,
                image_cache_path,
                refresh_interval,
                default_img,
                r_vita,
            );
        }
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

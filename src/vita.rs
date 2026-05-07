use std::{
    collections::HashMap,
    io::Read,
    net::{IpAddr, SocketAddr, TcpStream},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::sleep,
    time::Duration,
};

use reqwest::blocking::Client;
use scraper::{Html, Selector};
use zerocopy::FromBytes;

use crate::{
    Game, GameBytes,
    images::{get_chihiro_url, get_image, load_litterbox_cache},
};

pub fn vita_client(
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

    let addr = SocketAddr::new(ip, 0xCAFE);

    while running.load(Ordering::SeqCst) {
        let mut stream = match TcpStream::connect_timeout(&addr, refresh) {
            Ok(s) => {
                fail_count = 0;
                s
            }
            Err(e) => {
                fail_count += 1;
                log::warn!("[VITA] Connection failed: {}", e);
                if fail_count >= MAX_FAILS {
                    *game_data.lock().unwrap() = None;
                    log::warn!("[VITA] Unable to reach Vita, cleared activity");
                    fail_count = 0;
                }
                sleep(refresh);
                continue;
            }
        };

        if let Err(e) = stream.set_read_timeout(Some(Duration::from_secs(5))) {
            log::warn!("[VITA] Error when setting read time out: {}", e);
            sleep(refresh);
            continue;
        }

        let mut buffer = [0u8; std::mem::size_of::<GameBytes>()];

        match stream.read_exact(&mut buffer) {
            Ok(_) => {}
            Err(e) => {
                log::warn!("[VITA] Read failed: {}", e);
                sleep(refresh);
                continue;
            }
        }
        let game_raw = GameBytes::ref_from_bytes(&buffer).unwrap();

        if game_raw.magic != 0xCAFECAFE {
            log::warn!("[VITA] Invalid packet");
            sleep(refresh);
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

        let same = matches!(game_lock.as_ref(), Some(g) if g.titleid == titleid);

        let image_url = if same {
            game_lock.as_ref().unwrap().image_url.clone()
        } else {
            get_chihiro_url(&titleid.to_uppercase(), &tsv, &default_img)
        };

        let game: Game = Game {
            titleid,
            title,
            image_url,
        };

        if !same {
            log::info!("[VITA] Game ID: {}", game.titleid);
            log::info!("[VITA] Game: {}", game.title);
            log::info!("[VITA] Image: {}", game.image_url);
        }

        *game_lock = Some(game);

        fail_count = 0;

        sleep(refresh);
    }
}

pub fn vita_client_http(
    ip: IpAddr,
    game_data: Arc<Mutex<Option<Game>>>,
    tsv: HashMap<String, (String, String)>,
    cache_path: PathBuf,
    refresh: Duration,
    default_img: String,
    running: Arc<AtomicBool>,
) {
    let mut fail_count = 0u32;
    const MAX_FAILS: u32 = 5;

    let mut image_cache = load_litterbox_cache(&cache_path);

    let client = Client::builder()
        .timeout(refresh)
        .connect_timeout(refresh)
        .build()
        .expect("Failed to create HTTP poll client");

    let image_client = Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP image upload client");

    let url = format!("http://{}:51966", ip);
    let p_selector = Selector::parse("p").unwrap();
    let img_selector = Selector::parse("img").unwrap();

    while running.load(Ordering::SeqCst) {
        let response = match client.get(&url).send() {
            Ok(r) => r,
            Err(e) => {
                fail_count += 1;
                log::warn!("[VITA] Connection failed: {}", e);
                if fail_count >= MAX_FAILS {
                    *game_data.lock().unwrap() = None;
                    log::warn!("[VITA] Unable to reach Vita, cleared activity");
                    fail_count = 0;
                }
                sleep(refresh);
                continue;
            }
        };

        let data = match response.text() {
            Ok(t) => t.trim_matches(char::from(0)).to_string(),
            Err(e) => {
                fail_count += 1;
                log::warn!("[VITA] Failed to read: {}", e);
                sleep(refresh);
                continue;
            }
        };

        let doc = Html::parse_document(&data);

        let mut p_tags = doc.select(&p_selector);

        let title = p_tags
            .next()
            .map(|el| el.text().collect::<String>())
            .unwrap_or_else(|| "Live Area".to_string());

        let titleid = p_tags
            .next()
            .map(|el| el.text().collect::<String>())
            .unwrap_or_else(|| "livearea".to_string());

        let file_name = match doc
            .select(&img_selector)
            .next()
            .and_then(|img| img.value().attr("src"))
        {
            Some(src) => src.to_string(),
            None => {
                let image_url = get_chihiro_url(&titleid.to_uppercase(), &tsv, &default_img);
                let mut game_lock = game_data.lock().unwrap();
                *game_lock = Some(Game {
                    titleid,
                    title,
                    image_url,
                });
                sleep(refresh);
                fail_count = 0;
                continue;
            }
        };

        let titleid = match titleid {
            s if s.is_empty() => "livearea".to_string(),
            s => s,
        };

        let title = match title {
            s if s.is_empty() => "Live Area".to_string(),
            s => s,
        };

        let local_image = format!("http://{}:51966/{}", ip, file_name);

        let game_lock = game_data.lock().unwrap();

        let same = game_lock
            .as_ref()
            .map(|g| g.titleid == titleid)
            .unwrap_or(false);

        let image_url = if same {
            let url = game_lock.as_ref().unwrap().image_url.clone();
            drop(game_lock);
            url
        } else {
            drop(game_lock);
            get_image(
                &titleid,
                &local_image,
                &tsv,
                &default_img,
                &mut image_cache,
                &cache_path,
                &image_client,
            )
        };

        let mut game_lock = game_data.lock().unwrap();

        let game: Game = Game {
            titleid,
            title,
            image_url,
        };

        if !same {
            log::info!("[VITA] Game ID: {}", game.titleid);
            log::info!("[VITA] Game: {}", game.title);
            log::info!("[VITA] Image: {}", game.image_url);
        }

        *game_lock = Some(game);

        fail_count = 0;
        sleep(refresh);
    }
}

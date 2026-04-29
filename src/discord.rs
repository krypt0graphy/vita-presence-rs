use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::sleep,
    time::Duration,
};

use discord_rich_presence::{DiscordIpc, DiscordIpcClient, activity};

use crate::Game;

pub fn discord_client(
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
                sleep(Duration::from_millis(5000));
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
        } else {
            if let Err(e) = client.clear_activity() {
                log::warn!("[DISCORD] Failed to clear activity: {}", e);
            }
        }
        sleep(refresh);
    }
}

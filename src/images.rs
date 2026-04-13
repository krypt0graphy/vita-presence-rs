use std::{collections::HashMap, fs, path::Path};

use reqwest::blocking::{
    Client,
    multipart::{Form, Part},
};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct LitterboxImage {
    url: String,
    uploaded_at: u64,
}

const CACHE_MAX_AGE: u64 = 60 * 60 * 24 * 3;

pub fn load_litterbox_cache(cache_path: &Path) -> HashMap<String, LitterboxImage> {
    if !cache_path.exists() {
        return HashMap::new();
    }

    let json = fs::read_to_string(cache_path).unwrap_or_default();

    serde_json::from_str(&json).unwrap_or_default()
}

fn save_litterbox_cache(cache_path: &Path, cache: &HashMap<String, LitterboxImage>) {
    if let Ok(json) = serde_json::to_string_pretty(cache) {
        let _ = fs::write(cache_path, json);
    }
}

pub fn get_litterbox_image(
    cache: &mut HashMap<String, LitterboxImage>,
    cache_path: &Path,
    titleid: &str,
) -> Option<String> {
    if let Some(entry) = cache.get(titleid) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if now - entry.uploaded_at < CACHE_MAX_AGE {
            return Some(entry.url.clone());
        } else {
            cache.remove(titleid);
            save_litterbox_cache(cache_path, cache);
        }
    }
    None
}

pub fn insert_litterbox_cache(
    cache: &mut HashMap<String, LitterboxImage>,
    cache_path: &Path,
    titleid: &str,
    url: &str,
) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    cache.insert(
        titleid.to_string(),
        LitterboxImage {
            url: url.to_string(),
            uploaded_at: now,
        },
    );
    save_litterbox_cache(cache_path, cache);
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

pub fn get_chihiro_url(
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

pub fn get_tsv(cache_path: &Path, url: &str) -> HashMap<String, (String, String)> {
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

pub fn parse_tsv(tsv: &str) -> HashMap<String, (String, String)> {
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

pub fn get_image(
    titleid: &str,
    local_image: &str,
    tsv: &HashMap<String, (String, String)>,
    default_img: &str,
    image_cache: &mut HashMap<String, LitterboxImage>,
    cache_path: &Path,
    client: &Client,
) -> String {
    if titleid == "livearea" {
        return default_img.to_string();
    }

    if let Some(cached_url) = get_litterbox_image(image_cache, cache_path, titleid) {
        return cached_url;
    }

    let image_response = match client.get(local_image).send() {
        Err(e) => {
            log::warn!("[VITA] Failed to fetch image: {}", e);
            return get_chihiro_url(&titleid.to_uppercase(), tsv, default_img);
        }
        Ok(r) => r,
    };

    let image_bytes = match image_response.bytes() {
        Err(e) => {
            log::warn!("[VITA] Failed to read image bytes: {}", e);
            return get_chihiro_url(&titleid.to_uppercase(), tsv, default_img);
        }
        Ok(b) => b.to_vec(),
    };

    let litterbox_form = Form::new()
        .text("reqtype", "fileupload")
        .text("time", "72h")
        .part(
            "fileToUpload",
            Part::bytes(image_bytes).file_name(format!("{}.png", titleid)),
        );

    match client
        .post("https://litterbox.catbox.moe/resources/internals/api.php")
        .multipart(litterbox_form)
        .send()
    {
        Err(e) => {
            log::warn!("[VITA] Failed to upload to Litterbox: {}", e);
            get_chihiro_url(&titleid.to_uppercase(), tsv, default_img)
        }
        Ok(r) => match r.text() {
            Err(e) => {
                log::warn!("[VITA] Failed to read Litterbox response: {}", e);
                get_chihiro_url(&titleid.to_uppercase(), tsv, default_img)
            }
            Ok(t) => {
                let url = t.trim().to_string();
                insert_litterbox_cache(image_cache, cache_path, titleid, &url);
                url
            }
        },
    }
}

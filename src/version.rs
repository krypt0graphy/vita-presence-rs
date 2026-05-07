use std::time::Duration;

use reqwest::blocking::Client;

#[derive(serde::Deserialize)]
struct CrateResponse {
    #[serde(rename = "crate")]
    krate: CrateInfo,
}

#[derive(serde::Deserialize)]
struct CrateInfo {
    newest_version: String,
}

pub fn log_version() {
    let current = env!("CARGO_PKG_VERSION");
    log::info!("vita-presence-rs v{}", current);

    let client = match Client::builder()
        .timeout(Duration::from_secs(3))
        .user_agent(concat!("vita-presence-rs/", env!("CARGO_PKG_VERSION")))
        .build()
    {
        Ok(c) => c,
        Err(_) => return,
    };

    let response: CrateResponse = match client
        .get("https://crates.io/api/v1/crates/vita-presence-rs")
        .send()
        .and_then(|r| r.json())
    {
        Ok(r) => r,
        Err(e) => {
            log::debug!("Version check failed: {}", e);
            return;
        }
    };

    let latest = response.krate.newest_version;

    if latest != current {
        log::warn!("A newer version is available: v{}", latest);
        log::warn!(
            "Update at https://github.com/krypt0graphy/vita-presence-rs/releases or your installation method"
        )
    }
}

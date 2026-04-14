# vita-presence-rs
![GitHub License](https://img.shields.io/github/license/krypt0graphy/vita-presence-rs)
![GitHub Downloads (all assets, latest release)](https://img.shields.io/github/downloads/krypt0graphy/vita-presence-rs/latest/total)
![Crates.io Downloads (latest version)](https://img.shields.io/crates/dv/vita-presence-rs)
![AUR Version](https://img.shields.io/aur/version/vita-presence-rs)


![screenshot](images/screenshot.png)

This is a client for the [VitaPresence](https://github.com/Electry/VitaPresence) PS Vita kernel plugin

It works with the [original by @Electry](https://github.com/Electry/VitaPresence) and [@IruzzArcana's fork](https://github.com/IruzzArcana/VitaPresence) which adds native image support to the plugin

If you are using Electry's version the images are fetched from the Playstation Store Chihiro API, or the [HexFlow covers Repository](https://github.com/Andiweli/HexFlow-Covers)

If you are using IruzzArcana's version the images are fetched directly from the console

## Download
- [Latest Release](https://github.com/krypt0graphy/vita-presence-rs/releases/latest)
- [Crates.io](https://crates.io/crates/vita-presence-rs) 
```bash
cargo install vita-presence-rs
```
- [AUR](https://aur.archlinux.org/packages/vita-presence-rs)
```bash
yay -S vita-presence-rs
```
- [AUR Prebuilt](https://aur.archlinux.org/packages/vita-presence-rs-bin)
```bash
yay -S vita-presence-rs-bin
```

## Instructions
1. Install the kernel plugin on your Vita

    I recommend using [@IruzzArcana's version](https://github.com/IruzzArcana/VitaPresence) but it works with the [original from @Electry](https://github.com/Electry/VitaPresence) as well, if you installed from AutoPlugin II it will be the original

    ***Be sure to turn on the config option if you are using the first one***

2. Create an application on the [Discord Developer Portal](https://discord.com/developers/home) name it something like PS Vita, this will show on your profile as "**Playing *PS Vita*** and copy the Application ID

3. Configure the app at:
   - **Linux:** `~/.config/vita-presence-rs/config.json`
   - **Mac:** `~/Library/Application Support/vita-presence-rs/config.json`
   - **Windows:** `%APPDATA%\vita-presence-rs\config.json`
    
    You can run it once to generate a default config
    
```json
{
    "ip": "YOUR_VITA_IP",
    "client_id": "YOUR_DISCORD_APP_ID",
    "default_image": "https://gmedia.playstation.com/is/image/SIEPDC/ps-logo-favicon?$icon-196-196--t$",
    "show_live_area": false,
    "refresh_interval": 5,
    "use_iruzzarcana_fork": false 
}
```

| Field | Description |
|-------|-------------|
| `ip` | Your Vita's local IP address |
| `client_id` | Your Discord application ID |
| `default_image` | Image URL shown when no game image is found, some apps (including homebrew apps if using Electry's version) and the live area if that is enabled |
| `show_live_area` | Show presence when on the Vita home screen |
| `refresh_interval` | How often to poll the Vita in seconds |
| `use_iruzzarcana_fork` | Whether you are using [IruzzArcana's fork of the VitaPresence plugin](https://github.com/IruzzArcana/VitaPresence) on your Vita |

### Only make the last option true if this version of the plugin is installed on your PS Vita, it will not work otherwise

4. Run it

## Building

### Requirements
- [Rust](https://rustup.rs)

```bash
git clone https://github.com/krypt0graphy/vita-presence-rs.git
cd vita-presence-rs
cargo build --release
```

Binary will be at `target/release/vita-presence-rs` (or `vita-presence-rs.exe` on Windows).

## Credits
- [Electry](https://github.com/Electry) for VitaPresence
- [IruzzArcana](https://github.com/IruzzArcana) for their fork of VitaPresence
- [Andiweli](https://github.com/Andiweli) for the HexFlow covers repository
- [TheMightyV](https://github.com/TheMightyV) for vita-presence-the-server
# tgxplorer

Telegram Exported Chat Explorer — browse message chains from Telegram Desktop JSON exports. Native GUI built with [iced](https://github.com/iced-rs/iced).

![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange) ![License](https://img.shields.io/badge/license-MIT-blue)

## Quick start

```bash
cargo run --release -- path/to/result.json
```

Or open a file from the GUI:

```bash
cargo run --release
```

## Features

- **View modes**: ALL, MNTH, WEEK, 3DAYS, DAY, 6HRS, 3HRS, 1HRS, CHAINS
- Reply-based **message chain** discovery
- Full-text **search** across messages
- Content stats in header: 🔗 🖼️ 📹 📎 🔰 🎤 ⭕ 🔁
- Media type tags per message: `[image]` `[video]` `[file]` `[sticker]` `[voice]` `[video_circle]`
- **Copy** links and message text to clipboard
- 9 **themes** (Dark, Dracula, Nord, Solarized, Gruvbox, Catppuccin, Tokyo Night, Oxocarbon)
- Single binary, no runtime dependencies

## Build

### macOS

```bash
chmod +x build.sh && ./build.sh
```

### Linux (Debian/Ubuntu)

```bash
sudo apt install -y libxkbcommon-dev libwayland-dev libvulkan-dev pkg-config cmake
cargo build --release
./target/release/tgxplorer
```

### Windows

```powershell
cargo build --release
.\target\release\tgxplorer.exe
```

## Getting your Telegram export

1. Open **Telegram Desktop** → any chat → `⋮` → **Export chat history**
2. Choose **JSON** format
3. Point tgxplorer at the resulting `result.json`


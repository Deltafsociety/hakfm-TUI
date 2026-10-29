# HAK.FM TUI

A Rust terminal radio receiver for hacker focus, ambient work sessions, and late-night debugging. Built with Ratatui and Crossterm; audio playback is handled by `mpv`.

> This is an independent client, not an official HAK.FM project. It currently uses direct SomaFM MP3 streams for its station presets. It does not scrape or depend on HAK.FM's website.

## Features

- Dark terminal UI with green/cyan cyberpunk styling
- 18 curated radio presets including DEF CON Radio, Drone Zone, Groove Salad, and Deep Space One
- Keyboard navigation and station switching
- Search by station name, genre, or description
- Favorites saved in `~/.config/hakfm/config.json`
- Playback status, elapsed session timer, and connection-ended detection
- No browser required

## Requirements

On Arch Linux:

```bash
sudo pacman -S rust mpv
```

## Run

```bash
cargo run --release
```

## Controls

| Key | Action |
|---|---|
| `↑` / `k`, `↓` / `j` | Navigate stations |
| `Enter` | Play selected station |
| `Space` | Stop/resume the selected station |
| `←` / `h`, `→` / `l` | Tune previous/next station |
| `/` | Search |
| `s` | Add/remove favorite |
| `f` | Show favorites only |
| `x` | Stop playback |
| `r` | Reset filters |
| `?` | Help |
| `q` | Quit |

## Notes

- Streams can change or become unavailable. If one fails, try another station.
- The `Space` key stops and restarts a station when resuming. Seamless pause/resume and volume controls would require adding mpv IPC.
- The preset URLs use SomaFM's public MP3 stream pattern. The receiver does not host or redistribute audio.
- On quit, the application terminates the `mpv` process it started.


The moving bar is a lightweight animated visual, not an audio-spectrum measurement. Playback is handled by `mpv`; station streams are provided by SomaFM.

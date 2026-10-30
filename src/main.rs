use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Wrap,
    },
    Frame, Terminal,
};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

const AUTHOR_NAME: &str = "Delta Fsociety";
const AUTHOR_URL: &str = "https://zist.digital/delta/contact-me/";

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        let mut truncated: String = s.chars().take(max_len.saturating_sub(1)).collect();
        truncated.push('…');
        truncated
    } else {
        s.to_string()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StationSource {
    #[default]
    Preset,
    Custom,
    InternetRadio,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Station {
    pub name: String,
    pub genre: String,
    pub description: String,
    pub url: String,
    #[serde(default)]
    pub source: StationSource,
}

fn preset_stations() -> Vec<Station> {
    let mut list = vec![
        Station {
            name: "DEF CON Radio".into(),
            genre: "HACKER".into(),
            description: "The year-round DEF CON channel.".into(),
            url: "https://ice5.somafm.com/defcon-128-mp3".into(),
            source: StationSource::Preset,
        },
        Station {
            name: "MansionNET Radio".into(),
            genre: "COMMUNITY".into(),
            description: "Privacy-first community radio from MansionNET.".into(),
            url: "https://radio.inthemansion.com/listen/mansionnet/radio.mp3".into(),
            source: StationSource::Preset,
        },
    ];

    let somafm_rows = [
        ("Groove Salad", "FOCUS", "Downtempo grooves for long sessions.", "groovesalad"),
        ("Drone Zone", "AMBIENT", "Slow atmospheric soundscapes.", "dronezone"),
        ("Deep Space One", "AMBIENT", "Deep ambient for quiet work.", "deepspaceone"),
        ("The Dark Zone", "DARK AMBIENT", "The darker end of deep ambient.", "darkzone"),
        ("Doomed", "INDUSTRIAL", "Dark industrial and ambient.", "doomed"),
        ("cliqhop idm", "IDM", "Blips, beeps, and intricate beats.", "cliqhop"),
        ("Groove Salad Classic", "DOWNTEMPO", "Classic early-2000s chill.", "gsclassic"),
        ("Space Station Soma", "SPACE", "Spaced-out ambient electronica.", "spacestation"),
        ("Mission Control", "SPACE", "Mission audio woven into electronica.", "missioncontrol"),
        ("Vaporwaves", "VAPORWAVE", "A hazy vaporwave signal.", "vaporwaves"),
        ("Underground 80s", "SYNTH / NEW WAVE", "Early-80s synthpop and new wave.", "u80s"),
        ("Metal Detector", "METAL", "From black metal to doom and thrash.", "metal"),
        ("Dub Step Beyond", "BASS", "Dubstep, dub, and deep bass.", "dubstep"),
        ("Secret Agent", "SPY JAZZ", "Spy jazz and lounge atmosphere.", "secretagent"),
        ("Sonic Universe", "JAZZ", "Modern jazz and fusion.", "sonicuniverse"),
        ("Beat Blender", "ELECTRONIC", "Downtempo and electronic grooves.", "beatblender"),
        ("The Trip", "PSYCHEDELIC", "Psychedelic electronic journeys.", "thetrip"),
        ("Lush", "CHILL", "Atmospheric vocals and electronic music.", "lush"),
        ("Illinois Street Lounge", "LOUNGE", "Vintage lounge and exotica.", "illstreet"),
        ("Fluid", "DOWNTEMPO", "Downtempo electronic music.", "fluid"),
        ("PopTron", "INDIE", "Indie pop and alternative.", "poptron"),
        ("Black Rock FM", "FESTIVAL", "Burning Man community radio.", "brfm"),
    ];

    for (name, genre, description, slug) in somafm_rows {
        let slug = slug.trim();
        list.push(Station {
            name: name.to_string(),
            genre: genre.to_string(),
            description: description.to_string(),
            url: format!("https://ice5.somafm.com/{slug}-128-mp3"),
            source: StationSource::Preset,
        });
    }

    list
}

fn default_volume() -> u8 {
    65
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VisualizerMode {
    #[default]
    Spectrum,
    Waveform,
    Analyzer,
    MatrixDots,
    Off,
}

impl VisualizerMode {
    pub fn next(&self) -> Self {
        match self {
            Self::Spectrum => Self::Waveform,
            Self::Waveform => Self::Analyzer,
            Self::Analyzer => Self::MatrixDots,
            Self::MatrixDots => Self::Off,
            Self::Off => Self::Spectrum,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Spectrum => "Spectrum",
            Self::Waveform => "Waveform",
            Self::Analyzer => "Stereo VU",
            Self::MatrixDots => "Matrix Dots",
            Self::Off => "Off",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    Cyberpunk,
    Matrix,
    Nord,
    Dracula,
    Stealth,
}

impl ThemeMode {
    pub fn next(&self) -> Self {
        match self {
            Self::Cyberpunk => Self::Matrix,
            Self::Matrix => Self::Nord,
            Self::Nord => Self::Dracula,
            Self::Dracula => Self::Stealth,
            Self::Stealth => Self::Cyberpunk,
        }
    }

    pub fn theme(&self) -> Theme {
        match self {
            Self::Cyberpunk => Theme {
                name: "Cyberpunk",
                bg: Color::Rgb(8, 12, 16),
                panel: Color::Rgb(14, 21, 27),
                accent: Color::Rgb(0, 255, 153),
                secondary: Color::Rgb(60, 210, 220),
                muted: Color::Rgb(105, 130, 140),
                text: Color::Rgb(220, 235, 235),
                warning: Color::Rgb(255, 204, 0),
                error: Color::Rgb(255, 90, 105),
                border_dim: Color::Rgb(30, 48, 56),
                accent_bg: Color::Rgb(18, 38, 34),
                dim_text: Color::Rgb(75, 105, 115),
            },
            Self::Matrix => Theme {
                name: "Matrix",
                bg: Color::Rgb(6, 12, 7),
                panel: Color::Rgb(10, 20, 12),
                accent: Color::Rgb(35, 230, 90),
                secondary: Color::Rgb(85, 255, 120),
                muted: Color::Rgb(80, 125, 90),
                text: Color::Rgb(200, 240, 210),
                warning: Color::Rgb(220, 180, 30),
                error: Color::Rgb(255, 80, 80),
                border_dim: Color::Rgb(24, 46, 28),
                accent_bg: Color::Rgb(16, 36, 20),
                dim_text: Color::Rgb(60, 95, 70),
            },
            Self::Nord => Theme {
                name: "Nord",
                bg: Color::Rgb(15, 20, 28),
                panel: Color::Rgb(22, 29, 40),
                accent: Color::Rgb(136, 192, 208),
                secondary: Color::Rgb(129, 161, 193),
                muted: Color::Rgb(110, 126, 150),
                text: Color::Rgb(229, 233, 240),
                warning: Color::Rgb(235, 203, 139),
                error: Color::Rgb(191, 97, 106),
                border_dim: Color::Rgb(36, 48, 66),
                accent_bg: Color::Rgb(26, 42, 58),
                dim_text: Color::Rgb(85, 100, 125),
            },
            Self::Dracula => Theme {
                name: "Dracula",
                bg: Color::Rgb(15, 13, 24),
                panel: Color::Rgb(24, 21, 38),
                accent: Color::Rgb(255, 121, 198),
                secondary: Color::Rgb(189, 147, 249),
                muted: Color::Rgb(120, 110, 145),
                text: Color::Rgb(248, 248, 242),
                warning: Color::Rgb(255, 184, 108),
                error: Color::Rgb(255, 85, 85),
                border_dim: Color::Rgb(48, 38, 70),
                accent_bg: Color::Rgb(40, 26, 56),
                dim_text: Color::Rgb(95, 85, 120),
            },
            Self::Stealth => Theme {
                name: "Stealth",
                bg: Color::Rgb(10, 10, 10),
                panel: Color::Rgb(20, 20, 20),
                accent: Color::Rgb(245, 245, 245),
                secondary: Color::Rgb(156, 220, 254),
                muted: Color::Rgb(130, 130, 130),
                text: Color::Rgb(225, 225, 225),
                warning: Color::Rgb(240, 195, 80),
                error: Color::Rgb(240, 90, 90),
                border_dim: Color::Rgb(45, 45, 45),
                accent_bg: Color::Rgb(35, 35, 35),
                dim_text: Color::Rgb(90, 90, 90),
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub name: &'static str,
    pub bg: Color,
    pub panel: Color,
    pub accent: Color,
    pub secondary: Color,
    pub muted: Color,
    pub text: Color,
    pub warning: Color,
    pub error: Color,
    pub border_dim: Color,
    pub accent_bg: Color,
    pub dim_text: Color,
}

#[derive(Default, Serialize, Deserialize)]
struct Config {
    #[serde(default)]
    favorites: Vec<String>,
    #[serde(default)]
    last_station: Option<String>,
    #[serde(default = "default_volume")]
    volume: u8,
    #[serde(default)]
    custom_stations: Vec<Station>,
    #[serde(default)]
    visualizer: VisualizerMode,
    #[serde(default)]
    theme: ThemeMode,
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hakfm")
        .join("config.json")
}

fn load_config() -> Config {
    fs::read_to_string(config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Config {
            volume: default_volume(),
            ..Config::default()
        })
}

fn save_config(config: &Config) {
    let path = config_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(data) = serde_json::to_string_pretty(config) {
        let _ = fs::write(path, data);
    }
}

// Internet-Radio.com function is here, feel free to modify and send a pull request if you have a better idea <3
fn url_encode(input: &str) -> String {
    let mut encoded = String::new();
    for b in input.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            b' ' => encoded.push('+'),
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}

fn unescape_html(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            let mut entity = String::new();
            while let Some(&next_c) = chars.peek() {
                if next_c == ';' {
                    chars.next();
                    break;
                } else if next_c == ' ' || next_c == '<' || entity.len() > 10 {
                    break;
                }
                entity.push(chars.next().unwrap());
            }
            match entity.as_str() {
                "amp" => result.push('&'),
                "quot" => result.push('"'),
                "apos" | "#039" | "#39" => result.push('\''),
                "lt" => result.push('<'),
                "gt" => result.push('>'),
                "nbsp" => result.push(' '),
                s if s.starts_with('#') => {
                    if let Ok(cp) = s[1..].parse::<u32>() {
                        if let Some(ch) = char::from_u32(cp) {
                            result.push(ch);
                        } else {
                            result.push_str(&format!("&{};", s));
                        }
                    } else {
                        result.push_str(&format!("&{};", s));
                    }
                }
                other => {
                    result.push('&');
                    result.push_str(other);
                    result.push(';');
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

fn strip_tags(input: &str) -> String {
    let mut in_tag = false;
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            out.push(c);
        }
    }
    out
}

fn search_internet_radio(query: &str) -> Result<Vec<Station>, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let encoded_query = url_encode(trimmed);
    let url = format!("https://www.internet-radio.com/search/?radio={encoded_query}");

    let resp = ureq::get(&url)
        .set("User-Agent", "Mozilla/5.0 (hakfm; Linux x86_64; rv:120.0)")
        .timeout(Duration::from_secs(12))
        .call()
        .map_err(|e| format!("Network request failed: {e} (check your internet connection)"))?;

    let html = resp
        .into_string()
        .map_err(|e| format!("Failed to read response: {e}"))?;

    let mut stations = Vec::new();

    for row in html.split("<tr>").skip(1) {
        let row = match row.split("</tr>").next() {
            Some(r) => r,
            None => continue,
        };

        let stream_url = if let Some(pos) = row.find("/servers/tools/playlistgenerator/?u=") {
            let rest = &row[pos..];
            let end = rest.find('"').or_else(|| rest.find('\'')).unwrap_or(rest.len());
            let raw = &rest[..end].replace("&amp;", "&");
            if raw.starts_with('/') {
                format!("https://www.internet-radio.com{raw}")
            } else {
                raw.to_string()
            }
        } else if let Some(pos) = row.find("http://").or_else(|| row.find("https://")) {
            let rest = &row[pos..];
            let end = rest.find('"').or_else(|| rest.find('\'')).unwrap_or(rest.len());
            let raw = &rest[..end].replace("&amp;", "&");
            if raw.ends_with(".pls") || raw.ends_with(".m3u") {
                raw.to_string()
            } else {
                continue;
            }
        } else {
            continue;
        };

        let name = if let Some(h4_start) = row.find("<h4") {
            let after_h4 = &row[h4_start..];
            if let Some(tag_end) = after_h4.find('>') {
                let content = &after_h4[tag_end + 1..];
                if let Some(h4_close) = content.find("</h4>") {
                    unescape_html(&strip_tags(&content[..h4_close]))
                } else {
                    "Unknown Station".to_string()
                }
            } else {
                "Unknown Station".to_string()
            }
        } else {
            "Unknown Station".to_string()
        };
        let name = name.trim().to_string();
        if name.is_empty() {
            continue;
        }

        let genre = if let Some(g_start) = row.find("Genres:") {
            let rest = &row[g_start + 7..];
            let end = rest.find("</td>").unwrap_or(rest.len().min(300));
            let g = unescape_html(&strip_tags(&rest[..end]));
            let clean = g.split_whitespace().collect::<Vec<_>>().join(" ");
            if clean.is_empty() {
                "INTERNET-RADIO".to_string()
            } else {
                clean.to_uppercase()
            }
        } else {
            "INTERNET-RADIO".to_string()
        };

        let desc = if let Some(i_start) = row.find("<i") {
            let rest = &row[i_start..];
            if let Some(tag_end) = rest.find('>') {
                let content = &rest[tag_end + 1..];
                if let Some(i_close) = content.find("</i>") {
                    unescape_html(&strip_tags(&content[..i_close]))
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let bitrate = if let Some(kbps_idx) = row.find("Kbps") {
            let before = &row[..kbps_idx];
            let num: String = before
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_digit() || *c == ' ')
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            let num = num.trim();
            if !num.is_empty() {
                format!("{num} Kbps")
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let description = if !bitrate.is_empty() && !desc.trim().is_empty() {
            format!("{bitrate} · {}", desc.trim())
        } else if !bitrate.is_empty() {
            format!("{bitrate} stream")
        } else if !desc.trim().is_empty() {
            desc.trim().to_string()
        } else {
            "Internet radio stream".to_string()
        };

        stations.push(Station {
            name,
            genre,
            description,
            url: stream_url,
            source: StationSource::InternetRadio,
        });
    }

    Ok(stations)
}


// View & Input Modes functions are here


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ViewMode {
    Library,
    InternetRadio,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InputMode {
    Normal,
    Filter,
    InternetRadioSearch,
    AddStation,
}

#[derive(Default)]
struct AddStationForm {
    name: String,
    url: String,
    genre: String,
    description: String,
    focused_field: usize, // 0: Name, 1: URL, 2: Genre, 3: Description
    error_msg: Option<String>,
}

struct App {
    stations: Vec<Station>,
    filtered: Vec<usize>,
    selected: usize,
    config: Config,
    view_mode: ViewMode,
    input_mode: InputMode,
    search: String,
    status: String,
    current_station: Option<Station>,
    player: Option<Child>,
    paused: bool,
    started: Option<Instant>,
    elapsed_base: Duration,
    show_help: bool,
    show_favorites: bool,
    should_quit: bool,
    confirm_quit: bool,
    spinner_tick: usize,

    // Internet-Radio.com search
    ir_query: String,
    ir_search_input: String,
    ir_stations: Vec<Station>,
    ir_selected: usize,
    ir_loading: bool,
    ir_tx: mpsc::Sender<Result<Vec<Station>, String>>,
    ir_rx: mpsc::Receiver<Result<Vec<Station>, String>>,

    // Add custom station form
    add_form: AddStationForm,
}

impl App {
    fn new() -> Self {
        let mut stations = preset_stations();
        let config = load_config();

        for custom in &config.custom_stations {
            let mut c = custom.clone();
            c.source = StationSource::Custom;
            stations.push(c);
        }

        let (ir_tx, ir_rx) = mpsc::channel();

        let mut app = Self {
            stations,
            filtered: Vec::new(),
            selected: 0,
            config,
            view_mode: ViewMode::Library,
            input_mode: InputMode::Normal,
            search: String::new(),
            status: "Ready. Pick a station to tune in (ง'̀-'́)ง".into(),
            current_station: None,
            player: None,
            paused: false,
            started: None,
            elapsed_base: Duration::ZERO,
            show_help: false,
            show_favorites: false,
            should_quit: false,
            confirm_quit: false,
            spinner_tick: 0,

            ir_query: String::new(),
            ir_search_input: String::new(),
            ir_stations: Vec::new(),
            ir_selected: 0,
            ir_loading: false,
            ir_tx,
            ir_rx,

            add_form: AddStationForm::default(),
        };

        app.rebuild_filter();

        // Restore last station if configured
        if let Some(last_name) = &app.config.last_station {
            if let Some(idx) = app.stations.iter().position(|s| &s.name == last_name) {
                if let Some(fpos) = app.filtered.iter().position(|&fi| fi == idx) {
                    app.selected = fpos;
                }
            }
        }

        app
    }

    fn rebuild_filter(&mut self) {
        let query = self.search.to_lowercase();
        self.filtered = self
            .stations
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                let favorite_ok = !self.show_favorites || self.config.favorites.contains(&s.name);
                let search_ok = query.is_empty()
                    || s.name.to_lowercase().contains(&query)
                    || s.genre.to_lowercase().contains(&query)
                    || s.description.to_lowercase().contains(&query);
                favorite_ok && search_ok
            })
            .map(|(i, _)| i)
            .collect();

        if self.filtered.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len() - 1;
        }
    }

    fn play_selected(&mut self) {
        match self.view_mode {
            ViewMode::Library => {
                if self.filtered.is_empty() {
                    self.status = "No stations match your filter. Try lowering your standards ¯\\_(ツ)_/¯".into();
                    return;
                }
                let idx = self.filtered[self.selected];
                let station = self.stations[idx].clone();
                self.play_station(station);
            }
            ViewMode::InternetRadio => {
                if self.ir_stations.is_empty() {
                    self.status = "No web-radio stations loaded. Hit 'i' to search first ಠ_ಠ".into();
                    return;
                }
                let station = self.ir_stations[self.ir_selected].clone();
                self.play_station(station);
            }
        }
    }

    fn play_station(&mut self, station: Station) {
        self.stop_player();
        let url = station.url.clone();
        let name = station.name.clone();

        let child = Command::new("mpv")
            .args(["--no-video", "--really-quiet", "--force-window=no", "--term-status-msg="])
            .arg(&url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

        match child {
            Ok(child) => {
                self.player = Some(child);
                self.current_station = Some(station);
                self.paused = false;
                self.started = Some(Instant::now());
                self.elapsed_base = Duration::ZERO;
                self.config.last_station = Some(name.clone());
                self.status = format!("Playing '{name}' · Enjoy the noise (ง'̀-'́)ง");
                save_config(&self.config);
            }
            Err(e) => self.status = format!("mpv failed to start: {e} (is mpv installed?)"),
        }
    }

    fn stop_player(&mut self) {
        if let Some(mut child) = self.player.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.started = None;
        self.elapsed_base = Duration::ZERO;
        self.paused = false;
    }

    fn toggle_pause(&mut self) {
        if let Some(station) = self.current_station.clone() {
            if self.paused {
                self.play_station(station);
            } else {
                self.stop_player();
                self.paused = true;
                self.status = "Playback paused. Press Space to resume (or enjoy the silence ¯\\_(ツ)_/¯)".into();
            }
        } else {
            self.status = "Select a station first before hitting play ಠ_ಠ".into();
        }
    }

    fn next(&mut self, delta: isize) {
        match self.view_mode {
            ViewMode::Library => {
                if self.filtered.is_empty() {
                    return;
                }
                let len = self.filtered.len() as isize;
                self.selected = (self.selected as isize + delta).rem_euclid(len) as usize;
                self.play_selected();
            }
            ViewMode::InternetRadio => {
                if self.ir_stations.is_empty() {
                    return;
                }
                let len = self.ir_stations.len() as isize;
                self.ir_selected = (self.ir_selected as isize + delta).rem_euclid(len) as usize;
                self.play_selected();
            }
        }
    }

    fn toggle_favorite(&mut self) {
        let name_to_fav = match self.view_mode {
            ViewMode::Library => {
                if self.filtered.is_empty() {
                    return;
                }
                self.stations[self.filtered[self.selected]].name.clone()
            }
            ViewMode::InternetRadio => {
                if self.ir_stations.is_empty() {
                    return;
                }
                let ir_st = self.ir_stations[self.ir_selected].clone();
                // If not in library, add it as custom station
                if !self.stations.iter().any(|s| s.name == ir_st.name) {
                    let mut custom = ir_st.clone();
                    custom.source = StationSource::Custom;
                    self.config.custom_stations.push(custom.clone());
                    self.stations.push(custom);
                }
                ir_st.name
            }
        };

        if let Some(pos) = self.config.favorites.iter().position(|n| n == &name_to_fav) {
            self.config.favorites.remove(pos);
            self.status = format!("Removed '{name_to_fav}' from favorites.");
        } else {
            self.config.favorites.push(name_to_fav.clone());
            self.status = format!("Starred '{name_to_fav}' ★ (saved to favorites)");
        }
        save_config(&self.config);
        self.rebuild_filter();
    }

    fn add_selected_ir_to_library(&mut self) {
        if self.ir_stations.is_empty() {
            return;
        }
        let ir_st = self.ir_stations[self.ir_selected].clone();
        if self.stations.iter().any(|s| s.name == ir_st.name || s.url == ir_st.url) {
            self.status = format!("'{}' is already saved in your library ಠ_ಠ", ir_st.name);
            return;
        }

        let mut custom = ir_st.clone();
        custom.source = StationSource::Custom;
        self.config.custom_stations.push(custom.clone());
        save_config(&self.config);
        self.stations.push(custom);
        self.rebuild_filter();
        self.status = format!("Saved '{}' to your library! (⌐■_■)", ir_st.name);
    }

    fn delete_selected_custom_station(&mut self) {
        if self.view_mode != ViewMode::Library || self.filtered.is_empty() {
            return;
        }
        let idx = self.filtered[self.selected];
        let station = &self.stations[idx];
        if station.source != StationSource::Custom {
            self.status = "Cannot delete built-in presets (nice try though 凸(¬‿¬)凸)".into();
            return;
        }

        let name = station.name.clone();
        let url = station.url.clone();
        self.config.custom_stations.retain(|s| s.name != name && s.url != url);
        self.config.favorites.retain(|n| n != &name);
        save_config(&self.config);

        self.stations.remove(idx);
        self.rebuild_filter();
        self.status = format!("Deleted custom station '{name}'.");
    }

    fn start_ir_search(&mut self) {
        let query = self.ir_search_input.trim().to_string();
        if query.is_empty() {
            self.status = "Type a search query first, mind reading isn't implemented ಠ_ಠ".into();
            return;
        }

        self.ir_query = query.clone();
        self.ir_loading = true;
        self.ir_stations.clear();
        self.ir_selected = 0;
        self.view_mode = ViewMode::InternetRadio;
        self.status = format!("Searching internet-radio.com for '{query}'...");

        let tx = self.ir_tx.clone();
        thread::spawn(move || {
            let res = search_internet_radio(&query);
            let _ = tx.send(res);
        });
    }

    fn save_add_station_form(&mut self) {
        let name = self.add_form.name.trim().to_string();
        let url = self.add_form.url.trim().to_string();
        let genre = self.add_form.genre.trim().to_string();
        let desc = self.add_form.description.trim().to_string();

        if name.is_empty() {
            self.add_form.error_msg = Some("Station name is required (can't be blank!).".into());
            self.add_form.focused_field = 0;
            return;
        }
        if url.is_empty() {
            self.add_form.error_msg = Some("Stream URL is required (what are we streaming, silence? ಠ_ಠ).".into());
            self.add_form.focused_field = 1;
            return;
        }

        let final_genre = if genre.is_empty() { "CUSTOM".into() } else { genre.to_uppercase() };
        let final_desc = if desc.is_empty() { "User-added custom station".into() } else { desc };

        let new_station = Station {
            name: name.clone(),
            genre: final_genre,
            description: final_desc,
            url,
            source: StationSource::Custom,
        };

        self.config.custom_stations.push(new_station.clone());
        save_config(&self.config);

        self.stations.push(new_station);
        self.rebuild_filter();

        // Select the newly added station in library
        if let Some(pos) = self.filtered.iter().position(|&fi| self.stations[fi].name == name) {
            self.selected = pos;
        }

        self.status = format!("Added '{name}' to library! (ง'̀-'́)ง");
        self.input_mode = InputMode::Normal;
        self.add_form = AddStationForm::default();
        self.view_mode = ViewMode::Library;
    }

    pub fn theme(&self) -> Theme {
        self.config.theme.theme()
    }

    pub fn cycle_theme(&mut self) {
        self.config.theme = self.config.theme.next();
        save_config(&self.config);
        self.status = format!("Theme set to '{}'", self.config.theme.theme().name);
    }

    pub fn cycle_visualizer(&mut self) {
        self.config.visualizer = self.config.visualizer.next();
        save_config(&self.config);
        self.status = format!("Visualizer set to '{}'", self.config.visualizer.label());
    }

    fn elapsed(&self) -> Duration {
        if self.paused {
            self.elapsed_base
        } else {
            self.started
                .map(|s| self.elapsed_base + s.elapsed())
                .unwrap_or(self.elapsed_base)
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "--version" | "-v" | "-V" => {
                println!("hakfm {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                println!("hakfm v{} - Terminal radio receiver for hacker focus and deep work", env!("CARGO_PKG_VERSION"));
                println!("\nUsage: hakfm [OPTIONS]");
                println!("\nOptions:");
                println!("  -h, --help     Print help");
                println!("  -v, --version  Print version");
                return Ok(());
            }
            _ => {}
        }
    }

    if !Command::new("mpv")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        eprintln!("HAK.FM needs mpv installed to stream audio.");
        eprintln!("  Arch Linux:      sudo pacman -S mpv");
        eprintln!("  Debian / Ubuntu: sudo apt install mpv");
        eprintln!("  Fedora:          sudo dnf install mpv");
        eprintln!("  macOS:           brew install mpv");
        std::process::exit(1);
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new();

    let result = run_app(&mut terminal, &mut app);

    app.stop_player();
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    result?;
    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        // Poll for internet-radio background results
        if let Ok(res) = app.ir_rx.try_recv() {
            app.ir_loading = false;
            match res {
                Ok(stations) => {
                    let count = stations.len();
                    app.ir_stations = stations;
                    app.ir_selected = 0;
                    if count == 0 {
                        app.status = format!(
                            "0 stations found for '{}'. Try another keyword or genre ¯\\_(ツ)_/¯",
                            app.ir_query
                        );
                    } else {
                        app.status = format!(
                            "Found {} stations for '{}'. Pick your vibe (⌐■_■)",
                            count, app.ir_query
                        );
                    }
                }
                Err(err) => {
                    app.status = format!("Search failed: {err} (internet-radio.com might be down)");
                }
            }
        }

        terminal.draw(|f| draw(f, app))?;
        if app.should_quit {
            break;
        }

        if event::poll(Duration::from_millis(150))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                if app.confirm_quit {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                            app.should_quit = true;
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc | KeyCode::Char('q') => {
                            app.confirm_quit = false;
                            app.status = "Exit cancelled. Back to the music (⌐■_■)".into();
                        }
                        _ => {}
                    }
                    continue;
                }

                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    app.confirm_quit = true;
                    continue;
                }

                match app.input_mode {
                    InputMode::Filter => match key.code {
                        KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                            app.search.clear();
                            app.rebuild_filter();
                        }
                        KeyCode::Enter => {
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Backspace => {
                            app.search.pop();
                            app.rebuild_filter();
                        }
                        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                            app.search.push(c);
                            app.rebuild_filter();
                        }
                        _ => {}
                    },

                    InputMode::InternetRadioSearch => match key.code {
                        KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Enter => {
                            app.input_mode = InputMode::Normal;
                            app.start_ir_search();
                        }
                        KeyCode::Backspace => {
                            app.ir_search_input.pop();
                        }
                        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                            app.ir_search_input.push(c);
                        }
                        _ => {}
                    },

                    InputMode::AddStation => match key.code {
                        KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                            app.add_form = AddStationForm::default();
                        }
                        KeyCode::Tab | KeyCode::Down => {
                            app.add_form.focused_field = (app.add_form.focused_field + 1) % 4;
                        }
                        KeyCode::BackTab | KeyCode::Up => {
                            app.add_form.focused_field = (app.add_form.focused_field + 3) % 4;
                        }
                        KeyCode::Enter => {
                            if app.add_form.focused_field < 3
                                && (app.add_form.name.trim().is_empty()
                                    || app.add_form.url.trim().is_empty())
                            {
                                app.add_form.focused_field += 1;
                            } else {
                                app.save_add_station_form();
                            }
                        }
                        KeyCode::Backspace => {
                            match app.add_form.focused_field {
                                0 => {
                                    app.add_form.name.pop();
                                }
                                1 => {
                                    app.add_form.url.pop();
                                }
                                2 => {
                                    app.add_form.genre.pop();
                                }
                                3 => {
                                    app.add_form.description.pop();
                                }
                                _ => {}
                            }
                            app.add_form.error_msg = None;
                        }
                        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                            match app.add_form.focused_field {
                                0 => app.add_form.name.push(c),
                                1 => app.add_form.url.push(c),
                                2 => app.add_form.genre.push(c),
                                3 => app.add_form.description.push(c),
                                _ => {}
                            }
                            app.add_form.error_msg = None;
                        }
                        _ => {}
                    },

                    InputMode::Normal => match key.code {
                        KeyCode::Char('q') => app.confirm_quit = true,
                        KeyCode::Char('?') => app.show_help = !app.show_help,
                        KeyCode::Esc => {
                            app.show_help = false;
                            app.search.clear();
                            app.show_favorites = false;
                            app.rebuild_filter();
                            if app.view_mode == ViewMode::InternetRadio {
                                app.view_mode = ViewMode::Library;
                            }
                        }
                        KeyCode::Tab => {
                            app.view_mode = match app.view_mode {
                                ViewMode::Library => ViewMode::InternetRadio,
                                ViewMode::InternetRadio => ViewMode::Library,
                            };
                        }
                        KeyCode::Char('1') => {
                            app.view_mode = ViewMode::Library;
                        }
                        KeyCode::Char('2') => {
                            app.view_mode = ViewMode::InternetRadio;
                        }
                        KeyCode::Char('i') => {
                            app.input_mode = InputMode::InternetRadioSearch;
                            app.ir_search_input.clear();
                        }
                        KeyCode::Char('a') => {
                            if app.view_mode == ViewMode::InternetRadio {
                                app.add_selected_ir_to_library();
                            } else {
                                app.input_mode = InputMode::AddStation;
                                app.add_form = AddStationForm::default();
                            }
                        }
                        KeyCode::Char('d') | KeyCode::Delete => {
                            app.delete_selected_custom_station();
                        }
                        KeyCode::Char('/') => {
                            app.input_mode = InputMode::Filter;
                            app.search.clear();
                            app.rebuild_filter();
                        }
                        KeyCode::Char('f') => {
                            if app.view_mode == ViewMode::Library {
                                app.show_favorites = !app.show_favorites;
                                app.rebuild_filter();
                            }
                        }
                        KeyCode::Char('s') => app.toggle_favorite(),
                        KeyCode::Char(' ') => app.toggle_pause(),
                        KeyCode::Char('x') => {
                            app.stop_player();
                            app.current_station = None;
                            app.status = "Playback stopped.".into();
                        }
                        KeyCode::Enter => app.play_selected(),
                        KeyCode::Down | KeyCode::Char('j') => match app.view_mode {
                            ViewMode::Library => {
                                if !app.filtered.is_empty() {
                                    app.selected = (app.selected + 1).min(app.filtered.len() - 1);
                                }
                            }
                            ViewMode::InternetRadio => {
                                if !app.ir_stations.is_empty() {
                                    app.ir_selected =
                                        (app.ir_selected + 1).min(app.ir_stations.len() - 1);
                                }
                            }
                        },
                        KeyCode::Up | KeyCode::Char('k') => match app.view_mode {
                            ViewMode::Library => {
                                app.selected = app.selected.saturating_sub(1);
                            }
                            ViewMode::InternetRadio => {
                                app.ir_selected = app.ir_selected.saturating_sub(1);
                            }
                        },
                        KeyCode::Right | KeyCode::Char('l') => app.next(1),
                        KeyCode::Left | KeyCode::Char('h') => app.next(-1),
                        KeyCode::Char('r') => {
                            app.search.clear();
                            app.show_favorites = false;
                            app.rebuild_filter();
                            app.status = "Filters reset. Showing all stations.".into();
                        }
                        KeyCode::Char('t') => app.cycle_theme(),
                        KeyCode::Char('v') => app.cycle_visualizer(),
                        _ => {}
                    },
                }
            }
        }

        app.spinner_tick = app.spinner_tick.wrapping_add(1);

        if let Some(child) = app.player.as_mut() {
            if let Ok(Some(_)) = child.try_wait() {
                app.player = None;
                app.started = None;
                app.status = "Stream ended or mpv disconnected.".into();
            }
        }
    }
    Ok(())
}

fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let theme = app.theme();
    f.render_widget(Block::default().style(Style::default().bg(theme.bg)), area);

    // Responsive layout based on terminal dimensions
    let compact_mode = area.height < 22 || area.width < 75;
    let margin = if compact_mode { 0 } else { 1 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(margin)
        .constraints([
            Constraint::Length(3), // 1. Header (Logo, Tabs, Live Status Badge, Theme indicator)
            Constraint::Length(3), // 2. Player Bar (Now Playing metadata & Stream info)
            Constraint::Min(6),    // 3. Main Workspace (Stations List + Inspector & Visualizer)
            Constraint::Length(1), // 4. Bottom Status & Key Hints Bar
        ])
        .split(area);

    // ─────────────────────────────────────────────────────────────────────────────
    // 1. TOP BAR (Header, Mode Tabs, and On-Air Badge)
    // ─────────────────────────────────────────────────────────────────────────────
    let lib_active = app.view_mode == ViewMode::Library;
    let ir_active = app.view_mode == ViewMode::InternetRadio;

    let (air_dot_color, air_label) = if app.player.is_some() {
        if app.paused {
            (theme.warning, "⏸ PAUSED")
        } else {
            (theme.accent, "● ON AIR")
        }
    } else {
        (theme.muted, "○ IDLE")
    };

    let header_line = Line::from(vec![
        Span::styled(" HAK", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled(".FM", Style::default().fg(theme.text).add_modifier(Modifier::BOLD)),
        Span::styled("  //  ", Style::default().fg(theme.border_dim)),
        Span::styled(
            " [1] LIBRARY ",
            if lib_active {
                Style::default().fg(theme.accent).bg(theme.accent_bg).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.muted)
            },
        ),
        Span::raw(" "),
        Span::styled(
            " [2] WEB-RADIO ",
            if ir_active {
                Style::default().fg(theme.secondary).bg(theme.panel).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.muted)
            },
        ),
        Span::styled("  │  ", Style::default().fg(theme.border_dim)),
        Span::styled(
            air_label,
            Style::default().fg(air_dot_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", Style::default()),
        Span::styled(
            if app.player.is_some() && !app.paused {
                let el = app.elapsed().as_secs();
                format!("{:02}:{:02}", el / 60, el % 60)
            } else {
                String::new()
            },
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled("   ", Style::default()),
        Span::styled(
            format!("[t: {}]", theme.name),
            Style::default().fg(theme.dim_text),
        ),
    ]);

    let header = Paragraph::new(header_line).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.border_dim))
            .style(Style::default().bg(theme.panel)),
    );
    f.render_widget(header, chunks[0]);

    // ─────────────────────────────────────────────────────────────────────────────
    // 2. NOW PLAYING BANNER
    // ─────────────────────────────────────────────────────────────────────────────
    let now = app.current_station.as_ref();
    let elapsed = app.elapsed().as_secs();
    let is_active = app.player.is_some() && !app.paused;

    let player_border_color = if is_active {
        theme.accent
    } else if app.paused {
        theme.warning
    } else {
        theme.border_dim
    };

    let now_line1 = if let Some(station) = now {
        let tag = match station.source {
            StationSource::Preset => "PRESET",
            StationSource::Custom => "CUSTOM",
            StationSource::InternetRadio => "WEB",
        };
        Line::from(vec![
            Span::styled(" ▶ ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled(&station.name, Style::default().fg(theme.text).add_modifier(Modifier::BOLD)),
            Span::styled("  ·  ", Style::default().fg(theme.border_dim)),
            Span::styled(&station.genre, Style::default().fg(theme.secondary)),
            Span::styled("  ·  ", Style::default().fg(theme.border_dim)),
            Span::styled(format!("[{tag}]"), Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
            Span::styled("  ·  ", Style::default().fg(theme.border_dim)),
            Span::styled(
                if app.paused { "Paused" } else { "Playing (ง'̀-'́)ง" },
                Style::default().fg(if app.paused { theme.warning } else { theme.accent }),
            ),
            Span::styled("  ", Style::default()),
            Span::styled(format!("{:02}:{:02}", elapsed / 60, elapsed % 60), Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        ])
    } else if app.paused {
        Line::from(vec![
            Span::styled(" ⏸  ", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
            Span::styled("Playback paused · Press [Space] to resume audio", Style::default().fg(theme.muted)),
        ])
    } else {
        Line::from(vec![
            Span::styled(" ○  ", Style::default().fg(theme.muted)),
            Span::styled("Select a station below and press [Enter] to stream", Style::default().fg(theme.muted)),
        ])
    };

    let now_line2 = if let Some(station) = now {
        Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(
                truncate_str(&station.description, chunks[1].width.saturating_sub(6) as usize),
                Style::default().fg(theme.muted),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(
                "Use ↑/↓ or j/k to navigate, [Tab] for Internet-Radio search",
                Style::default().fg(theme.dim_text),
            ),
        ])
    };

    let player_card = Paragraph::new(vec![now_line1, now_line2]).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(player_border_color))
            .style(Style::default().bg(theme.panel)),
    );
    f.render_widget(player_card, chunks[1]);

    // ─────────────────────────────────────────────────────────────────────────────
    // 3. MAIN WORKSPACE (Left: Stations List, Right: Inspector & Visualizer)
    // ─────────────────────────────────────────────────────────────────────────────
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(chunks[2]);

    match app.view_mode {
        ViewMode::Library => {
            let (list_rect, filter_rect) = if app.input_mode == InputMode::Filter || !app.search.is_empty() {
                let sub = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(3), Constraint::Min(4)])
                    .split(body[0]);
                (sub[1], Some(sub[0]))
            } else {
                (body[0], None)
            };

            if let Some(f_rect) = filter_rect {
                let filter_box = Paragraph::new(format!(" 🔍 {}_", app.search))
                    .style(Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))
                    .block(
                        Block::default()
                            .title(" LIVE FILTER (Enter to confirm, Esc to clear) ")
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(theme.accent))
                            .style(Style::default().bg(theme.panel)),
                    );
                f.render_widget(filter_box, f_rect);
            }

            let title = if app.show_favorites {
                format!(" STATIONS ★ FAVORITES [{}] ", app.filtered.len())
            } else if !app.search.is_empty() {
                format!(" STATIONS // FILTER: \"{}\" [{}] ", app.search, app.filtered.len())
            } else {
                format!(" STATIONS // LIBRARY [{}] ", app.filtered.len())
            };

            let station_items: Vec<ListItem> = app
                .filtered
                .iter()
                .map(|&station_idx| {
                    let station = &app.stations[station_idx];
                    let is_current = app
                        .current_station
                        .as_ref()
                        .map(|s| &s.name)
                        == Some(&station.name);
                    let is_fav = app.config.favorites.contains(&station.name);

                    let prefix = if is_current {
                        "▶ "
                    } else if is_fav {
                        "★ "
                    } else {
                        "  "
                    };

                    let source_tag = match station.source {
                        StationSource::Preset => "",
                        StationSource::Custom => "[C] ",
                        StationSource::InternetRadio => "[W] ",
                    };

                    let name_display = format!("{:<22}", truncate_str(&station.name, 22));
                    let line = Line::from(vec![
                        Span::styled(
                            prefix,
                            Style::default().fg(if is_current {
                                theme.accent
                            } else if is_fav {
                                theme.warning
                            } else {
                                theme.muted
                            }).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            source_tag,
                            Style::default().fg(theme.warning).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            name_display,
                            Style::default()
                                .fg(if is_current { theme.accent } else { theme.text })
                                .add_modifier(if is_current {
                                    Modifier::BOLD
                                } else {
                                    Modifier::empty()
                                }),
                        ),
                        Span::styled(&station.genre, Style::default().fg(theme.muted)),
                    ]);
                    ListItem::new(line)
                })
                .collect();

            let list_block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.secondary))
                .style(Style::default().bg(theme.panel))
                .padding(Padding::horizontal(1));

            let list = List::new(station_items)
                .block(list_block)
                .highlight_style(
                    Style::default()
                        .fg(theme.accent)
                        .bg(theme.accent_bg)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("› ");

            let mut state = ListState::default();
            if !app.filtered.is_empty() {
                state.select(Some(app.selected));
            }
            f.render_stateful_widget(list, list_rect, &mut state);

            let selected_station = app
                .filtered
                .get(app.selected)
                .and_then(|&i| app.stations.get(i));

            render_right_column(f, body[1], selected_station, app, false, &theme);
        }

        ViewMode::InternetRadio => {
            let spinner_frame = SPINNER[app.spinner_tick % SPINNER.len()];
            let title = if app.ir_loading {
                format!(" INTERNET-RADIO.COM // {} SEARCHING... ", spinner_frame)
            } else if app.ir_query.is_empty() {
                " INTERNET-RADIO.COM // [Press 'i' to search] ".into()
            } else {
                format!(
                    " INTERNET-RADIO.COM // \"{}\" [{} found] ",
                    app.ir_query,
                    app.ir_stations.len()
                )
            };

            let station_items: Vec<ListItem> = if app.ir_loading {
                vec![ListItem::new(Line::from(Span::styled(
                    format!("  {} Searching internet-radio.com for '{}'...", spinner_frame, app.ir_query),
                    Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD),
                )))]
            } else if app.ir_stations.is_empty() {
                vec![
                    ListItem::new(Line::from("")),
                    ListItem::new(Line::from(Span::styled(
                        "  No stations loaded. (¯\\_(ツ)_/¯)",
                        Style::default().fg(theme.muted),
                    ))),
                    ListItem::new(Line::from(Span::styled(
                        "  Press 'i' to search thousands of stations (e.g. ambient, lofi, synthwave)",
                        Style::default().fg(theme.secondary),
                    ))),
                ]
            } else {
                app.ir_stations
                    .iter()
                    .map(|station| {
                        let is_current = app
                            .current_station
                            .as_ref()
                            .map(|s| &s.name)
                            == Some(&station.name);
                        let is_fav = app.config.favorites.contains(&station.name);
                        let in_lib = app.stations.iter().any(|s| s.name == station.name);

                        let prefix = if is_current {
                            "▶ "
                        } else if is_fav {
                            "★ "
                        } else if in_lib {
                            "✓ "
                        } else {
                            "  "
                        };

                        let name_display = format!("{:<24}", truncate_str(&station.name, 24));
                        let line = Line::from(vec![
                            Span::styled(
                                prefix,
                                Style::default().fg(if is_current {
                                    theme.accent
                                } else if is_fav {
                                    theme.warning
                                } else if in_lib {
                                    theme.secondary
                                } else {
                                    theme.muted
                                }).add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                name_display,
                                Style::default()
                                    .fg(if is_current { theme.accent } else { theme.text })
                                    .add_modifier(if is_current {
                                        Modifier::BOLD
                                    } else {
                                        Modifier::empty()
                                    }),
                            ),
                            Span::styled(&station.genre, Style::default().fg(theme.muted)),
                        ]);
                        ListItem::new(line)
                    })
                    .collect()
            };

            let list_block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.secondary))
                .style(Style::default().bg(theme.panel))
                .padding(Padding::horizontal(1));

            let list = List::new(station_items)
                .block(list_block)
                .highlight_style(
                    Style::default()
                        .fg(theme.accent)
                        .bg(theme.accent_bg)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("› ");

            let mut state = ListState::default();
            if !app.ir_stations.is_empty() {
                state.select(Some(app.ir_selected));
            }
            f.render_stateful_widget(list, body[0], &mut state);

            let selected_station = app.ir_stations.get(app.ir_selected);
            render_right_column(f, body[1], selected_station, app, true, &theme);
        }
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // 4. BOTTOM KEYBAR & HINTS
    // ─────────────────────────────────────────────────────────────────────────────
    let key_spans = match app.view_mode {
        ViewMode::Library => {
            if area.width >= 96 {
                vec![
                    Span::styled("Enter ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                    Span::styled("Play  ", Style::default().fg(theme.muted)),
                    Span::styled("Space ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                    Span::styled("Pause  ", Style::default().fg(theme.muted)),
                    Span::styled("Tab ", Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD)),
                    Span::styled("Views  ", Style::default().fg(theme.muted)),
                    Span::styled("/ ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                    Span::styled("Filter  ", Style::default().fg(theme.muted)),
                    Span::styled("v ", Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD)),
                    Span::styled("Vis  ", Style::default().fg(theme.muted)),
                    Span::styled("t ", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
                    Span::styled("Theme  ", Style::default().fg(theme.muted)),
                    Span::styled("a ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                    Span::styled("Add  ", Style::default().fg(theme.muted)),
                    Span::styled("s ", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
                    Span::styled("Fav★  ", Style::default().fg(theme.muted)),
                    Span::styled("? ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                    Span::styled("Help  ", Style::default().fg(theme.muted)),
                    Span::styled("q ", Style::default().fg(theme.error).add_modifier(Modifier::BOLD)),
                    Span::styled("Quit", Style::default().fg(theme.muted)),
                ]
            } else {
                vec![
                    Span::styled("Enter ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                    Span::styled("Play ", Style::default().fg(theme.muted)),
                    Span::styled("Space ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                    Span::styled("Pause ", Style::default().fg(theme.muted)),
                    Span::styled("Tab ", Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD)),
                    Span::styled("Views ", Style::default().fg(theme.muted)),
                    Span::styled("/ ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                    Span::styled("Filter ", Style::default().fg(theme.muted)),
                    Span::styled("v ", Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD)),
                    Span::styled("Vis ", Style::default().fg(theme.muted)),
                    Span::styled("t ", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
                    Span::styled("Theme ", Style::default().fg(theme.muted)),
                    Span::styled("? ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                    Span::styled("Help ", Style::default().fg(theme.muted)),
                    Span::styled("q ", Style::default().fg(theme.error).add_modifier(Modifier::BOLD)),
                    Span::styled("Quit", Style::default().fg(theme.muted)),
                ]
            }
        }
        ViewMode::InternetRadio => {
            if area.width >= 96 {
                vec![
                    Span::styled("Enter ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                    Span::styled("Play  ", Style::default().fg(theme.muted)),
                    Span::styled("Space ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                    Span::styled("Pause  ", Style::default().fg(theme.muted)),
                    Span::styled("i ", Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD)),
                    Span::styled("Search  ", Style::default().fg(theme.muted)),
                    Span::styled("v ", Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD)),
                    Span::styled("Vis  ", Style::default().fg(theme.muted)),
                    Span::styled("t ", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
                    Span::styled("Theme  ", Style::default().fg(theme.muted)),
                    Span::styled("a ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                    Span::styled("Save to Lib  ", Style::default().fg(theme.muted)),
                    Span::styled("Tab ", Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD)),
                    Span::styled("Library  ", Style::default().fg(theme.muted)),
                    Span::styled("? ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                    Span::styled("Help  ", Style::default().fg(theme.muted)),
                    Span::styled("q ", Style::default().fg(theme.error).add_modifier(Modifier::BOLD)),
                    Span::styled("Quit", Style::default().fg(theme.muted)),
                ]
            } else {
                vec![
                    Span::styled("Enter ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                    Span::styled("Play ", Style::default().fg(theme.muted)),
                    Span::styled("Space ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                    Span::styled("Pause ", Style::default().fg(theme.muted)),
                    Span::styled("i ", Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD)),
                    Span::styled("Search ", Style::default().fg(theme.muted)),
                    Span::styled("v ", Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD)),
                    Span::styled("Vis ", Style::default().fg(theme.muted)),
                    Span::styled("t ", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
                    Span::styled("Theme ", Style::default().fg(theme.muted)),
                    Span::styled("Tab ", Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD)),
                    Span::styled("Library ", Style::default().fg(theme.muted)),
                    Span::styled("? ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                    Span::styled("Help ", Style::default().fg(theme.muted)),
                    Span::styled("q ", Style::default().fg(theme.error).add_modifier(Modifier::BOLD)),
                    Span::styled("Quit", Style::default().fg(theme.muted)),
                ]
            }
        }
    };

    f.render_widget(Paragraph::new(Line::from(key_spans)), chunks[3]);

    // ─────────────────────────────────────────────────────────────────────────────
    // 5. OVERLAID MODALS
    // ─────────────────────────────────────────────────────────────────────────────
    if app.confirm_quit {
        draw_quit_modal(f, area, &theme);
    } else if app.input_mode == InputMode::InternetRadioSearch {
        draw_ir_search_modal(f, &app.ir_search_input, area, &theme);
    } else if app.input_mode == InputMode::AddStation {
        draw_add_station_modal(f, &app.add_form, area, &theme);
    } else if app.show_help {
        draw_help_modal(f, area, &theme);
    }
}

fn render_right_column(
    f: &mut Frame,
    area: Rect,
    station: Option<&Station>,
    app: &App,
    is_web_mode: bool,
    theme: &Theme,
) {
    let (inspector_rect, visualizer_rect) = match app.config.visualizer {
        VisualizerMode::Off => (area, None),
        _ => {
            if area.height >= 12 {
                let sub = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(7), Constraint::Length(5)])
                    .split(area);
                (sub[0], Some(sub[1]))
            } else {
                (area, None)
            }
        }
    };

    render_inspector(f, inspector_rect, station, app, is_web_mode, theme);
    if let Some(v_rect) = visualizer_rect {
        render_visualizer(f, v_rect, app, theme);
    }
}

fn render_inspector(
    f: &mut Frame,
    area: Rect,
    station: Option<&Station>,
    app: &App,
    is_web_mode: bool,
    theme: &Theme,
) {
    let block = Block::default()
        .title(" STATION INSPECTOR ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border_dim))
        .style(Style::default().bg(theme.panel))
        .padding(Padding::new(2, 2, 0, 0));

    let mut lines = Vec::new();

    if let Some(s) = station {
        lines.push(Line::from(vec![
            Span::styled(&s.name, Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        ]));

        let (source_name, source_color, source_bg) = match s.source {
            StationSource::Preset => ("PRESET", theme.muted, Color::Rgb(20, 30, 36)),
            StationSource::Custom => ("CUSTOM", theme.warning, Color::Rgb(40, 35, 15)),
            StationSource::InternetRadio => ("WEB-RADIO", theme.secondary, Color::Rgb(15, 38, 45)),
        };
        let is_fav = app.config.favorites.contains(&s.name);

        let mut badges = vec![
            Span::styled(
                format!(" {} ", s.genre.to_uppercase()),
                Style::default().fg(theme.secondary).bg(Color::Rgb(15, 35, 42)).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                format!(" {source_name} "),
                Style::default().fg(source_color).bg(source_bg).add_modifier(Modifier::BOLD),
            ),
        ];

        if is_fav {
            badges.push(Span::raw(" "));
            badges.push(Span::styled(
                " ★ FAVORITE ",
                Style::default().fg(theme.warning).bg(Color::Rgb(40, 35, 15)).add_modifier(Modifier::BOLD),
            ));
        }

        if is_web_mode && app.stations.iter().any(|st| st.name == s.name) {
            badges.push(Span::raw(" "));
            badges.push(Span::styled(
                " ✓ SAVED ",
                Style::default().fg(theme.accent).bg(theme.accent_bg).add_modifier(Modifier::BOLD),
            ));
        }

        lines.push(Line::from(badges));
        lines.push(Line::from(""));

        if !s.description.is_empty() {
            lines.push(Line::from(Span::styled(
                "ABOUT",
                Style::default().fg(theme.muted).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::styled(&s.description, Style::default().fg(theme.text))));
            lines.push(Line::from(""));
        }

        lines.push(Line::from(Span::styled(
            "STREAM URL",
            Style::default().fg(theme.muted).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            truncate_str(&s.url, area.width.saturating_sub(6) as usize),
            Style::default().fg(theme.dim_text),
        )));
        lines.push(Line::from(""));

        let divider_len = area.width.saturating_sub(6) as usize;
        lines.push(Line::from(Span::styled(
            "─".repeat(divider_len),
            Style::default().fg(theme.border_dim),
        )));
        lines.push(Line::from(""));

        lines.push(Line::from(Span::styled(
            "CONTROLS",
            Style::default().fg(theme.muted).add_modifier(Modifier::BOLD),
        )));

        let is_playing_this = app.current_station.as_ref().map(|st| &st.name) == Some(&s.name);
        if is_playing_this {
            lines.push(Line::from(vec![
                Span::styled(" [Space] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled(if app.paused { "Resume stream" } else { "Pause stream" }, Style::default().fg(theme.text)),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(" [Enter] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled("Tune in & play stream", Style::default().fg(theme.text)),
            ]));
        }

        lines.push(Line::from(vec![
            Span::styled(" [s]     ", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
            Span::styled(if is_fav { "Remove favorite" } else { "Star as favorite ★" }, Style::default().fg(theme.text)),
        ]));

        if is_web_mode {
            let in_lib = app.stations.iter().any(|st| st.name == s.name);
            lines.push(Line::from(vec![
                Span::styled(" [a]     ", Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD)),
                Span::styled(if in_lib { "Already in Library ✓" } else { "Save to Library" }, Style::default().fg(if in_lib { theme.accent } else { theme.text })),
            ]));
        } else if s.source == StationSource::Custom {
            lines.push(Line::from(vec![
                Span::styled(" [d]     ", Style::default().fg(theme.error).add_modifier(Modifier::BOLD)),
                Span::styled("Delete custom station", Style::default().fg(theme.text)),
            ]));
        }
    } else {
        lines.push(Line::from(Span::styled("No station selected", Style::default().fg(theme.muted))));
        lines.push(Line::from("Use ↑/↓ or j/k to navigate stations."));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("STATUS: ", Style::default().fg(theme.muted).add_modifier(Modifier::BOLD)),
        Span::styled(
            &app.status,
            Style::default().fg(if app.status.contains("failed")
                || app.status.contains("error")
                || app.status.contains("ended")
                || app.status.contains("disconnected")
            {
                theme.error
            } else {
                theme.accent
            }),
        ),
    ]));

    f.render_widget(Paragraph::new(lines).block(block).wrap(Wrap { trim: true }), area);
}

fn render_visualizer(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let is_active = app.player.is_some() && !app.paused;
    let border_color = if is_active {
        theme.accent
    } else if app.paused {
        theme.warning
    } else {
        theme.border_dim
    };

    let title = format!(" AUDIO VISUALIZER ── [v] {} ", app.config.visualizer.label());
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(theme.panel))
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let phase = app
        .started
        .map(|started| started.elapsed().as_millis() as f64 / 160.0)
        .unwrap_or(0.0);
    let vis_width = inner.width as usize;

    let mut lines = Vec::new();

    match app.config.visualizer {
        VisualizerMode::Spectrum => {
            lines.push(spectrum_line(vis_width, phase, is_active, theme));
        }
        VisualizerMode::Waveform => {
            lines.push(wave_line(vis_width, phase, is_active, theme));
        }
        VisualizerMode::Analyzer => {
            let (l, r) = stereo_vu_lines(vis_width, phase, is_active, theme);
            lines.push(l);
            if inner.height > 2 {
                lines.push(r);
            }
        }
        VisualizerMode::MatrixDots => {
            lines.push(matrix_dots_line(vis_width, phase, is_active, theme));
        }
        VisualizerMode::Off => return,
    }

    if inner.height >= 3 {
        let elapsed = app.elapsed().as_secs();
        let status_line = if let Some(st) = &app.current_station {
            if is_active {
                Line::from(vec![
                    Span::styled("● ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                    Span::styled(&st.name, Style::default().fg(theme.text).add_modifier(Modifier::BOLD)),
                    Span::styled(" · ", Style::default().fg(theme.border_dim)),
                    Span::styled(format!("{:02}:{:02}", elapsed / 60, elapsed % 60), Style::default().fg(theme.accent)),
                ])
            } else if app.paused {
                Line::from(vec![
                    Span::styled("⏸ Paused ", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
                    Span::styled(&st.name, Style::default().fg(theme.muted)),
                ])
            } else {
                Line::from(Span::styled("○ Idle", Style::default().fg(theme.muted)))
            }
        } else {
            Line::from(Span::styled("○ Idle", Style::default().fg(theme.muted)))
        };
        lines.push(status_line);
    }

    f.render_widget(Paragraph::new(lines), inner);
}

fn spectrum_line(width: usize, phase: f64, active: bool, theme: &Theme) -> Line<'static> {
    if width == 0 {
        return Line::from("");
    }
    if !active {
        let mut spans = Vec::with_capacity(width);
        for i in 0..width {
            let char_block = if i % 4 == 0 { "▂" } else if i % 2 == 0 { " " } else { " " };
            spans.push(Span::styled(char_block, Style::default().fg(theme.muted)));
        }
        return Line::from(spans);
    }

    const BARS: [&str; 8] = [" ", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
    let mut spans = Vec::with_capacity(width);
    for i in 0..width {
        let x = i as f64;
        let t = phase;
        let low_pulse = (t * 2.2).sin().abs() * (1.0 - (x / width as f64).min(1.0)).powi(2);
        let mid_pulse = ((t * 1.5 + x * 0.42).sin()).abs() * 0.65;
        let high_shimmer = ((t * 3.3 + x * 0.85).cos()).abs() * (x / width as f64).min(1.0) * 0.55;
        let comb = (low_pulse * 0.5 + mid_pulse * 0.35 + high_shimmer * 0.35).clamp(0.0, 1.0);
        let h = ((comb * 7.0).round() as usize).min(BARS.len() - 1);
        let color = if h >= 6 {
            theme.warning
        } else if h >= 3 {
            theme.accent
        } else {
            theme.secondary
        };
        spans.push(Span::styled(BARS[h], Style::default().fg(color)));
    }
    Line::from(spans)
}

fn wave_line(width: usize, phase: f64, active: bool, theme: &Theme) -> Line<'static> {
    if width == 0 {
        return Line::from("");
    }

    if !active {
        return Line::from(Span::styled("─".repeat(width), Style::default().fg(theme.muted)));
    }

    const BARS: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
    let mut spans = Vec::with_capacity(width);
    for i in 0..width {
        let x = i as f64;
        let t = phase - x * 0.24;
        let wave =
            (t.sin() * 0.52 + (t * 0.47 + 1.2).sin() * 0.31 + (t * 0.19).sin() * 0.17).abs();
        let height = ((wave * 7.0).round() as usize).min(BARS.len() - 1);
        let color = if height >= 5 { theme.accent } else { theme.secondary };
        spans.push(Span::styled(BARS[height], Style::default().fg(color)));
    }
    Line::from(spans)
}

fn stereo_vu_lines(width: usize, phase: f64, active: bool, theme: &Theme) -> (Line<'static>, Line<'static>) {
    let bar_len = width.saturating_sub(15).max(4);
    if !active {
        let l = Line::from(vec![
            Span::styled("L ", Style::default().fg(theme.muted).add_modifier(Modifier::BOLD)),
            Span::styled(format!("[{}]", "░".repeat(bar_len)), Style::default().fg(theme.border_dim)),
            Span::styled(" -∞ dB", Style::default().fg(theme.muted)),
        ]);
        let r = Line::from(vec![
            Span::styled("R ", Style::default().fg(theme.muted).add_modifier(Modifier::BOLD)),
            Span::styled(format!("[{}]", "░".repeat(bar_len)), Style::default().fg(theme.border_dim)),
            Span::styled(" -∞ dB", Style::default().fg(theme.muted)),
        ]);
        return (l, r);
    }

    let l_val = ((phase * 1.8).sin().abs() * 0.75 + (phase * 0.6).cos().abs() * 0.25).clamp(0.05, 0.98);
    let r_val = ((phase * 1.9 + 0.9).sin().abs() * 0.72 + (phase * 0.5 + 0.3).cos().abs() * 0.28).clamp(0.05, 0.98);

    let l_fill = ((l_val * bar_len as f64).round() as usize).min(bar_len);
    let r_fill = ((r_val * bar_len as f64).round() as usize).min(bar_len);

    let l_db = format!(" -{:.0} dB", (1.0 - l_val) * 24.0);
    let r_db = format!(" -{:.0} dB", (1.0 - r_val) * 24.0);

    let l_spans = vec![
        Span::styled("L ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("[", Style::default().fg(theme.border_dim)),
        Span::styled("█".repeat(l_fill), Style::default().fg(if l_fill >= bar_len * 4 / 5 { theme.warning } else { theme.accent })),
        Span::styled("░".repeat(bar_len.saturating_sub(l_fill)), Style::default().fg(theme.border_dim)),
        Span::styled("]", Style::default().fg(theme.border_dim)),
        Span::styled(l_db, Style::default().fg(theme.secondary)),
    ];
    let r_spans = vec![
        Span::styled("R ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("[", Style::default().fg(theme.border_dim)),
        Span::styled("█".repeat(r_fill), Style::default().fg(if r_fill >= bar_len * 4 / 5 { theme.warning } else { theme.accent })),
        Span::styled("░".repeat(bar_len.saturating_sub(r_fill)), Style::default().fg(theme.border_dim)),
        Span::styled("]", Style::default().fg(theme.border_dim)),
        Span::styled(r_db, Style::default().fg(theme.secondary)),
    ];

    (Line::from(l_spans), Line::from(r_spans))
}

fn matrix_dots_line(width: usize, phase: f64, active: bool, theme: &Theme) -> Line<'static> {
    if width == 0 {
        return Line::from("");
    }
    if !active {
        return Line::from(Span::styled("· ".repeat(width / 2), Style::default().fg(theme.muted)));
    }

    const DOTS: [&str; 8] = ["⣾", "⣽", "⣻", "⢿", "⣷", "⣯", "⣟", "⡿"];
    let mut spans = Vec::with_capacity(width);
    for i in 0..width {
        let idx = (((phase * 2.0) as usize) + i * 3) % DOTS.len();
        let color = if (i + (phase as usize)) % 4 == 0 {
            theme.accent
        } else if (i + (phase as usize)) % 2 == 0 {
            theme.secondary
        } else {
            theme.muted
        };
        spans.push(Span::styled(DOTS[idx], Style::default().fg(color)));
    }
    Line::from(spans)
}

fn draw_quit_modal(f: &mut Frame, area: Rect, theme: &Theme) {
    let popup = centered_rect(54, 28, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" QUIT HAK.FM? ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.error).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(theme.panel))
        .padding(Padding::new(2, 2, 1, 1));

    let content = vec![
        Line::from(Span::styled("Exit HAK.FM?", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD))),
        Line::from("Playback will stop and your terminal will return to silence."),
        Line::from(""),
        Line::from(vec![
            Span::styled(" [Y / Enter] ", Style::default().fg(theme.error).bg(Color::Rgb(40, 20, 25)).add_modifier(Modifier::BOLD)),
            Span::styled(" Quit    ", Style::default().fg(theme.error)),
            Span::styled(" [N / Esc / q] ", Style::default().fg(theme.accent).bg(theme.accent_bg).add_modifier(Modifier::BOLD)),
            Span::styled(" Stay & Listen ", Style::default().fg(theme.accent)),
        ]),
    ];

    f.render_widget(Paragraph::new(content).block(block).alignment(Alignment::Center), popup);
}

fn draw_ir_search_modal(f: &mut Frame, query: &str, area: Rect, theme: &Theme) {
    let popup = centered_rect(60, 24, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" SEARCH INTERNET-RADIO.COM ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.secondary))
        .style(Style::default().bg(theme.panel))
        .padding(Padding::new(2, 2, 1, 1));

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(block.inner(popup));
    f.render_widget(block, popup);

    let prompt = Paragraph::new(Span::styled(
        "Enter station genre or keywords (e.g. ambient, lofi, synthwave):",
        Style::default().fg(theme.text),
    ));
    f.render_widget(prompt, inner[0]);

    let input_box = Paragraph::new(format!(" {query}_"))
        .style(Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.accent))
                .style(Style::default().bg(Color::Rgb(10, 16, 22))),
        );
    f.render_widget(input_box, inner[1]);

    let hints = Paragraph::new(Line::from(vec![
        Span::styled(" [Enter] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("Search   ", Style::default().fg(theme.muted)),
        Span::styled(" [Esc] ", Style::default().fg(theme.error).add_modifier(Modifier::BOLD)),
        Span::styled("Cancel", Style::default().fg(theme.muted)),
    ]));
    f.render_widget(hints, inner[2]);
}

fn draw_add_station_modal(f: &mut Frame, form: &AddStationForm, area: Rect, theme: &Theme) {
    let popup = centered_rect(66, 54, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" ADD CUSTOM RADIO STATION ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent))
        .style(Style::default().bg(theme.panel))
        .padding(Padding::new(2, 2, 1, 1));

    let inner_area = block.inner(popup);
    f.render_widget(block, popup);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Name
            Constraint::Length(3), // URL
            Constraint::Length(3), // Genre
            Constraint::Length(3), // Description
            Constraint::Length(1), // Error / Note
            Constraint::Length(1), // Hints
        ])
        .split(inner_area);

    let f0_active = form.focused_field == 0;
    let p0 = Paragraph::new(format!(" {}{}", form.name, if f0_active { "_" } else { "" }))
        .block(
            Block::default()
                .title(" Station Name * ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(if f0_active { theme.accent } else { theme.border_dim }))
                .style(Style::default().bg(Color::Rgb(10, 16, 22))),
        );
    f.render_widget(p0, rows[0]);

    let f1_active = form.focused_field == 1;
    let p1 = Paragraph::new(format!(" {}{}", form.url, if f1_active { "_" } else { "" }))
        .block(
            Block::default()
                .title(" Stream URL * (.mp3 / .pls / .m3u / icecast / shoutcast) ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(if f1_active { theme.accent } else { theme.border_dim }))
                .style(Style::default().bg(Color::Rgb(10, 16, 22))),
        );
    f.render_widget(p1, rows[1]);

    let f2_active = form.focused_field == 2;
    let p2 = Paragraph::new(format!(" {}{}", form.genre, if f2_active { "_" } else { "" }))
        .block(
            Block::default()
                .title(" Genre (optional, default: CUSTOM) ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(if f2_active { theme.accent } else { theme.border_dim }))
                .style(Style::default().bg(Color::Rgb(10, 16, 22))),
        );
    f.render_widget(p2, rows[2]);

    let f3_active = form.focused_field == 3;
    let p3 = Paragraph::new(format!(" {}{}", form.description, if f3_active { "_" } else { "" }))
        .block(
            Block::default()
                .title(" Description (optional) ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(if f3_active { theme.accent } else { theme.border_dim }))
                .style(Style::default().bg(Color::Rgb(10, 16, 22))),
        );
    f.render_widget(p3, rows[3]);

    let msg = if let Some(err) = &form.error_msg {
        Line::from(Span::styled(format!(" ⚠ {err}"), Style::default().fg(theme.error).add_modifier(Modifier::BOLD)))
    } else {
        Line::from(Span::styled(" Station will be saved locally to ~/.config/hakfm/config.json", Style::default().fg(theme.muted)))
    };
    f.render_widget(Paragraph::new(msg), rows[4]);

    let hints = Line::from(vec![
        Span::styled(" [Tab/↓] ", Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD)),
        Span::styled("Next  ", Style::default().fg(theme.muted)),
        Span::styled(" [Enter] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("Save Station  ", Style::default().fg(theme.muted)),
        Span::styled(" [Esc] ", Style::default().fg(theme.error).add_modifier(Modifier::BOLD)),
        Span::styled("Cancel", Style::default().fg(theme.muted)),
    ]);
    f.render_widget(Paragraph::new(hints), rows[5]);
}

fn draw_help_modal(f: &mut Frame, area: Rect, theme: &Theme) {
    let popup = centered_rect(72, 85, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" HELP & KEYBOARD SHORTCUTS ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent))
        .style(Style::default().bg(theme.panel))
        .padding(Padding::new(2, 2, 1, 1));

    let content = vec![
        Line::from(vec![
            Span::styled("HAK.FM", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled(" ── Terminal Radio Receiver for Hacker Focus", Style::default().fg(theme.text)),
        ]),
        Line::from(""),
        Line::from(Span::styled("PLAYBACK CONTROLS", Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD))),
        Line::from(vec![
            Span::styled("  Enter          ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("Play selected station", Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  Space          ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("Pause / resume audio stream", Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  x              ", Style::default().fg(theme.error).add_modifier(Modifier::BOLD)),
            Span::styled("Stop playback completely", Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  ←/h , →/l      ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("Tune previous / next station directly", Style::default().fg(theme.text)),
        ]),
        Line::from(""),
        Line::from(Span::styled("NAVIGATION & CUSTOMIZATION", Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD))),
        Line::from(vec![
            Span::styled("  ↑/k , ↓/j      ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("Navigate stations list", Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  Tab / 1 / 2    ", Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD)),
            Span::styled("Switch between [1] Library and [2] Web-Radio", Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  v              ", Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD)),
            Span::styled("Cycle visualizer mode (Spectrum, Waveform, Stereo VU, Matrix, Off)", Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  t              ", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
            Span::styled("Cycle color theme (Cyberpunk, Matrix, Nord, Dracula, Stealth)", Style::default().fg(theme.text)),
        ]),
        Line::from(""),
        Line::from(Span::styled("STATION MANAGEMENT", Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD))),
        Line::from(vec![
            Span::styled("  /              ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("Live filter library by name or genre", Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  i              ", Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD)),
            Span::styled("Search internet-radio.com in TUI", Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  a              ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("Add custom station URL (or save web station to library)", Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  s              ", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
            Span::styled("Toggle favorite (★ starred)", Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  f              ", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
            Span::styled("Show favorites only", Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  d / Del        ", Style::default().fg(theme.error).add_modifier(Modifier::BOLD)),
            Span::styled("Delete selected custom station", Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  r              ", Style::default().fg(theme.text).add_modifier(Modifier::BOLD)),
            Span::styled("Reset all search filters", Style::default().fg(theme.text)),
        ]),
        Line::from(""),
        Line::from(Span::styled("APPLICATION", Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD))),
        Line::from(vec![
            Span::styled("  ?              ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("Toggle this help screen", Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  q / Esc        ", Style::default().fg(theme.error).add_modifier(Modifier::BOLD)),
            Span::styled("Quit application", Style::default().fg(theme.text)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Forged by ", Style::default().fg(theme.muted)),
            Span::styled(AUTHOR_NAME, Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled(" · ", Style::default().fg(theme.border_dim)),
            Span::styled(AUTHOR_URL, Style::default().fg(theme.secondary).add_modifier(Modifier::UNDERLINED)),
        ]),
    ];

    f.render_widget(Paragraph::new(content).block(block).wrap(Wrap { trim: true }), popup);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_ui_render_standard_terminal() {
        let backend = TestBackend::new(88, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new();
        terminal.draw(|f| draw(f, &app)).unwrap();
        println!("\n=== UI PREVIEW (88x24) ===\n{}", terminal.backend());
    }

    #[test]
    fn test_ui_render_all_themes() {
        let backend = TestBackend::new(88, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        for _ in 0..5 {
            terminal.draw(|f| draw(f, &app)).unwrap();
            app.cycle_theme();
        }
    }

    #[test]
    fn test_ui_render_all_visualizers() {
        let backend = TestBackend::new(88, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        for _ in 0..5 {
            terminal.draw(|f| draw(f, &app)).unwrap();
            app.cycle_visualizer();
        }
    }

    #[test]
    fn test_ui_render_large_terminal() {
        let backend = TestBackend::new(140, 45);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        app.view_mode = ViewMode::InternetRadio;
        terminal.draw(|f| draw(f, &app)).unwrap();
    }

    #[test]
    fn test_ui_render_compact_terminal() {
        let backend = TestBackend::new(70, 18);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        app.input_mode = InputMode::Filter;
        app.search = "ambient".to_string();
        terminal.draw(|f| draw(f, &app)).unwrap();
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("superlongstationname", 10), "superlong…");
    }
}

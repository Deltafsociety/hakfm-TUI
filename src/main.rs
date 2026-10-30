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
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
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

const BG: Color = Color::Rgb(8, 12, 16);
const PANEL: Color = Color::Rgb(14, 21, 27);
const GREEN: Color = Color::Rgb(0, 255, 153);
const MUTED: Color = Color::Rgb(105, 130, 140);
const TEXT: Color = Color::Rgb(220, 235, 235);
const CYAN: Color = Color::Rgb(60, 210, 220);
const RED: Color = Color::Rgb(255, 90, 105);
const YELLOW: Color = Color::Rgb(255, 204, 0);

const AUTHOR_NAME: &str = "Delta Fsociety";
const AUTHOR_URL: &str = "https://zist.digital/delta/contact-me/";

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

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
        .map_err(|e| format!("The internet tubes clogged: {e} (╯°□°)╯︵ ┻━┻"))?;

    let html = resp
        .into_string()
        .map_err(|e| format!("Received garbled trash from server: {e} ¯\\_(ツ)_/¯"))?;

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
            status: "Ready to melt braincells. Pick a damn station (ง'̀-'́)ง".into(),
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
                    self.status = "No stations match your picky filter, Einstein! ¯\\_(ツ)_/¯".into();
                    return;
                }
                let idx = self.filtered[self.selected];
                let station = self.stations[idx].clone();
                self.play_station(station);
            }
            ViewMode::InternetRadio => {
                if self.ir_stations.is_empty() {
                    self.status = "No web-radio stations loaded. Search first, genius! ಠ_ಠ".into();
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
                self.status = format!("Blasting '{name}'! Kiss your eardrums goodbye (ง'̀-'́)ง");
                save_config(&self.config);
            }
            Err(e) => self.status = format!("mpv choked and died: {e} (╯°□°)╯︵ ┻━┻"),
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
                self.status = "Muted. Enjoy your pathetic silence, buzzkill. ¯\\_(ツ)_/¯".into();
            }
        } else {
            self.status = "Pick a station first, don't just mash buttons like a monkey! ಠ_ಠ".into();
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
            self.status = format!("Dumped '{name_to_fav}'. Friendship ended with this station. ಠ_ಠ");
        } else {
            self.config.favorites.push(name_to_fav.clone());
            self.status = format!("Simping for '{name_to_fav}'? Starred! ★ ( ͡° ͜ʖ ͡°)");
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
            self.status = format!("Greed is a sin: '{}' is already sitting in your vault! ಠ_ಠ", ir_st.name);
            return;
        }

        let mut custom = ir_st.clone();
        custom.source = StationSource::Custom;
        self.config.custom_stations.push(custom.clone());
        save_config(&self.config);
        self.stations.push(custom);
        self.rebuild_filter();
        self.status = format!("Yoink! Kidnapped '{}' into your Vault! (⌐■_■)", ir_st.name);
    }

    fn delete_selected_custom_station(&mut self) {
        if self.view_mode != ViewMode::Library || self.filtered.is_empty() {
            return;
        }
        let idx = self.filtered[self.selected];
        let station = &self.stations[idx];
        if station.source != StationSource::Custom {
            self.status = "Hands off, script kiddie! Presets are hardcoded and immortal. 凸(¬‿¬)凸".into();
            return;
        }

        let name = station.name.clone();
        let url = station.url.clone();
        self.config.custom_stations.retain(|s| s.name != name && s.url != url);
        self.config.favorites.retain(|n| n != &name);
        save_config(&self.config);

        self.stations.remove(idx);
        self.rebuild_filter();
        self.status = format!("Nuked custom station '{name}' into digital ash! [x_x] (╯°□°)╯");
    }

    fn start_ir_search(&mut self) {
        let query = self.ir_search_input.trim().to_string();
        if query.is_empty() {
            self.status = "Type a search query first, psychic reading isn't implemented! ಠ_ಠ".into();
            return;
        }

        self.ir_query = query.clone();
        self.ir_loading = true;
        self.ir_stations.clear();
        self.ir_selected = 0;
        self.view_mode = ViewMode::InternetRadio;
        self.status = format!("Scouring the internet sewers for '{query}'... (ง'̀-'́)ง");

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
            self.add_form.error_msg = Some("Hey Einstein, a station needs a damn NAME! (ノಠ益ಠ)ノ彡┻━┻".into());
            self.add_form.focused_field = 0;
            return;
        }
        if url.is_empty() {
            self.add_form.error_msg = Some("You forgot the stream URL, genius. What are we playing, silence? ಠ_ಠ".into());
            self.add_form.focused_field = 1;
            return;
        }

        let final_genre = if genre.is_empty() { "CUSTOM".into() } else { genre.to_uppercase() };
        let final_desc = if desc.is_empty() { "Sketchy user-injected audio pipeline".into() } else { desc };

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

        self.status = format!("Injected '{name}' into your Vault! Blast it! (ง'̀-'́)ง");
        self.input_mode = InputMode::Normal;
        self.add_form = AddStationForm::default();
        self.view_mode = ViewMode::Library;
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
    if !Command::new("mpv")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        eprintln!("HAK.FM needs mpv, you doofus! (╯°□°)╯︵ ┻━┻\nInstall it on Arch: sudo pacman -S mpv\nOr Debian/Ubuntu: sudo apt install mpv");
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
                            "0 results for '{}'. Did your cat walk on the keyboard? ¯\\_(ツ)_/¯",
                            app.ir_query
                        );
                    } else {
                        app.status = format!(
                            "Plundered {} stations for '{}'. Pick your poison! (⌐■_■)",
                            count, app.ir_query
                        );
                    }
                }
                Err(err) => {
                    app.status = format!("Search exploded: {err} (╯°□°)╯︵ ┻━┻");
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
                            app.status = "Ragequit aborted! Back to the noise, badass (⌐■_■)".into();
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
                            app.status = "Mercilessly slaughtered playback. [x_x] (¬_¬)".into();
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
                            app.status = "Wiped the slate clean. Back to square one. ¯\\_(ツ)_/¯".into();
                        }
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
                app.status = "Stream flatlined or mpv committed seppuku. [x_x] (;´༎ຶД༎ຶ`)".into();
            }
        }
    }
    Ok(())
}

fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    f.render_widget(Block::default().style(Style::default().bg(BG)), area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Header & Tabs
            Constraint::Length(4), // Now Playing & Waveform
            Constraint::Min(8),    // Body (Left: list, Right: details)
            Constraint::Length(3), // Keys / Shortcuts
            Constraint::Length(2), // Footer & Delta Fsociety link
        ])
        .split(area);

    // 1. Header with View Tabs
    let lib_active = app.view_mode == ViewMode::Library;
    let ir_active = app.view_mode == ViewMode::InternetRadio;

    let header_line1 = Line::from(vec![
        Span::styled(" HAK", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
        Span::styled(".FM", Style::default().fg(TEXT).add_modifier(Modifier::BOLD)),
        Span::styled("  // NOISE PROTOCOL ", Style::default().fg(MUTED)),
        Span::styled("●", Style::default().fg(if app.player.is_some() { GREEN } else { RED })),
        Span::styled(
            if app.player.is_some() { " ON AIR (ง'̀-'́)ง  " } else { " DEAD AIR [x_x]  " },
            Style::default().fg(if app.player.is_some() { GREEN } else { MUTED }).add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled(
            " [1: THE VAULT] ",
            if lib_active {
                Style::default().fg(GREEN).bg(Color::Rgb(20, 45, 38)).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(MUTED)
            },
        ),
        Span::raw(" "),
        Span::styled(
            " [2: WEB PIRACY] ",
            if ir_active {
                Style::default().fg(CYAN).bg(Color::Rgb(15, 35, 45)).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(MUTED)
            },
        ),
    ]);

    let header_line2 = Line::from(vec![
        Span::styled(
            " terminal noise injector for code gremlins & caffeine addicts (⌐■_■)",
            Style::default().fg(MUTED),
        ),
    ]);

    let header = Paragraph::new(vec![header_line1, header_line2]).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::Rgb(30, 55, 60))),
    );
    f.render_widget(header, chunks[0]);

    // 2. Now Playing Banner & Audio Waveform (its just a lil animation so it doesnt take so much process power)
    let now = app.current_station.as_ref();
    let elapsed = app.elapsed().as_secs();
    let status_text = if let Some(station) = now {
        let tag = match station.source {
            StationSource::Preset => "PRESET-LOOT",
            StationSource::Custom => "ILLEGAL-FEED",
            StationSource::InternetRadio => "WEB-PIRACY",
        };
        format!(
            "  [{tag}] {}   //   {}   //   {:02}:{:02}   //   BLASTING YOUR EARDRUMS (ง'̀-'́)ง",
            station.name,
            station.genre,
            elapsed / 60,
            elapsed % 60
        )
    } else if app.paused {
        "  Paused like a coward... Press [SPACE] to wake up, lazybones! ¯\\_(ツ)_/¯".to_string()
    } else {
        "  Dead silence. Pick a damn station and slam [ENTER] already! (╯°□°)╯".to_string()
    };

    let phase = app
        .started
        .map(|started| started.elapsed().as_millis() as f64 / 170.0)
        .unwrap_or(0.0);
    let wave_width = chunks[1].width.saturating_sub(4) as usize;
    let wave = wave_line(wave_width, phase, app.player.is_some() && !app.paused);

    let now_widget = Paragraph::new(vec![
        Line::from(Span::styled(
            status_text,
            Style::default()
                .fg(if app.player.is_some() { GREEN } else { MUTED })
                .bg(PANEL)
                .add_modifier(Modifier::BOLD),
        )),
        wave,
    ])
    .style(Style::default().bg(PANEL))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if app.player.is_some() { GREEN } else { MUTED })),
    );
    f.render_widget(now_widget, chunks[1]);

    // 3. Body: Left List & Right Details
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(chunks[2]);

    match app.view_mode {
        ViewMode::Library => {
            let title = if app.show_favorites {
                " STATIONS // YOUR ADDICTIONS [★] "
            } else {
                " STATIONS // THE UNDERGROUND VAULT "
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
                        StationSource::Custom => "[CUSTOM] ",
                        StationSource::InternetRadio => "[PIRATED] ",
                    };

                    let name_display = format!("{:<22}", station.name);
                    let line = Line::from(vec![
                        Span::styled(
                            prefix,
                            Style::default().fg(if is_current { GREEN } else { CYAN }),
                        ),
                        Span::styled(
                            source_tag,
                            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            name_display,
                            Style::default()
                                .fg(if is_current { GREEN } else { TEXT })
                                .add_modifier(if is_current {
                                    Modifier::BOLD
                                } else {
                                    Modifier::empty()
                                }),
                        ),
                        Span::styled(station.genre.clone(), Style::default().fg(MUTED)),
                    ]);
                    ListItem::new(line)
                })
                .collect();

            let list_block = Block::default()
                .title(format!("{title}[{}] 凸(¬‿¬)凸 ", app.filtered.len()))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(CYAN))
                .style(Style::default().bg(PANEL));

            let list = List::new(station_items)
                .block(list_block)
                .highlight_style(
                    Style::default()
                        .fg(GREEN)
                        .bg(Color::Rgb(20, 40, 35))
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("› ");

            let mut state = ListState::default();
            if !app.filtered.is_empty() {
                state.select(Some(app.selected));
            }
            f.render_stateful_widget(list, body[0], &mut state);

            // Right column for Library
            let right = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(8), Constraint::Min(5)])
                .split(body[1]);

            let selected_station = app
                .filtered
                .get(app.selected)
                .and_then(|&i| app.stations.get(i));

            let details = if let Some(station) = selected_station {
                let source_label = match station.source {
                    StationSource::Preset => "IMMORTAL PRESET (can't touch this 凸(¬‿¬)凸)",
                    StationSource::Custom => "USER JUNK · Hit 'd' to incinerate [x_x]",
                    StationSource::InternetRadio => "LOOTED FROM INTERNET-RADIO.COM",
                };
                vec![
                    Line::from(Span::styled(
                        "TARGET FREQUENCY DOSSIER",
                        Style::default().fg(MUTED).add_modifier(Modifier::BOLD),
                    )),
                    Line::from(Span::styled(
                        &station.name,
                        Style::default().fg(GREEN).add_modifier(Modifier::BOLD),
                    )),
                    Line::from(Span::styled(&station.description, Style::default().fg(TEXT))),
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("VIBE    {}", station.genre),
                        Style::default().fg(CYAN),
                    )),
                    Line::from(Span::styled(
                        format!("ORIGIN  {source_label}"),
                        Style::default().fg(MUTED),
                    )),
                ]
            } else {
                vec![Line::from("No stations match your picky filter. ¯\\_(ツ)_/¯")]
            };

            f.render_widget(
                Paragraph::new(details).wrap(Wrap { trim: true }).block(
                    Block::default()
                        .title(" STATION INFO ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Rgb(45, 80, 75)))
                        .style(Style::default().bg(PANEL)),
                ),
                right[0],
            );

            let search_label = if app.input_mode == InputMode::Filter {
                format!(" SNIFFING > {}_ (ง'̀-'́)ง", app.search)
            } else if !app.search.is_empty() {
                format!(" ACTIVE SNIFF // {}", app.search)
            } else {
                " PIRATE COMMAND CENTER (⌐■_■)".into()
            };

            let notes = vec![
                Line::from(Span::styled(
                    search_label,
                    Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "• Hit 'a' to force-feed your own custom audio feed.",
                    Style::default().fg(TEXT),
                )),
                Line::from(Span::styled(
                    "• Hit 'i' to loot internet-radio.com for weird frequencies.",
                    Style::default().fg(CYAN),
                )),
                Line::from(Span::styled(
                    "• Hit 'd' to nuke custom stations into oblivion [x_x].",
                    Style::default().fg(MUTED),
                )),
                Line::from(Span::styled(
                    "• Hit '?' if your brain cells failed you and you forgot keys.",
                    Style::default().fg(YELLOW),
                )),
                Line::from(""),
                Line::from(Span::styled("LAST SCREAM:", Style::default().fg(MUTED))),
                Line::from(Span::styled(
                    &app.status,
                    Style::default().fg(if app.status.contains("choked")
                        || app.status.contains("slaughtered")
                        || app.status.contains("flatlined")
                        || app.status.contains("failed")
                        || app.status.contains("died")
                    {
                        RED
                    } else {
                        GREEN
                    }),
                )),
            ];

            f.render_widget(
                Paragraph::new(notes).wrap(Wrap { trim: true }).block(
                    Block::default()
                        .title(" ACTIVITY ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Rgb(45, 80, 75)))
                        .style(Style::default().bg(PANEL)),
                ),
                right[1],
            );
        }

        ViewMode::InternetRadio => {
            let spinner_frame = SPINNER[app.spinner_tick % SPINNER.len()];
            let title = if app.ir_loading {
                format!(" INTERNET-RADIO.COM // {} SCRAPING THE SEWERS... ", spinner_frame)
            } else if app.ir_query.is_empty() {
                " INTERNET-RADIO.COM // (Press 'i' or stay ignorant >:-) ".into()
            } else {
                format!(
                    " INTERNET-RADIO.COM // LOOT FOR '{}' [{}] (⌐■_■) ",
                    app.ir_query,
                    app.ir_stations.len()
                )
            };

            let station_items: Vec<ListItem> = if app.ir_loading {
                vec![ListItem::new(Line::from(Span::styled(
                    format!("  {} Scraping internet-radio.com for '{}'...", spinner_frame, app.ir_query),
                    Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
                )))]
            } else if app.ir_stations.is_empty() {
                vec![
                    ListItem::new(Line::from("")),
                    ListItem::new(Line::from(Span::styled(
                        "  No stations loaded. The web gave you the cold shoulder. ¯\\_(ツ)_/¯",
                        Style::default().fg(MUTED),
                    ))),
                    ListItem::new(Line::from(Span::styled(
                        "  Press 'i' and search something that actually exists, genius!",
                        Style::default().fg(CYAN),
                    ))),
                ]
            } else {
                app.ir_stations
                    .iter()
                    .enumerate()
                    .map(|(_, station)| {
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

                        let name_display = format!("{:<25}", station.name);
                        let line = Line::from(vec![
                            Span::styled(
                                prefix,
                                Style::default().fg(if is_current { GREEN } else { CYAN }),
                            ),
                            Span::styled(
                                name_display,
                                Style::default()
                                    .fg(if is_current { GREEN } else { TEXT })
                                    .add_modifier(if is_current {
                                        Modifier::BOLD
                                    } else {
                                        Modifier::empty()
                                    }),
                        ),
                        Span::styled(station.genre.clone(), Style::default().fg(MUTED)),
                    ]);
                    ListItem::new(line)
                })
                .collect()
            };

            let list_block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(CYAN))
                .style(Style::default().bg(PANEL));

            let list = List::new(station_items)
                .block(list_block)
                .highlight_style(
                    Style::default()
                        .fg(GREEN)
                        .bg(Color::Rgb(20, 40, 35))
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("› ");

            let mut state = ListState::default();
            if !app.ir_stations.is_empty() {
                state.select(Some(app.ir_selected));
            }
            f.render_stateful_widget(list, body[0], &mut state);

            // Right column for Internet-Radio
            let right = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(9), Constraint::Min(5)])
                .split(body[1]);

            let selected_station = app.ir_stations.get(app.ir_selected);

            let details = if let Some(station) = selected_station {
                let in_lib = app.stations.iter().any(|s| s.name == station.name);
                let lib_hint = if in_lib {
                    "ALREADY HOARDED IN YOUR VAULT (✓)"
                } else {
                    "Press 'a' or 's' to kidnap into your Vault!"
                };

                vec![
                    Line::from(Span::styled(
                        "INSPECTING SKETCHY WEB FREQUENCY",
                        Style::default().fg(MUTED).add_modifier(Modifier::BOLD),
                    )),
                    Line::from(Span::styled(
                        &station.name,
                        Style::default().fg(GREEN).add_modifier(Modifier::BOLD),
                    )),
                    Line::from(Span::styled(&station.description, Style::default().fg(TEXT))),
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("VIBES    {}", station.genre),
                        Style::default().fg(CYAN),
                    )),
                    Line::from(Span::styled(
                        format!("STATUS   {lib_hint}"),
                        Style::default().fg(if in_lib { GREEN } else { YELLOW }),
                    )),
                ]
            } else {
                vec![
                    Line::from("No station selected. Quit daydreaming. ಠ_ಠ"),
                    Line::from("Press 'i' to loot internet-radio.com"),
                ]
            };

            f.render_widget(
                Paragraph::new(details).wrap(Wrap { trim: true }).block(
                    Block::default()
                        .title(" STATION DETAILS ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Rgb(45, 80, 75)))
                        .style(Style::default().bg(PANEL)),
                ),
                right[0],
            );

            let notes = vec![
                Line::from(Span::styled(
                    " DEEP WEB RECONNAISSANCE (⌐■_■)",
                    Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "• Slam [Enter] to blast this unverified noise stream.",
                    Style::default().fg(TEXT),
                )),
                Line::from(Span::styled(
                    "• Hit 'a' to kidnap this station into your Vault.",
                    Style::default().fg(GREEN),
                )),
                Line::from(Span::styled(
                    "• Hit 'i' to raid internet-radio.com for more booty.",
                    Style::default().fg(CYAN),
                )),
                Line::from(Span::styled(
                    "• Hit 'Tab' to retreat to your safe library cave.",
                    Style::default().fg(MUTED),
                )),
                Line::from(""),
                Line::from(Span::styled("LAST SCREAM:", Style::default().fg(MUTED))),
                Line::from(Span::styled(
                    &app.status,
                    Style::default().fg(if app.status.contains("choked")
                        || app.status.contains("slaughtered")
                        || app.status.contains("flatlined")
                        || app.status.contains("failed")
                        || app.status.contains("died")
                    {
                        RED
                    } else {
                        GREEN
                    }),
                )),
            ];

            f.render_widget(
                Paragraph::new(notes).wrap(Wrap { trim: true }).block(
                    Block::default()
                        .title(" ACTIVITY ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Rgb(45, 80, 75)))
                        .style(Style::default().bg(PANEL)),
                ),
                right[1],
            );
        }
    }

    // 4. Shortcut Keys Bar
    let keys_spans = match app.view_mode {
        ViewMode::Library => vec![
            Span::styled(" ↑/↓ ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("stalk  ", Style::default().fg(MUTED)),
            Span::styled("ENTER ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("blast  ", Style::default().fg(MUTED)),
            Span::styled("SPACE ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("chill  ", Style::default().fg(MUTED)),
            Span::styled("i ", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("pillage-web  ", Style::default().fg(MUTED)),
            Span::styled("a ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("inject  ", Style::default().fg(MUTED)),
            Span::styled("/ ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("sniff  ", Style::default().fg(MUTED)),
            Span::styled("s ", Style::default().fg(YELLOW).add_modifier(Modifier::BOLD)),
            Span::styled("simp  ", Style::default().fg(MUTED)),
            Span::styled("d ", Style::default().fg(RED).add_modifier(Modifier::BOLD)),
            Span::styled("nuke  ", Style::default().fg(MUTED)),
            Span::styled("Tab ", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("switch  ", Style::default().fg(MUTED)),
            Span::styled("? ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("clues  ", Style::default().fg(MUTED)),
            Span::styled("q ", Style::default().fg(RED).add_modifier(Modifier::BOLD)),
            Span::styled("ragequit", Style::default().fg(MUTED)),
        ],
        ViewMode::InternetRadio => vec![
            Span::styled(" ↑/↓ ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("stalk  ", Style::default().fg(MUTED)),
            Span::styled("ENTER ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("blast  ", Style::default().fg(MUTED)),
            Span::styled("SPACE ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("chill  ", Style::default().fg(MUTED)),
            Span::styled("a ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("kidnap  ", Style::default().fg(MUTED)),
            Span::styled("i ", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("hunt  ", Style::default().fg(MUTED)),
            Span::styled("s ", Style::default().fg(YELLOW).add_modifier(Modifier::BOLD)),
            Span::styled("simp  ", Style::default().fg(MUTED)),
            Span::styled("Tab ", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
            Span::styled("vault  ", Style::default().fg(MUTED)),
            Span::styled("? ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("clues  ", Style::default().fg(MUTED)),
            Span::styled("q ", Style::default().fg(RED).add_modifier(Modifier::BOLD)),
            Span::styled("ragequit", Style::default().fg(MUTED)),
        ],
    };

    let keys = Paragraph::new(Line::from(keys_spans)).alignment(Alignment::Center).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::Rgb(30, 55, 60))),
    );
    f.render_widget(keys, chunks[3]);

    // 5. Footer with Author Credit
    draw_footer(f, chunks[4]);

    // 6. Overlaid Modals
    if app.confirm_quit {
        draw_quit_modal(f, area);
    } else if app.input_mode == InputMode::InternetRadioSearch {
        draw_ir_search_modal(f, &app.ir_search_input, area);
    } else if app.input_mode == InputMode::AddStation {
        draw_add_station_modal(f, &app.add_form, area);
    } else if app.show_help {
        draw_help_modal(f, area);
    }
}


// Modal Dialogs & Footer

fn draw_quit_modal(f: &mut Frame, area: Rect) {
    let popup = centered_rect(65, 45, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" [!] RAGEQUIT DETECTED?! (╯°□°)╯︵ ┻━┻ ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(RED).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(PANEL));
    f.render_widget(block, popup);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2), // Headline
            Constraint::Length(2), // Subtext
            Constraint::Length(1), // Emoji banner
            Constraint::Length(1), // Blank
            Constraint::Length(2), // Action buttons
        ])
        .split(popup);

    let headline = Paragraph::new(vec![
        Line::from(Span::styled(
            " Whoa, hold up! Are you seriously bailing out right now?! (ง'̀-'́)ง",
            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            " Your eardrums will starve and your code will get even buggier.",
            Style::default().fg(TEXT),
        )),
    ]);
    f.render_widget(headline, inner[0]);

    let body = Paragraph::new(vec![
        Line::from(Span::styled(
            " Are you really sure you want to run away like a coward? ¯\\_(ツ)_/¯",
            Style::default().fg(MUTED),
        )),
    ]);
    f.render_widget(body, inner[1]);

    let art = Paragraph::new(Line::from(vec![
        Span::styled("   (•_•)  ", Style::default().fg(MUTED)),
        Span::styled("( •_•)>⌐■-■  ", Style::default().fg(CYAN)),
        Span::styled("(⌐■_■) DEAL WITH THE NOISE", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(art, inner[2]);

    let actions = Paragraph::new(Line::from(vec![
        Span::styled(" [Y/Enter] ", Style::default().fg(RED).add_modifier(Modifier::BOLD)),
        Span::styled("Yeah, I suck, let me ragequit (ಥ﹏ಥ)   ", Style::default().fg(RED)),
        Span::styled(" [N/Esc/q] ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
        Span::styled("Hell no, keep rocking! (ง'̀-'́)ง", Style::default().fg(GREEN)),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(actions, inner[4]);
}

fn draw_footer(f: &mut Frame, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let sub = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    // Line 1: Program info
    let line1 = Paragraph::new(Line::from(vec![
        Span::styled("HAK.FM", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
        Span::styled("  ·  Cyberpunk Noise Transmitter & Audio Syndicate ", Style::default().fg(Color::Rgb(50, 90, 80))),
        Span::styled("凸(¬‿¬)凸", Style::default().fg(YELLOW)),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(line1, sub[0]);

    // Line 2: Author name + contact (rendered properly without cell diff bugs)
    if sub.len() > 1 {
        let line2 = if area.width >= 90 {
            Paragraph::new(Line::from(vec![
                Span::styled("Forged with caffeine & spite by ", Style::default().fg(Color::Rgb(75, 115, 105))),
                Span::styled(
                    AUTHOR_NAME,
                    Style::default()
                        .fg(CYAN)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ),
                Span::styled(" · ", Style::default().fg(MUTED)),
                Span::styled(
                    AUTHOR_URL,
                    Style::default()
                        .fg(Color::Rgb(80, 160, 150))
                        .add_modifier(Modifier::UNDERLINED),
                ),
                Span::styled(" (⌐■_■)", Style::default().fg(GREEN)),
            ]))
        } else {
            Paragraph::new(Line::from(vec![
                Span::styled("Forged by ", Style::default().fg(Color::Rgb(75, 115, 105))),
                Span::styled(
                    AUTHOR_NAME,
                    Style::default()
                        .fg(CYAN)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ),
                Span::styled(" · zist.digital (⌐■_■)", Style::default().fg(Color::Rgb(80, 160, 150))),
            ]))
        };
        f.render_widget(line2.alignment(Alignment::Center), sub[1]);
    }
}

fn draw_ir_search_modal(f: &mut Frame, query: &str, area: Rect) {
    let popup = centered_rect(65, 30, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" SEARCH HTTPS://WWW.INTERNET-RADIO.COM/ ( ͡° ͜ʖ ͡°) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(CYAN))
        .style(Style::default().bg(PANEL));
    f.render_widget(block, popup);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(2),
        ])
        .split(popup);

    let prompt = Paragraph::new(Span::styled(
        "Spill a keyword, genre, or vibe before I lose my patience (e.g. ambient, cyberpunk, synthwave):",
        Style::default().fg(TEXT),
    ));
    f.render_widget(prompt, inner[0]);

    let input_box = Paragraph::new(format!(" {query}_"))
        .style(Style::default().fg(GREEN).add_modifier(Modifier::BOLD))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(CYAN))
                .style(Style::default().bg(Color::Rgb(10, 16, 22))),
        );
    f.render_widget(input_box, inner[1]);

    let hints = Paragraph::new(vec![Line::from(vec![
        Span::styled(" [ENTER] ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
        Span::styled("Unleash hounds   ", Style::default().fg(MUTED)),
        Span::styled(" [ESC] ", Style::default().fg(RED).add_modifier(Modifier::BOLD)),
        Span::styled("Chicken out ¯\\_(ツ)_/¯", Style::default().fg(MUTED)),
    ])]);
    f.render_widget(hints, inner[2]);
}

fn draw_add_station_modal(f: &mut Frame, form: &AddStationForm, area: Rect) {
    let popup = centered_rect(68, 60, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" INJECT SKETCHY CUSTOM STREAM (ง'̀-'́)ง ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(GREEN))
        .style(Style::default().bg(PANEL));
    f.render_widget(block, popup);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3), // Name
            Constraint::Length(3), // URL
            Constraint::Length(3), // Genre
            Constraint::Length(3), // Description
            Constraint::Length(2), // Error or info
            Constraint::Length(1), // Key hints
        ])
        .split(popup);

    // Field 0: Name
    let f0_active = form.focused_field == 0;
    let p0 = Paragraph::new(format!(
        " {}{}",
        form.name,
        if f0_active { "_" } else { "" }
    ))
    .block(
        Block::default()
            .title(" Station Moniker * (give it a name or else ಠ_ಠ) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if f0_active { GREEN } else { MUTED }))
            .style(Style::default().bg(Color::Rgb(10, 16, 22))),
    );
    f.render_widget(p0, rows[0]);

    // Field 1: Stream URL
    let f1_active = form.focused_field == 1;
    let p1 = Paragraph::new(format!(
        " {}{}",
        form.url,
        if f1_active { "_" } else { "" }
    ))
    .block(
        Block::default()
            .title(" Stream Pipeline URL * (.mp3 / .pls / .m3u / icecast / raw noise) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if f1_active { GREEN } else { MUTED }))
            .style(Style::default().bg(Color::Rgb(10, 16, 22))),
    );
    f.render_widget(p1, rows[1]);

    // Field 2: Genre
    let f2_active = form.focused_field == 2;
    let p2 = Paragraph::new(format!(
        " {}{}",
        form.genre,
        if f2_active { "_" } else { "" }
    ))
    .block(
        Block::default()
            .title(" Vibe / Genre (whatever nonsense tag you like, default: CUSTOM) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if f2_active { GREEN } else { MUTED }))
            .style(Style::default().bg(Color::Rgb(10, 16, 22))),
    );
    f.render_widget(p2, rows[2]);

    // Field 3: Description
    let f3_active = form.focused_field == 3;
    let p3 = Paragraph::new(format!(
        " {}{}",
        form.description,
        if f3_active { "_" } else { "" }
    ))
    .block(
        Block::default()
            .title(" Description (bullshit excuse for listening to this) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if f3_active { GREEN } else { MUTED }))
            .style(Style::default().bg(Color::Rgb(10, 16, 22))),
    );
    f.render_widget(p3, rows[3]);

    // Error or guide message
    let msg = if let Some(err) = &form.error_msg {
        Line::from(Span::styled(
            format!(" ⚠ {err}"),
            Style::default().fg(RED).add_modifier(Modifier::BOLD),
        ))
    } else {
        Line::from(Span::styled(
            " Saving this biohazard payload to ~/.config/hakfm/config.json",
            Style::default().fg(MUTED),
        ))
    };
    f.render_widget(Paragraph::new(msg), rows[4]);

    // Key hints
    let hints = Line::from(vec![
        Span::styled(" [Tab/↓] ", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
        Span::styled("Next torture field  ", Style::default().fg(MUTED)),
        Span::styled(" [Enter] ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
        Span::styled("Inject into matrix  ", Style::default().fg(MUTED)),
        Span::styled(" [Esc] ", Style::default().fg(RED).add_modifier(Modifier::BOLD)),
        Span::styled("Chicken out ¯\\_(ツ)_/¯", Style::default().fg(MUTED)),
    ]);
    f.render_widget(Paragraph::new(hints), rows[5]);
}

fn draw_help_modal(f: &mut Frame, area: Rect) {
    let popup = centered_rect(70, 85, area);
    f.render_widget(Clear, popup);
    f.render_widget(
        Block::default()
            .title(" SURVIVAL MANUAL & KEYBINDINGS (FOR NOOBS & CHAMPS) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(GREEN))
            .style(Style::default().bg(PANEL)),
        popup,
    );

    let help = Paragraph::new(vec![
        Line::from(Span::styled(
            "How to pilot this chaotic noise machine without crying (ง'̀-'́)ง",
            Style::default().fg(GREEN).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("↑ / k       Stalk up (Vim keys for real hackers, arrows for noobs)"),
        Line::from("↓ / j       Stalk down (respect the Vim supremacy)"),
        Line::from("Enter       Blast the selected noise transmitter into your skull"),
        Line::from("Space       Toggle chill / blast (or enjoy sweet, sweet silence)"),
        Line::from("← / h       Previous frequency (instant ear damage)"),
        Line::from("→ / l       Next frequency (keep rolling the dice)"),
        Line::from("Tab / 1 / 2 Flip between [1: The Vault] <-> [2: Web Piracy]"),
        Line::from("i           Raid internet-radio.com for unholy frequencies"),
        Line::from("a           Inject custom station URL (or kidnap IR station to Vault)"),
        Line::from("d / Del     Nuke user-added station into digital oblivion [x_x]"),
        Line::from("/           Sniff out stations by search query"),
        Line::from("s           Simp / Star station (save to your dirty secrets ★)"),
        Line::from("f           Filter down to your starred obsessions only"),
        Line::from("x           Mercilessly slaughter playback [x_x]"),
        Line::from("r           Wipe overcomplicated filters back to zero ¯\\_(ツ)_/¯"),
        Line::from("?           Toggle this glorious cheat-sheet"),
        Line::from("q           Ragequit like a coward (with exit confirmation warning!)"),
        Line::from(""),
        Line::from(Span::styled(
            "Supreme Mastermind & Overlord (⌐■_■)",
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  Author:   ", Style::default().fg(MUTED)),
            Span::styled(format!("{AUTHOR_NAME} (Delta)"), Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("  ¯\\_(ツ)_/¯", Style::default().fg(YELLOW)),
        ]),
        Line::from(vec![
            Span::styled("  Contact:  ", Style::default().fg(MUTED)),
            Span::styled(AUTHOR_URL, Style::default().fg(CYAN).add_modifier(Modifier::UNDERLINED)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Requires mpv installed and internet. Don't come crying if your WiFi sucks. 凸(¬‿¬)凸",
            Style::default().fg(MUTED),
        )),
    ])
    .block(Block::default().padding(ratatui::widgets::Padding::new(2, 2, 1, 1)))
    .wrap(Wrap { trim: false });
    f.render_widget(help, popup);
}

fn wave_line(width: usize, phase: f64, active: bool) -> Line<'static> {
    if width == 0 {
        return Line::from("");
    }

    if !active {
        return Line::from(Span::styled("─".repeat(width), Style::default().fg(MUTED)));
    }

    const BARS: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
    let mut spans = Vec::with_capacity(width);
    for i in 0..width {
        let x = i as f64;
        let t = phase - x * 0.24;
        let wave =
            (t.sin() * 0.52 + (t * 0.47 + 1.2).sin() * 0.31 + (t * 0.19).sin() * 0.17).abs();
        let height = ((wave * 7.0).round() as usize).min(BARS.len() - 1);
        let color = if height >= 5 { GREEN } else { CYAN };
        spans.push(Span::styled(BARS[height], Style::default().fg(color)));
    }
    Line::from(spans)
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

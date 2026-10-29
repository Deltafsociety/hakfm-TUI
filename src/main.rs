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
    fs,
    io,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

const BG: Color = Color::Rgb(8, 12, 16);
const PANEL: Color = Color::Rgb(14, 21, 27);
const GREEN: Color = Color::Rgb(0, 255, 153);
const MUTED: Color = Color::Rgb(105, 130, 140);
const TEXT: Color = Color::Rgb(220, 235, 235);
const CYAN: Color = Color::Rgb(60, 210, 220);
const RED: Color = Color::Rgb(255, 90, 105);

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Station {
    name: String,
    genre: String,
    description: String,
    url: String,
}

fn stations() -> Vec<Station> {
    // Direct SomaFM MP3 streams. These are broadcaster stream URLs, not HAK.FM APIs.
    let rows = [
        ("DEF CON Radio", "HACKER", "The year-round DEF CON channel.", "defcon"),
        ("Groove Salad", "FOCUS", "Downtempo grooves for long sessions.", "groovesalad"),
        ("Drone Zone", "AMBIENT", "Slow atmospheric soundscapes.", "dronezone"),
        ("Deep Space One", "AMBIENT", "Deep ambient for quiet work.", "deepspaceone"),
        ("The Dark Zone", "DARK AMBIENT", "The darker end of deep ambient.", "darkzone"),
        ("Doomed", "INDUSTRIAL", "Dark industrial and ambient.", "doomed"),
        ("cliqhop idm", "IDM", "Blips, beeps, and intricate beats.", "cliqhop"),
        ("Groove Salad Classic", "DOWNTEMPO", "Classic early-2000s chill.", "gsclassic"),
        ("Space Station Soma", "SPACE", "Spaced-out ambient electronica.", "spacestation"),
        ("Mission Control", "SPACE", "Mission audio woven into electronica.", "missioncontrol"),
        ("Vaporwaves", "VAPORWAVE", "A hazy vaporwave signal.", " vaporwaves"),
        ("Underground 80s", "SYNTH / NEW WAVE", "Early-80s synthpop and new wave.", "u80s"),
        ("Metal Detector", "METAL", "From black metal to doom and thrash.", "metal"),
        ("Dub Step Beyond", "BASS", "Dubstep, dub, and deep bass.", "dubstep"),
        ("Secret Agent", "SPY JAZZ", "Spy jazz and lounge atmosphere.", " secretagent"),
        ("Sonic Universe", "JAZZ", "Modern jazz and fusion.", "sonicuniverse"),
        ("Beat Blender", "ELECTRONIC", "Downtempo and electronic grooves.", "beatblender"),
        ("The Trip", "PSYCHEDELIC", "Psychedelic electronic journeys.", "thetrip"),
    ];
    rows.iter().map(|(name, genre, description, slug)| {
        let slug = slug.trim();
        Station {
            name: (*name).to_string(),
            genre: (*genre).to_string(),
            description: (*description).to_string(),
            url: format!("https://ice5.somafm.com/{slug}-128-mp3"),
        }
    }).collect()
}

#[derive(Default, Serialize, Deserialize)]
struct Config {
    favorites: Vec<String>,
    last_station: Option<String>,
    volume: u8,
}

fn config_path() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from("."))
        .join("hakfm").join("config.json")
}

fn load_config() -> Config {
    fs::read_to_string(config_path()).ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Config { volume: 65, ..Config::default() })
}

fn save_config(config: &Config) {
    let path = config_path();
    if let Some(parent) = path.parent() { let _ = fs::create_dir_all(parent); }
    if let Ok(data) = serde_json::to_string_pretty(config) { let _ = fs::write(path, data); }
}

struct App {
    stations: Vec<Station>,
    filtered: Vec<usize>,
    selected: usize,
    current: Option<usize>,
    config: Config,
    search_mode: bool,
    search: String,
    status: String,
    player: Option<Child>,
    paused: bool,
    started: Option<Instant>,
    elapsed_base: Duration,
    show_help: bool,
    show_favorites: bool,
    should_quit: bool,
}

impl App {
    fn new() -> Self {
        let stations = stations();
        let config = load_config();
        let mut app = Self {
            stations, filtered: Vec::new(), selected: 0, current: None, config,
            search_mode: false, search: String::new(),
            status: "Ready. Pick a station.".into(),
            player: None, paused: false, started: None,
            elapsed_base: Duration::ZERO, show_help: false, show_favorites: false,
            should_quit: false,
        };
        app.rebuild_filter();
        app
    }

    fn rebuild_filter(&mut self) {
        let query = self.search.to_lowercase();
        self.filtered = self.stations.iter().enumerate().filter(|(_, s)| {
            let favorite_ok = !self.show_favorites || self.config.favorites.contains(&s.name);
            let search_ok = query.is_empty()
                || s.name.to_lowercase().contains(&query)
                || s.genre.to_lowercase().contains(&query)
                || s.description.to_lowercase().contains(&query);
            favorite_ok && search_ok
        }).map(|(i, _)| i).collect();
        if self.filtered.is_empty() { self.selected = 0; }
        else if self.selected >= self.filtered.len() { self.selected = self.filtered.len() - 1; }
    }

    fn play_selected(&mut self) {
        if self.filtered.is_empty() { self.status = "No stations match that search.".into(); return; }
        let idx = self.filtered[self.selected];
        self.play_station(idx);
    }

    fn play_station(&mut self, idx: usize) {
        self.stop_player();
        let station = &self.stations[idx];
        let url = station.url.clone();
        let name = station.name.clone();
        let child = Command::new("mpv")
            .args(["--no-video", "--really-quiet", "--force-window=no", "--term-status-msg="])
            .arg(url)
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
            .spawn();
        match child {
            Ok(child) => {
                self.player = Some(child);
                self.current = Some(idx);
                self.paused = false;
                self.started = Some(Instant::now());
                self.elapsed_base = Duration::ZERO;
                self.config.last_station = Some(name.clone());
                self.status = format!("Playing {name}");
                save_config(&self.config);
            }
            Err(e) => self.status = format!("Could not start mpv: {e}"),
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
        // Keep the implementation dependency-light: pause/resume is achieved by restarting
        // the selected station. mpv IPC can be added later for seamless pause semantics.
        if let Some(idx) = self.current {
            if self.paused {
                self.play_station(idx);
            } else {
                self.stop_player();
                self.paused = true;
                self.status = "Stopped. Press Space to resume.".into();
            }
        } else {
            self.status = "Choose a station first.".into();
        }
    }

    fn next(&mut self, delta: isize) {
        if self.filtered.is_empty() { return; }
        let len = self.filtered.len() as isize;
        self.selected = (self.selected as isize + delta).rem_euclid(len) as usize;
        self.play_selected();
    }

    fn toggle_favorite(&mut self) {
        if self.filtered.is_empty() { return; }
        let name = self.stations[self.filtered[self.selected]].name.clone();
        if let Some(pos) = self.config.favorites.iter().position(|n| n == &name) {
            self.config.favorites.remove(pos);
            self.status = format!("Removed {name} from favorites.");
        } else {
            self.config.favorites.push(name.clone());
            self.status = format!("Added {name} to favorites.");
        }
        save_config(&self.config);
        self.rebuild_filter();
    }

    fn elapsed(&self) -> Duration {
        if self.paused { self.elapsed_base }
        else { self.started.map(|s| self.elapsed_base + s.elapsed()).unwrap_or(self.elapsed_base) }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if !Command::new("mpv").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false) {
        eprintln!("HAK.FM needs mpv. Install it on Arch with: sudo pacman -S mpv");
        std::process::exit(1);
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, crossterm::event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new();

    let result = run_app(&mut terminal, &mut app);

    app.stop_player();
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, crossterm::event::DisableMouseCapture)?;
    terminal.show_cursor()?;
    result?;
    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| draw(f, app))?;
        if app.should_quit { break; }
        if event::poll(Duration::from_millis(150))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press { continue; }
                if app.search_mode {
                    match key.code {
                        KeyCode::Esc => { app.search_mode = false; app.search.clear(); app.rebuild_filter(); }
                        KeyCode::Enter => app.search_mode = false,
                        KeyCode::Backspace => { app.search.pop(); app.rebuild_filter(); }
                        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => { app.search.push(c); app.rebuild_filter(); }
                        _ => {}
                    }
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') => app.should_quit = true,
                    KeyCode::Char('?') => app.show_help = !app.show_help,
                    KeyCode::Esc => { app.show_help = false; app.search.clear(); app.show_favorites = false; app.rebuild_filter(); }
                    KeyCode::Char('/') => { app.search_mode = true; app.search.clear(); app.rebuild_filter(); }
                    KeyCode::Char('f') => { app.show_favorites = !app.show_favorites; app.rebuild_filter(); }
                    KeyCode::Char('s') => app.toggle_favorite(),
                    KeyCode::Char(' ') => app.toggle_pause(),
                    KeyCode::Char('x') => { app.stop_player(); app.current = None; app.status = "Playback stopped.".into(); }
                    KeyCode::Enter => app.play_selected(),
                    KeyCode::Down | KeyCode::Char('j') => {
                        if !app.filtered.is_empty() { app.selected = (app.selected + 1).min(app.filtered.len() - 1); }
                    }
                    KeyCode::Up | KeyCode::Char('k') => { app.selected = app.selected.saturating_sub(1); }
                    KeyCode::Right | KeyCode::Char('l') => app.next(1),
                    KeyCode::Left | KeyCode::Char('h') => app.next(-1),
                    KeyCode::Char('r') => {
                        app.search.clear(); app.show_favorites = false; app.rebuild_filter();
                        app.status = "Filters cleared.".into();
                    }
                    _ => {}
                }
            }
        }
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
    f.render_widget(Block::default().style(Style::default().bg(BG)), area);
    let chunks = Layout::default().direction(Direction::Vertical).margin(1)
        .constraints([Constraint::Length(3), Constraint::Length(4), Constraint::Min(8), Constraint::Length(3), Constraint::Length(2)])
        .split(area);

    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(" HAK", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
            Span::styled(".FM", Style::default().fg(TEXT).add_modifier(Modifier::BOLD)),
            Span::styled("  /  RADIO", Style::default().fg(MUTED)),
            Span::raw(" "),
            Span::styled("●", Style::default().fg(if app.player.is_some() { GREEN } else { MUTED })),
            Span::styled(if app.player.is_some() { " ON AIR " } else { " IDLE " }, Style::default().fg(if app.player.is_some() { GREEN } else { MUTED })),
        ]),
        Line::from(Span::styled("internet radio for late sessions", Style::default().fg(MUTED))),
    ]).block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::Rgb(30, 55, 60))));
    f.render_widget(header, chunks[0]);

    let now = app.current.and_then(|i| app.stations.get(i));
    let elapsed = app.elapsed().as_secs();
    let status_text = if let Some(station) = now {
        format!("  {}   ·   {}   ·   {:02}:{:02}", station.name, station.genre, elapsed / 60, elapsed % 60)
    } else if app.paused {
        "  Paused · press Space to resume".to_string()
    } else {
        "  Nothing playing · choose a station and press Enter".to_string()
    };
    let phase = app.started.map(|started| started.elapsed().as_millis() as f64 / 170.0).unwrap_or(0.0);
    let wave_width = chunks[1].width.saturating_sub(4) as usize;
    let wave = wave_line(wave_width, phase, app.player.is_some() && !app.paused);
    let now_widget = Paragraph::new(vec![Line::from(Span::styled(
        status_text,
        Style::default().fg(if app.player.is_some() { GREEN } else { MUTED }).bg(PANEL).add_modifier(Modifier::BOLD),
    )), wave])
        .style(Style::default().bg(PANEL))
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(if app.player.is_some() { GREEN } else { MUTED })));
    f.render_widget(now_widget, chunks[1]);

    let body = Layout::default().direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)]).split(chunks[2]);

    let title = if app.show_favorites { " STATIONS // FAVORITES " } else { " STATIONS " };
    let station_items: Vec<ListItem> = app.filtered.iter().map(|station_idx| {
        let station = &app.stations[*station_idx];
        let is_current = app.current == Some(*station_idx);
        let is_fav = app.config.favorites.contains(&station.name);
        let prefix = if is_current { "▶ " } else if is_fav { "★ " } else { "  " };
        let line = Line::from(vec![
            Span::styled(prefix, Style::default().fg(if is_current { GREEN } else { CYAN })),
            Span::styled(format!("{:<23}", station.name), Style::default().fg(if is_current { GREEN } else { TEXT }).add_modifier(if is_current { Modifier::BOLD } else { Modifier::empty() })),
            Span::styled(station.genre.clone(), Style::default().fg(MUTED)),
        ]);
        ListItem::new(line)
    }).collect();

    let list_block = Block::default().title(format!("{title} [{}]", app.filtered.len()))
        .borders(Borders::ALL).border_style(Style::default().fg(CYAN)).style(Style::default().bg(PANEL));
    let list = List::new(station_items).block(list_block)
        .highlight_style(Style::default().fg(GREEN).bg(Color::Rgb(20, 40, 35)).add_modifier(Modifier::BOLD))
        .highlight_symbol("› ");
    let mut state = ListState::default();
    if !app.filtered.is_empty() { state.select(Some(app.selected)); }
    f.render_stateful_widget(list, body[0], &mut state);

    let right = Layout::default().direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(5)]).split(body[1]);
    let selected_station = app.filtered.get(app.selected).and_then(|i| app.stations.get(*i));
    let details = if let Some(station) = selected_station {
        vec![
            Line::from(Span::styled("SELECTED", Style::default().fg(MUTED).add_modifier(Modifier::BOLD))),
            Line::from(""),
            Line::from(Span::styled(&station.name, Style::default().fg(GREEN).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled(&station.description, Style::default().fg(TEXT))),
            Line::from(""),
            Line::from(Span::styled(format!("FORMAT  MP3 · 128 KBPS"), Style::default().fg(MUTED))),
        ]
    } else {
        vec![Line::from("No stations match this filter.")]
    };
    f.render_widget(Paragraph::new(details).wrap(Wrap { trim: true })
        .block(Block::default().title(" STATION INFO ").borders(Borders::ALL).border_style(Style::default().fg(Color::Rgb(45, 80, 75))).style(Style::default().bg(PANEL))), right[0]);

    let search_label = if app.search_mode { format!(" SEARCH > {}_", app.search) } else if !app.search.is_empty() { format!(" FILTER // {}", app.search) } else { " SEARCH / NOTES".into() };
    let notes = vec![
        Line::from(Span::styled(search_label, Style::default().fg(CYAN).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(Span::styled("A good soundtrack for getting things done.", Style::default().fg(TEXT))),
        Line::from(Span::styled("Audio streams come from SomaFM.", Style::default().fg(MUTED))),
        Line::from(""),
        Line::from(Span::styled("STATUS", Style::default().fg(MUTED))),
        Line::from(Span::styled(&app.status, Style::default().fg(if app.status.contains("Could not") || app.status.contains("ended") { RED } else { GREEN }))),
    ];
    f.render_widget(Paragraph::new(notes).wrap(Wrap { trim: true })
        .block(Block::default().title(" ACTIVITY ").borders(Borders::ALL).border_style(Style::default().fg(Color::Rgb(45, 80, 75))).style(Style::default().bg(PANEL))), right[1]);

    let keys = Paragraph::new(Line::from(vec![
        Span::styled(" ↑/↓ ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)), Span::styled("navigate  ", Style::default().fg(MUTED)),
        Span::styled("ENTER ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)), Span::styled("tune  ", Style::default().fg(MUTED)),
        Span::styled("SPACE ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)), Span::styled("pause  ", Style::default().fg(MUTED)),
        Span::styled("S ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)), Span::styled("star  ", Style::default().fg(MUTED)),
        Span::styled("/ ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)), Span::styled("search  ", Style::default().fg(MUTED)),
        Span::styled("F ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)), Span::styled("favorites  ", Style::default().fg(MUTED)),
        Span::styled("Q ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)), Span::styled("quit", Style::default().fg(MUTED)),
    ])).alignment(Alignment::Center).block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(Color::Rgb(30, 55, 60))));
    f.render_widget(keys, chunks[3]);

    let footer = Paragraph::new(Span::styled("HAK.FM  ·  SomaFM streams", Style::default().fg(Color::Rgb(50, 90, 80))))
        .alignment(Alignment::Center);
    f.render_widget(footer, chunks[4]);

    if app.show_help {
        let popup = centered_rect(60, 65, area);
        f.render_widget(Clear, popup);
        f.render_widget(Block::default().title(" HELP ").borders(Borders::ALL).border_style(Style::default().fg(GREEN)).style(Style::default().bg(PANEL)), popup);
        let help = Paragraph::new(vec![
            Line::from(Span::styled("Keyboard shortcuts", Style::default().fg(GREEN).add_modifier(Modifier::BOLD))),
            Line::from(""),
            Line::from("↑ / k       Move selection up"),
            Line::from("↓ / j       Move selection down"),
            Line::from("Enter       Play selected station"),
            Line::from("Space       Stop / resume selected station"),
            Line::from("← / h       Previous station and play"),
            Line::from("→ / l       Next station and play"),
            Line::from("/           Search by name, genre, description"),
            Line::from("s           Toggle favorite"),
            Line::from("f           Show / hide favorites"),
            Line::from("x           Stop playback"),
            Line::from("r           Reset filters"),
            Line::from("?           Close this manual"),
            Line::from("q           Quit and stop playback"),
            Line::from(""),
            Line::from(Span::styled("Requires mpv installed and an internet connection.", Style::default().fg(MUTED))),
        ]).block(Block::default().padding(ratatui::widgets::Padding::new(2, 2, 1, 1))).wrap(Wrap { trim: false });
        f.render_widget(help, popup);
    }
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
        let wave = (t.sin() * 0.52 + (t * 0.47 + 1.2).sin() * 0.31 + (t * 0.19).sin() * 0.17).abs();
        let height = ((wave * 7.0).round() as usize).min(BARS.len() - 1);
        let color = if height >= 5 { GREEN } else { CYAN };
        spans.push(Span::styled(BARS[height], Style::default().fg(color)));
    }
    Line::from(spans)
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vertical = Layout::default().direction(Direction::Vertical)
        .constraints([Constraint::Percentage((100 - percent_y) / 2), Constraint::Percentage(percent_y), Constraint::Percentage((100 - percent_y) / 2)])
        .split(r);
    Layout::default().direction(Direction::Horizontal)
        .constraints([Constraint::Percentage((100 - percent_x) / 2), Constraint::Percentage(percent_x), Constraint::Percentage((100 - percent_x) / 2)])
        .split(vertical[1])[1]
}

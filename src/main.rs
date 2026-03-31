mod data;

use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};
use data::{
    filter_messages, find_all_chains, load_chat_history, parse_date, ContentStats, Message,
};
use iced::widget::{
    button, column, container, pick_list, row, scrollable, text,
    text_input, Column,
};
use iced::widget::rule;
use iced::widget::space;
use iced::widget::text::Wrapping;
use iced::{clipboard, Color, Element, Length, Subscription, Task, Theme};
use iced::{event, keyboard, Event};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

fn main() -> iced::Result {
    iced::application(App::boot, App::update, App::view)
        .theme(App::theme)
        .subscription(App::subscription)
        .window_size((1100.0, 700.0))
        .run()
}

// ---------------------------------------------------------------------------
// View modes — radio-button toggle, only one active
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    All,
    Month,
    Week,
    ThreeDays,
    Day,
    SixHours,
    ThreeHours,
    OneHour,
    Chains,
}

impl ViewMode {
    fn label(self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::Month => "MNTH",
            Self::Week => "WEEK",
            Self::ThreeDays => "3DAYS",
            Self::Day => "DAY",
            Self::SixHours => "6HRS",
            Self::ThreeHours => "3HRS",
            Self::OneHour => "1HRS",
            Self::Chains => "CHAINS",
        }
    }
    const ORDERED: &'static [Self] = &[
        Self::All,
        Self::Month,
        Self::Week,
        Self::ThreeDays,
        Self::Day,
        Self::SixHours,
        Self::ThreeHours,
        Self::OneHour,
        Self::Chains,
    ];
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

struct App {
    file_path: Option<PathBuf>,
    chat_name: Option<String>,
    messages: HashMap<String, Message>,
    all_chains: Vec<Vec<Message>>,
    selected_chain: Option<usize>,
    sorted_messages: Vec<Message>,
    content_stats: ContentStats,
    all_links: Vec<String>,
    show_all_links: bool,
    raw_json_view: Option<String>, // currently displayed raw JSON
    view_mode: ViewMode,
    time_groups: Vec<TimeGroup>,
    expanded_groups: HashSet<String>,
    search_query: String,
    search_active: bool,
    context_window: usize,
    context_input: String,
    min_chain_length: usize,
    current_theme: Theme,
    theme_name: ThemeName,
}

#[derive(Debug, Clone)]
struct TimeGroup {
    key: String,
    label: String,
    chain_indices: Vec<usize>,
    message_indices: Vec<usize>,
    #[allow(dead_code)]
    message_count: usize,
}

// ---------------------------------------------------------------------------
// Theme
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemeName {
    Dark,
    Light,
    Dracula,
    Nord,
    SolarizedDark,
    GruvboxDark,
    CatppuccinMocha,
    TokyoNight,
    Oxocarbon,
}

impl std::fmt::Display for ThemeName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::Dracula => "Dracula",
            Self::Nord => "Nord",
            Self::SolarizedDark => "Solarized Dark",
            Self::GruvboxDark => "Gruvbox Dark",
            Self::CatppuccinMocha => "Catppuccin Mocha",
            Self::TokyoNight => "Tokyo Night",
            Self::Oxocarbon => "Oxocarbon",
        })
    }
}

impl ThemeName {
    const ALL: &'static [Self] = &[
        Self::Dark,
        Self::Light,
        Self::Dracula,
        Self::Nord,
        Self::SolarizedDark,
        Self::GruvboxDark,
        Self::CatppuccinMocha,
        Self::TokyoNight,
        Self::Oxocarbon,
    ];
    fn to_theme(self) -> Theme {
        match self {
            Self::Dark => Theme::Dark,
            Self::Light => Theme::Light,
            Self::Dracula => Theme::Dracula,
            Self::Nord => Theme::Nord,
            Self::SolarizedDark => Theme::SolarizedDark,
            Self::GruvboxDark => Theme::GruvboxDark,
            Self::CatppuccinMocha => Theme::CatppuccinMocha,
            Self::TokyoNight => Theme::TokyoNight,
            Self::Oxocarbon => Theme::Oxocarbon,
        }
    }
}

// ---------------------------------------------------------------------------
// Msg
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Msg {
    OpenFile,
    FileOpened(Option<PathBuf>),
    SelectChain(usize),
    SelectGroup(String),
    ToggleGroup(String),
    SetViewMode(ViewMode),
    SearchChanged(String),
    ToggleSearch,
    ThemeSelected(ThemeName),
    CopyText(String),
    OpenUrl(String),
    ContextChanged(String),
    ToggleAllLinks,
    ShowRawJson(i64),
    CloseRawJson,
    NavUp,
    NavDown,
}

// ---------------------------------------------------------------------------
// Helpers for content tags shown in message view
// ---------------------------------------------------------------------------

fn content_tag_emoji(tag: &str) -> &'static str {
    match tag {
        "link" => "🔗",
        "image" => "🖼️",
        "video" => "📹",
        "file" => "📎",
        "sticker" => "🔰",
        "voice" => "🎤",
        "video_circle" => "⭕",
        "audio" => "🎵",
        "animation" => "🎞️",
        _ => "❓",
    }
}

/// Telegram-style avatar colors (from_rgb8 for exact values).
const TG_COLORS: [Color; 12] = [
    Color::from_rgb8(0xFF, 0x6B, 0x6B), // Red
    Color::from_rgb8(0xFF, 0x9F, 0x4A), // Orange
    Color::from_rgb8(0xFF, 0xD9, 0x3D), // Yellow
    Color::from_rgb8(0x6B, 0xCB, 0x77), // Green
    Color::from_rgb8(0x4D, 0x96, 0xFF), // Light Blue
    Color::from_rgb8(0x6C, 0x63, 0xFF), // Blue
    Color::from_rgb8(0xC0, 0x84, 0xFC), // Purple
    Color::from_rgb8(0xFF, 0x6B, 0x9D), // Pink
    Color::from_rgb8(0x4E, 0xCD, 0xC4), // Turquoise
    Color::from_rgb8(0x96, 0xCE, 0xB4), // Lime
    Color::from_rgb8(0xF4, 0xA2, 0x61), // Peach
    Color::from_rgb8(0xE9, 0xC4, 0x6A), // Magenta
];

/// Deterministic color based on name hash (like Telegram).
fn avatar_color(name: &str) -> Color {
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    let hash = hasher.finish();
    TG_COLORS[(hash % 12) as usize]
}

/// First letter uppercase for avatar.
fn avatar_letter(name: &str) -> String {
    name.chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".into())
}

/// Build a 26px colored circle avatar with a letter inside.
fn avatar_circle<'a>(name: &str) -> Element<'a, Msg> {
    let color = avatar_color(name);
    let letter = avatar_letter(name);
    let size: f32 = 26.0;
    container(
        text(letter)
            .size(size * 0.48)
            .color(Color::WHITE),
    )
    .width(size)
    .height(size)
    .align_x(iced::alignment::Horizontal::Center)
    .align_y(iced::alignment::Vertical::Center)
    .style(move |_theme: &Theme| container::Style {
        background: Some(color.into()),
        border: iced::Border {
            radius: (size / 2.0).into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    })
    .into()
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

impl App {
    fn boot() -> (Self, Task<Msg>) {
        let mut app = Self {
            file_path: None,
            chat_name: None,
            messages: HashMap::new(),
            all_chains: Vec::new(),
            selected_chain: None,
            sorted_messages: Vec::new(),
            content_stats: ContentStats::default(),
            all_links: Vec::new(),
            show_all_links: false,
            raw_json_view: None,
            view_mode: ViewMode::Chains,
            time_groups: Vec::new(),
            expanded_groups: HashSet::new(),
            search_query: String::new(),
            search_active: false,
            context_window: 0,
            context_input: "0".into(),
            min_chain_length: 2,
            current_theme: Theme::Dark,
            theme_name: ThemeName::Dark,
        };
        if let Some(path) = std::env::args().nth(1) {
            app.load_file(&PathBuf::from(path));
        }
        (app, Task::none())
    }

    fn load_file(&mut self, path: &PathBuf) {
        let result = load_chat_history(path);
        let (name, dict) = match result {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Error loading file: {}", e);
                self.chat_name = Some(format!("Error: {}", e));
                return;
            }
        };
        self.chat_name = name;
        self.file_path = Some(path.clone());

        let start = parse_date("1970-01-01").unwrap();
        let end = chrono::Local::now().naive_local();
        let filtered = filter_messages(&dict, start, end, None);
        let chains = find_all_chains(&filtered, &dict, self.min_chain_length);

        // Build sorted index list (oldest first) instead of cloning all messages
        let mut sorted_ids: Vec<i64> = dict.values().map(|m| m.id).collect();
        sorted_ids.sort();
        let sorted: Vec<Message> = sorted_ids
            .iter()
            .filter_map(|id| dict.get(&id.to_string()).cloned())
            .collect();

        self.content_stats = ContentStats::from_messages(&dict);

        // Collect all unique links, sorted alphabetically
        let mut all_links: Vec<String> = Vec::new();
        for msg in dict.values() {
            for e in &msg.text_entities {
                if (e.entity_type == "link" || e.entity_type == "text_link") && !e.text.is_empty() {
                    all_links.push(e.text.clone());
                }
            }
        }
        all_links.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
        all_links.dedup();
        self.all_links = all_links;
        self.show_all_links = false;

        self.messages = dict;
        self.all_chains = chains;
        self.sorted_messages = sorted;
        self.selected_chain = if self.all_chains.is_empty() {
            None
        } else {
            Some(0)
        };
        self.search_active = false;
        self.search_query.clear();
        self.rebuild_time_groups();
    }

    #[allow(dead_code)]
    fn rebuild_chains(&mut self) {
        let start = parse_date("1970-01-01").unwrap();
        let end = chrono::Local::now().naive_local();
        let filtered = filter_messages(&self.messages, start, end, None);
        self.all_chains = find_all_chains(&filtered, &self.messages, self.min_chain_length);
        self.selected_chain = if self.all_chains.is_empty() {
            None
        } else {
            Some(0)
        };
        self.rebuild_time_groups();
    }

    // -- time groups --

    fn rebuild_time_groups(&mut self) {
        self.time_groups.clear();
        match self.view_mode {
            ViewMode::All => {}
            ViewMode::Chains => self.build_chain_groups(),
            _ => self.build_message_time_groups(),
        }
    }

    fn build_chain_groups(&mut self) {
        let visible = self.visible_chain_indices();
        let mut by_day: BTreeMap<NaiveDate, Vec<(usize, NaiveDateTime, usize)>> = BTreeMap::new();
        for idx in &visible {
            let chain = &self.all_chains[*idx];
            if let Some(dt) = parse_date(&chain[0].date) {
                by_day
                    .entry(dt.date())
                    .or_default()
                    .push((*idx, dt, chain.len()));
            }
        }
        for (day, mut entries) in by_day.into_iter() {
            entries.sort_by_key(|(_, dt, _)| *dt);
            let total_msgs: usize = entries.iter().map(|(_, _, c)| c).sum();
            let chain_indices: Vec<usize> = entries.iter().map(|(i, _, _)| *i).collect();
            self.time_groups.push(TimeGroup {
                key: day.format("%Y-%m-%d").to_string(),
                label: format!(
                    "{} — {} msgs, {} chains",
                    day.format("%Y-%m-%d"),
                    total_msgs,
                    chain_indices.len()
                ),
                chain_indices,
                message_indices: Vec::new(),
                message_count: total_msgs,
            });
        }
    }

    fn build_message_time_groups(&mut self) {
        let visible = self.visible_message_indices();
        let mut buckets: BTreeMap<String, (String, Vec<usize>, usize)> = BTreeMap::new();
        for idx in visible {
            let msg = &self.sorted_messages[idx];
            let dt = match parse_date(&msg.date) {
                Some(d) => d,
                None => continue,
            };
            let (key, label) = self.bucket_key_label(dt);
            let entry = buckets
                .entry(key)
                .or_insert_with(|| (label, Vec::new(), 0));
            entry.1.push(idx);
            entry.2 += 1;
        }
        let mut keys: Vec<String> = buckets.keys().cloned().collect();
        keys.sort();
        for k in keys {
            let (label, message_indices, message_count) = buckets.remove(&k).unwrap();
            self.time_groups.push(TimeGroup {
                key: k,
                label: format!("{} — {} msgs", label, message_count),
                chain_indices: Vec::new(),
                message_indices,
                message_count,
            });
        }
    }

    fn bucket_key_label(&self, dt: NaiveDateTime) -> (String, String) {
        match self.view_mode {
            ViewMode::Month => {
                let key = dt.format("%Y-%m").to_string();
                let label = dt.format("%Y %B").to_string();
                (key, label)
            }
            ViewMode::Week => {
                let iso = dt.date().iso_week();
                let key = format!("{}-W{:02}", iso.year(), iso.week());
                let label = format!("{} Week {}", iso.year(), iso.week());
                (key, label)
            }
            ViewMode::ThreeDays => {
                let day = dt.date();
                let bucket = day.ordinal0() / 3;
                let start_ord = bucket * 3;
                let start = NaiveDate::from_yo_opt(day.year(), start_ord + 1).unwrap_or(day);
                let end = NaiveDate::from_yo_opt(day.year(), start_ord + 3).unwrap_or(start);
                let key = format!("{}-{:03}", day.year(), bucket);
                let label = format!("{} – {}", start.format("%Y-%m-%d"), end.format("%m-%d"));
                (key, label)
            }
            ViewMode::Day => {
                let key = dt.format("%Y-%m-%d").to_string();
                let label = dt.format("%Y-%m-%d %a").to_string();
                (key, label)
            }
            ViewMode::SixHours => {
                let slot = dt.hour() / 6;
                let (h0, h1) = (slot * 6, slot * 6 + 6);
                let key = format!("{}-{:02}", dt.format("%Y-%m-%d"), slot);
                let label = format!("{} {:02}:00–{:02}:00", dt.format("%Y-%m-%d"), h0, h1);
                (key, label)
            }
            ViewMode::ThreeHours => {
                let slot = dt.hour() / 3;
                let (h0, h1) = (slot * 3, slot * 3 + 3);
                let key = format!("{}-{:02}", dt.format("%Y-%m-%d"), slot);
                let label = format!("{} {:02}:00–{:02}:00", dt.format("%Y-%m-%d"), h0, h1);
                (key, label)
            }
            ViewMode::OneHour => {
                let h = dt.hour();
                let key = format!("{}-{:02}", dt.format("%Y-%m-%d"), h);
                let label = format!("{} {:02}:00–{:02}:00", dt.format("%Y-%m-%d"), h, h + 1);
                (key, label)
            }
            _ => {
                let key = dt.format("%Y-%m-%d").to_string();
                (key.clone(), key)
            }
        }
    }

    /// Returns indices of messages that directly match the search query.
    fn matched_message_indices(&self) -> HashSet<usize> {
        if !self.search_active || self.search_query.is_empty() {
            return HashSet::new();
        }
        let q = self.search_query.to_lowercase();
        self.sorted_messages
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                m.text_entities
                    .iter()
                    .any(|e| e.text.to_lowercase().contains(&q))
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Returns visible indices: matched messages + context window neighbors.
    fn visible_message_indices(&self) -> Vec<usize> {
        if !self.search_active || self.search_query.is_empty() {
            return (0..self.sorted_messages.len()).collect();
        }
        let matched = self.matched_message_indices();
        if matched.is_empty() {
            return Vec::new();
        }
        if self.context_window == 0 {
            let mut v: Vec<usize> = matched.into_iter().collect();
            v.sort();
            return v;
        }
        // Context window: N total context messages split before/after
        // 1 -> 0 before, 1 after
        // 2 -> 1 before, 1 after
        // 3 -> 1 before, 2 after
        // 4 -> 2 before, 2 after
        let before = self.context_window / 2;
        let after = self.context_window - before;
        let len = self.sorted_messages.len();
        let mut expanded: HashSet<usize> = HashSet::new();
        for &idx in &matched {
            let start = idx.saturating_sub(before);
            let end = (idx + after + 1).min(len);
            for i in start..end {
                expanded.insert(i);
            }
        }
        let mut v: Vec<usize> = expanded.into_iter().collect();
        v.sort();
        v
    }

    fn visible_chain_indices(&self) -> Vec<usize> {
        if !self.search_active || self.search_query.is_empty() {
            return (0..self.all_chains.len()).collect();
        }
        let q = self.search_query.to_lowercase();
        self.all_chains
            .iter()
            .enumerate()
            .filter(|(_, chain)| {
                chain.iter().any(|m| {
                    m.text_entities
                        .iter()
                        .any(|e| e.text.to_lowercase().contains(&q))
                })
            })
            .map(|(i, _)| i)
            .collect()
    }

    fn theme(&self) -> Theme {
        self.current_theme.clone()
    }

    fn subscription(&self) -> Subscription<Msg> {
        event::listen_with(|ev, status, _| {
            if let event::Status::Captured = status {
                return None;
            }
            match ev {
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::ArrowUp),
                    ..
                }) => Some(Msg::NavUp),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::ArrowDown),
                    ..
                }) => Some(Msg::NavDown),
                _ => None,
            }
        })
    }

    // -- update --

    fn update(&mut self, message: Msg) -> Task<Msg> {
        match message {
            Msg::OpenFile => {
                return Task::perform(
                    async {
                        let handle = rfd::AsyncFileDialog::new()
                            .add_filter("JSON", &["json"])
                            .pick_file()
                            .await;
                        handle.map(|h| h.path().to_path_buf())
                    },
                    Msg::FileOpened,
                );
            }
            Msg::FileOpened(Some(path)) => self.load_file(&path),
            Msg::FileOpened(None) => {}
            Msg::SelectChain(i) => {
                self.selected_chain = Some(i);
                self.show_all_links = false;
                self.raw_json_view = None;
            }
            Msg::SelectGroup(key) => {
                self.expanded_groups.clear();
                self.expanded_groups.insert(key);
                self.selected_chain = None;
                self.show_all_links = false;
                self.raw_json_view = None;
            }
            Msg::ToggleGroup(key) => {
                if !self.expanded_groups.remove(&key) {
                    self.expanded_groups.insert(key);
                }
                self.show_all_links = false;
            }
            Msg::SetViewMode(mode) => {
                self.view_mode = mode;
                self.expanded_groups.clear();
                self.selected_chain = None;
                self.show_all_links = false;
                self.rebuild_time_groups();
            }
            Msg::SearchChanged(q) => {
                self.search_query = q;
            }
            Msg::ToggleSearch => {
                self.search_active = !self.search_active;
                self.rebuild_time_groups();
            }
            Msg::ThemeSelected(name) => {
                self.theme_name = name;
                self.current_theme = name.to_theme();
            }
            Msg::CopyText(s) => {
                return clipboard::write(s);
            }
            Msg::ContextChanged(s) => {
                self.context_input = s.clone();
                if let Ok(n) = s.parse::<usize>() {
                    self.context_window = n;
                    if self.search_active {
                        self.rebuild_time_groups();
                    }
                }
            }
            Msg::ToggleAllLinks => {
                self.show_all_links = !self.show_all_links;
                self.raw_json_view = None;
            }
            Msg::ShowRawJson(msg_id) => {
                // Look up message by id and serialize to pretty JSON
                let key = msg_id.to_string();
                if let Some(msg) = self.messages.get(&key) {
                    match serde_json::to_string_pretty(msg) {
                        Ok(json) => self.raw_json_view = Some(json),
                        Err(e) => self.raw_json_view = Some(format!("Error: {}", e)),
                    }
                } else {
                    self.raw_json_view = Some(format!("Message {} not found", msg_id));
                }
            }
            Msg::CloseRawJson => {
                self.raw_json_view = None;
            }
            Msg::OpenUrl(url) => {
                #[cfg(target_os = "macos")]
                { let _ = std::process::Command::new("open").arg(&url).spawn(); }
                #[cfg(target_os = "linux")]
                { let _ = std::process::Command::new("xdg-open").arg(&url).spawn(); }
                #[cfg(target_os = "windows")]
                { let _ = std::process::Command::new("cmd").args(["/C", "start", &url]).spawn(); }
            }
            Msg::NavUp => {
                self.show_all_links = false;
                match self.view_mode {
                    ViewMode::Chains => {
                        if let Some(idx) = self.selected_chain {
                            if idx > 0 {
                                self.selected_chain = Some(idx - 1);
                            }
                        }
                    }
                    _ => {
                        // Navigate groups: find current expanded, move to previous
                        if let Some(cur_key) = self.expanded_groups.iter().next().cloned() {
                            if let Some(pos) = self.time_groups.iter().position(|g| g.key == cur_key) {
                                if pos > 0 {
                                    let new_key = self.time_groups[pos - 1].key.clone();
                                    self.expanded_groups.clear();
                                    self.expanded_groups.insert(new_key);
                                }
                            }
                        }
                    }
                }
            }
            Msg::NavDown => {
                self.show_all_links = false;
                match self.view_mode {
                    ViewMode::Chains => {
                        let max = self.all_chains.len().saturating_sub(1);
                        if let Some(idx) = self.selected_chain {
                            if idx < max {
                                self.selected_chain = Some(idx + 1);
                            }
                        } else if !self.all_chains.is_empty() {
                            self.selected_chain = Some(0);
                        }
                    }
                    _ => {
                        if let Some(cur_key) = self.expanded_groups.iter().next().cloned() {
                            if let Some(pos) = self.time_groups.iter().position(|g| g.key == cur_key) {
                                if pos + 1 < self.time_groups.len() {
                                    let new_key = self.time_groups[pos + 1].key.clone();
                                    self.expanded_groups.clear();
                                    self.expanded_groups.insert(new_key);
                                }
                            }
                        } else if !self.time_groups.is_empty() {
                            let key = self.time_groups[0].key.clone();
                            self.expanded_groups.clear();
                            self.expanded_groups.insert(key);
                        }
                    }
                }
            }
        }
        Task::none()
    }

    // -----------------------------------------------------------------------
    // View
    // -----------------------------------------------------------------------

    fn view(&self) -> Element<'_, Msg> {
        let toolbar = self.view_toolbar();
        let content = if self.messages.is_empty() {
            self.view_empty()
        } else {
            self.view_main()
        };
        column![toolbar, content].spacing(0).into()
    }

    fn view_toolbar(&self) -> Element<'_, Msg> {
        let open_btn = button("Open JSON…").on_press(Msg::OpenFile).padding(6);

        let title_text: Element<'_, Msg> = match &self.chat_name {
            Some(n) => text(format!("  {}  ", n)).size(16).into(),
            None => text("  tgxplorer  ").size(16).into(),
        };

        // Emoji stats chips (only when data loaded)
        let stats_row: Element<'_, Msg> = if !self.messages.is_empty() {
            let s = &self.content_stats;
            let mut chips: Vec<Element<'_, Msg>> = Vec::new();
            // Links chip is a button
            if s.links > 0 {
                chips.push(
                    button(text(format!("🔗{}", s.links)).size(12))
                        .on_press(Msg::ToggleAllLinks)
                        .padding([1, 4])
                        .style(if self.show_all_links {
                            button::primary
                        } else {
                            button::text
                        })
                        .into(),
                );
            }
            let other_pairs: &[(&str, usize)] = &[
                ("🖼️", s.images),
                ("📹", s.videos),
                ("📎", s.files),
                ("🔰", s.stickers),
                ("🎤", s.voice),
                ("⭕", s.video_circles),
                ("🔁", s.reposts),
            ];
            for &(emoji, count) in other_pairs {
                if count > 0 {
                    chips.push(
                        text(format!("{}{}", emoji, count))
                            .size(12)
                            .into(),
                    );
                }
            }
            row(chips).spacing(8).into()
        } else {
            text("").into()
        };

        // Search
        let search_input = text_input("Search…", &self.search_query)
            .on_input(Msg::SearchChanged)
            .on_submit(Msg::ToggleSearch)
            .width(180)
            .padding(5);

        let search_btn = button(
            text(if self.search_active { "Clear" } else { "Search" }).size(13),
        )
        .on_press(Msg::ToggleSearch)
        .padding(6);

        let theme_pick =
            pick_list(ThemeName::ALL, Some(self.theme_name), Msg::ThemeSelected).padding(4);

        let ctx_label = text("Context:").size(12);
        let ctx_input = text_input("0", &self.context_input)
            .on_input(Msg::ContextChanged)
            .width(36)
            .padding(5);

        let tb = row![
            open_btn,
            title_text,
            stats_row,
            space::horizontal(),
            row![search_input, search_btn].spacing(4),
            row![ctx_label, ctx_input].spacing(3).align_y(iced::Alignment::Center),
            theme_pick,
        ]
        .spacing(10)
        .padding(8)
        .align_y(iced::Alignment::Center);

        container(tb).width(Length::Fill).into()
    }

    fn view_empty(&self) -> Element<'_, Msg> {
        let msg = column![
            text("tgxplorer").size(32),
            text("Telegram Exported Chat Explorer").size(16),
            text("").size(8),
            text("Open a Telegram export JSON file to begin.").size(14),
            text("").size(4),
            button("Open JSON…").on_press(Msg::OpenFile).padding(10),
        ]
        .spacing(6)
        .align_x(iced::Alignment::Center);

        container(msg)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }

    fn view_main(&self) -> Element<'_, Msg> {
        // Mode toggle bar
        let mode_buttons: Vec<Element<'_, Msg>> = ViewMode::ORDERED
            .iter()
            .map(|&mode| {
                let active = self.view_mode == mode;
                button(text(mode.label()).size(12))
                    .on_press(Msg::SetViewMode(mode))
                    .padding([3, 8])
                    .style(if active {
                        button::primary
                    } else {
                        button::secondary
                    })
                    .into()
            })
            .collect();
        let mode_bar = row(mode_buttons)
            .spacing(3)
            .padding([4, 6])
            .align_y(iced::Alignment::Center);

        let sidebar = self.view_sidebar();
        let detail = self.view_detail();
        let detail_scroll = scrollable(container(detail).padding(10)).height(Length::Fill);

        let body = row![
            container(sidebar).padding(6).width(340),
            container(detail_scroll).width(Length::Fill),
        ]
        .spacing(4)
        .height(Length::Fill);

        column![mode_bar, body].spacing(2).into()
    }

    fn view_sidebar(&self) -> Element<'_, Msg> {
        if self.view_mode == ViewMode::All {
            let info = column![
                text("Showing all messages").size(13),
                text(format!("{} total", self.sorted_messages.len())).size(12),
            ]
            .spacing(4)
            .padding(8);
            return scrollable(info).height(Length::Fill).into();
        }

        let mut items: Vec<Element<'_, Msg>> = Vec::new();
        for group in &self.time_groups {
            let expanded = self.expanded_groups.contains(&group.key);
            let arrow = if expanded { "▼" } else { "▶" };
            let group_btn = button(text(format!("{} {}", arrow, group.label)).size(12))
                .on_press(if self.view_mode == ViewMode::Chains {
                    Msg::ToggleGroup(group.key.clone())
                } else {
                    Msg::SelectGroup(group.key.clone())
                })
                .width(Length::Fill)
                .padding([4, 6])
                .style(if expanded && self.view_mode != ViewMode::Chains {
                    button::primary
                } else {
                    button::text
                });
            items.push(group_btn.into());

            if self.view_mode == ViewMode::Chains && expanded {
                for &ci in &group.chain_indices {
                    let chain = &self.all_chains[ci];
                    let first = &chain[0];
                    let ts = parse_date(&first.date)
                        .map(|d| d.format("%H:%M").to_string())
                        .unwrap_or_default();
                    let sender = first.from.as_deref().unwrap_or("");
                    let preview: String = first
                        .text_entities
                        .iter()
                        .filter(|e| e.entity_type == "plain")
                        .flat_map(|e| e.text.chars())
                        .take(28)
                        .collect();
                    let label = format!("  {} {} {}…", ts, sender, preview);
                    let selected = self.selected_chain == Some(ci);
                    items.push(
                        button(text(label).size(11))
                            .on_press(Msg::SelectChain(ci))
                            .width(Length::Fill)
                            .padding([2, 16])
                            .style(if selected {
                                button::primary
                            } else {
                                button::secondary
                            })
                            .into(),
                    );
                }
            }
        }
        scrollable(Column::with_children(items).spacing(1).width(Length::Fill))
            .height(Length::Fill)
            .into()
    }

    fn view_detail(&self) -> Element<'_, Msg> {
        // Show raw JSON panel if active
        if let Some(ref json) = self.raw_json_view {
            let mut json_items: Vec<Element<'_, Msg>> = vec![
                row![
                    text("Raw JSON").size(16),
                    space::horizontal(),
                    button(text("⎘ Copy").size(12))
                        .on_press(Msg::CopyText(json.clone()))
                        .padding([3, 8])
                        .style(button::secondary),
                    button(text("✕ Close").size(12))
                        .on_press(Msg::CloseRawJson)
                        .padding([3, 8])
                        .style(button::secondary),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into(),
                rule::horizontal(1).into(),
            ];
            // Render JSON lines with syntax-colored formatting
            for line in json.lines() {
                json_items.push(
                    text(line.to_string())
                        .size(13)
                        .font(iced::Font::MONOSPACE)
                        .wrapping(Wrapping::None)
                        .into(),
                );
            }
            return Column::with_children(json_items)
                .spacing(1)
                .width(Length::Fill)
                .into();
        }

        // Show all links panel if toggled
        if self.show_all_links {
            let mut link_items: Vec<Element<'_, Msg>> = vec![
                text(format!("All links ({})", self.all_links.len()))
                    .size(16)
                    .into(),
                rule::horizontal(1).into(),
            ];
            for url in &self.all_links {
                let full_url = if url.starts_with("http://") || url.starts_with("https://") {
                    url.clone()
                } else {
                    format!("https://{}", url)
                };
                let link_row = row![
                    text(format!("🔗 {}", url))
                        .size(13)
                        .wrapping(Wrapping::Word),
                    button(text("⎘").size(11))
                        .on_press(Msg::CopyText(url.clone()))
                        .padding([1, 4])
                        .style(button::text),
                    button(text("↗").size(11))
                        .on_press(Msg::OpenUrl(full_url))
                        .padding([1, 4])
                        .style(button::text),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center);
                link_items.push(link_row.into());
            }
            // Copy all links button
            let all_text = self.all_links.join("\n");
            link_items.push(rule::horizontal(1).into());
            link_items.push(
                button(text("Copy all links").size(13))
                    .on_press(Msg::CopyText(all_text))
                    .padding(6)
                    .into(),
            );
            return Column::with_children(link_items)
                .spacing(3)
                .width(Length::Fill)
                .into();
        }

        let matched = self.matched_message_indices();
        match self.view_mode {
            ViewMode::All => {
                let indices = self.visible_message_indices();
                if indices.is_empty() {
                    return column![text("No messages")].into();
                }
                let items: Vec<Element<'_, Msg>> = indices
                    .iter()
                    .take(2000)
                    .map(|&i| self.view_message(&self.sorted_messages[i], matched.contains(&i)))
                    .collect();
                Column::with_children(items)
                    .spacing(4)
                    .width(Length::Fill)
                    .into()
            }
            ViewMode::Chains => {
                if let Some(idx) = self.selected_chain {
                    if idx < self.all_chains.len() {
                        return self.view_chain(&self.all_chains[idx]);
                    }
                }
                column![text("Select a chain from the list")].into()
            }
            _ => {
                if let Some(key) = self.expanded_groups.iter().next() {
                    if let Some(group) = self.time_groups.iter().find(|g| &g.key == key) {
                        let items: Vec<Element<'_, Msg>> = group
                            .message_indices
                            .iter()
                            .take(2000)
                            .map(|&i| self.view_message(&self.sorted_messages[i], matched.contains(&i)))
                            .collect();
                        return Column::with_children(items)
                            .spacing(4)
                            .width(Length::Fill)
                            .into();
                    }
                }
                column![text("Select a time group from the sidebar")].into()
            }
        }
    }

    fn view_chain<'a>(&'a self, chain: &'a [Message]) -> Element<'a, Msg> {
        let msgs: Vec<Element<'_, Msg>> = chain.iter().map(|m| self.view_message(m, false)).collect();
        Column::with_children(msgs)
            .spacing(4)
            .width(Length::Fill)
            .into()
    }

    fn view_message<'a>(&'a self, msg: &'a Message, highlight: bool) -> Element<'a, Msg> {
        let sender = msg.from.as_deref().unwrap_or("Unknown");
        let fwd = msg
            .forwarded_from
            .as_ref()
            .map(|f| format!(" (fwd: {})", f))
            .unwrap_or_default();
        // Build tg:// link for forwarded user id
        let fwd_tg_link: Option<String> = msg.forwarded_from_id.as_ref().and_then(|id| {
            let numeric: String = id.chars().filter(|c| c.is_ascii_digit()).collect();
            if numeric.is_empty() {
                None
            } else {
                Some(format!("tg://user?id={}", numeric))
            }
        });
        let dt = parse_date(&msg.date)
            .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| msg.date.clone());

        // Collect text
        let plain: String = msg
            .text_entities
            .iter()
            .filter(|e| e.entity_type == "plain" || e.entity_type == "pre")
            .map(|e| e.text.as_str())
            .collect::<Vec<_>>()
            .join("");

        let links: Vec<&str> = msg
            .text_entities
            .iter()
            .filter(|e| e.entity_type == "link" || e.entity_type == "text_link")
            .map(|e| e.text.as_str())
            .collect();

        let mentions: Vec<&str> = msg
            .text_entities
            .iter()
            .filter(|e| e.entity_type == "mention")
            .map(|e| e.text.as_str())
            .collect();

        // -- Build parts --

        let mut parts: Vec<Element<'_, Msg>> = vec![rule::horizontal(1).into()];

        // Avatar circle + sender name + timestamp
        let avatar = avatar_circle(sender);
        let header = row![
            avatar,
            text(format!("{}{}", sender, fwd)).size(14),
            space::horizontal(),
            text(dt).size(12),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center);
        parts.push(header.into());

        // Forwarded user tg:// link
        if let Some(ref tg_link) = fwd_tg_link {
            let fwd_row = row![
                text(format!("↗ {}", tg_link)).size(11),
                button(text("⎘").size(10))
                    .on_press(Msg::CopyText(tg_link.clone()))
                    .padding([1, 3])
                    .style(button::text),
                button(text("↗").size(10))
                    .on_press(Msg::OpenUrl(tg_link.clone()))
                    .padding([1, 3])
                    .style(button::text),
            ]
            .spacing(3)
            .align_y(iced::Alignment::Center);
            parts.push(fwd_row.into());
        }

        // Content type tag -- sticker shows emoji in brackets
        if let Some(tag) = msg.content_tag() {
            let emoji = content_tag_emoji(tag);
            let detail = match tag {
                "sticker" => {
                    let se = msg.sticker_emoji.as_deref().unwrap_or("");
                    format!("{} sticker ({})", emoji, se)
                }
                "file" => msg
                    .file_name
                    .as_ref()
                    .map(|f| format!("{} {}", emoji, f))
                    .unwrap_or_else(|| format!("{} file", emoji)),
                _ => format!("{} {}", emoji, tag),
            };
            parts.push(text(detail).size(12).into());
        }


        if !mentions.is_empty() {
            let mut mention_items: Vec<Element<'_, Msg>> = vec![
                text("Mentions:").size(12).into(),
            ];
            for m in &mentions {
                mention_items.push(
                    row![
                        text(*m).size(12),
                        button(text("⎘").size(10))
                            .on_press(Msg::CopyText(m.to_string()))
                            .padding([1, 3])
                            .style(button::text),
                    ]
                    .spacing(2)
                    .align_y(iced::Alignment::Center)
                    .into(),
                );
            }
            if mentions.len() > 1 {
                let all_mentions = mentions.iter().copied().collect::<Vec<_>>().join(" ");
                mention_items.push(
                    button(text("⎘ all").size(10))
                        .on_press(Msg::CopyText(all_mentions))
                        .padding([1, 4])
                        .style(button::text)
                        .into(),
                );
            }
            parts.push(row(mention_items).spacing(4).align_y(iced::Alignment::Center).into());
        }

        // Links — each with copy and open buttons
        for url in &links {
            let full_url = if url.starts_with("http://") || url.starts_with("https://") {
                url.to_string()
            } else {
                format!("https://{}", url)
            };
            let link_row = row![
                text(format!("🔗 {}", url))
                    .size(12)
                    .wrapping(Wrapping::Word),
                button(text("⎘").size(11))
                    .on_press(Msg::CopyText(url.to_string()))
                    .padding([1, 4])
                    .style(button::text),
                button(text("↗").size(11))
                    .on_press(Msg::OpenUrl(full_url))
                    .padding([1, 4])
                    .style(button::text),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center);
            parts.push(link_row.into());
        }

        // Message body — wrapping text + copy button + RAW button
        let raw_btn = button(text("{ }").size(11))
            .on_press(Msg::ShowRawJson(msg.id))
            .padding([2, 6])
            .style(button::text);

        if !plain.is_empty() {
            let body_text = text(plain.clone()).size(14).wrapping(Wrapping::Word);
            let copy_btn = button(text("⎘").size(12))
                .on_press(Msg::CopyText(plain))
                .padding([2, 6])
                .style(button::text);
            parts.push(
                row![container(body_text).width(Length::Fill), copy_btn, raw_btn]
                    .spacing(4)
                    .align_y(iced::Alignment::Start)
                    .into(),
            );
        } else {
            // No body text, just show RAW button
            parts.push(raw_btn.into());
        }

        let msg_col = Column::with_children(parts)
            .spacing(3)
            .padding(6)
            .width(Length::Fill);

        if highlight {
            container(msg_col)
                .width(Length::Fill)
                .style(|_theme: &Theme| container::Style {
                    background: Some(Color::from_rgba8(0xFF, 0xD9, 0x3D, 0.15).into()),
                    border: iced::Border {
                        radius: 4.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    ..Default::default()
                })
                .into()
        } else {
            msg_col.into()
        }
    }
}

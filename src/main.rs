mod data;

use data::{
    filter_messages, find_all_chains, load_chat_history, parse_date, Message,
};
use iced::widget::{
    button, column, container, horizontal_rule, horizontal_space, pick_list, row, scrollable,
    text, text_input, Column,
};
use iced::{Element, Length, Task, Theme};
use std::collections::HashMap;
use std::path::PathBuf;

fn main() -> iced::Result {
    iced::application("tgxplorer", App::update, App::view)
        .theme(App::theme)
        .window_size((1100.0, 700.0))
        .run_with(App::new)
}

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

struct App {
    // Data
    file_path: Option<PathBuf>,
    chat_name: Option<String>,
    messages: HashMap<String, Message>,
    all_chains: Vec<Vec<Message>>,
    selected_chain: Option<usize>,
    // Filters
    search_query: String,
    search_active: bool,
    min_chain_length: usize,
    min_chain_input: String,
    // Theme
    current_theme: Theme,
    theme_name: ThemeName,
}

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
// Messages
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Msg {
    OpenFile,
    FileOpened(Option<PathBuf>),
    SelectChain(usize),
    SearchChanged(String),
    ToggleSearch,
    ThemeSelected(ThemeName),
    MinChainChanged(String),
    ApplyMinChain,
}

impl App {
    fn new() -> (Self, Task<Msg>) {
        let mut app = Self {
            file_path: None,
            chat_name: None,
            messages: HashMap::new(),
            all_chains: Vec::new(),
            selected_chain: None,
            search_query: String::new(),
            search_active: false,
            min_chain_length: 2,
            min_chain_input: "2".into(),
            current_theme: Theme::Dark,
            theme_name: ThemeName::Dark,
        };

        // Check CLI arg
        let task = if let Some(path) = std::env::args().nth(1) {
            let p = PathBuf::from(path);
            app.load_file(&p);
            Task::none()
        } else {
            Task::none()
        };

        (app, task)
    }

    fn load_file(&mut self, path: &PathBuf) {
        let (name, dict) = load_chat_history(path);
        self.chat_name = name;
        self.file_path = Some(path.clone());

        let start = parse_date("1970-01-01").unwrap();
        let end = chrono::Local::now().naive_local();
        let filtered = filter_messages(&dict, start, end, None);
        let chains = find_all_chains(&filtered, &dict, self.min_chain_length);

        self.messages = dict;
        self.all_chains = chains;
        self.selected_chain = if self.all_chains.is_empty() {
            None
        } else {
            Some(0)
        };
        self.search_active = false;
        self.search_query.clear();
    }

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
    }

    fn visible_chains(&self) -> Vec<(usize, &Vec<Message>)> {
        if !self.search_active || self.search_query.is_empty() {
            return self.all_chains.iter().enumerate().collect();
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
            .collect()
    }

    fn theme(&self) -> Theme {
        self.current_theme.clone()
    }

    // -----------------------------------------------------------------------
    // Update
    // -----------------------------------------------------------------------

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
            Msg::FileOpened(Some(path)) => {
                self.load_file(&path);
            }
            Msg::FileOpened(None) => {}
            Msg::SelectChain(i) => {
                self.selected_chain = Some(i);
            }
            Msg::SearchChanged(q) => {
                self.search_query = q;
            }
            Msg::ToggleSearch => {
                self.search_active = !self.search_active;
            }
            Msg::ThemeSelected(name) => {
                self.theme_name = name;
                self.current_theme = name.to_theme();
            }
            Msg::MinChainChanged(s) => {
                self.min_chain_input = s;
            }
            Msg::ApplyMinChain => {
                if let Ok(n) = self.min_chain_input.parse::<usize>() {
                    self.min_chain_length = n.max(1);
                    self.rebuild_chains();
                }
            }
        }
        Task::none()
    }

    // -----------------------------------------------------------------------
    // View
    // -----------------------------------------------------------------------

    fn view(&self) -> Element<Msg> {
        let toolbar = self.view_toolbar();
        let content = if self.all_chains.is_empty() {
            self.view_empty()
        } else {
            self.view_main()
        };
        column![toolbar, content].spacing(0).into()
    }

    fn view_toolbar(&self) -> Element<Msg> {
        let open_btn = button("Open JSON…").on_press(Msg::OpenFile).padding(6);

        let title_text = match &self.chat_name {
            Some(n) => text(format!("  {}  ", n)).size(16),
            None => text("  tgxplorer  ").size(16),
        };

        let chain_count = text(format!(
            "Chains: {}",
            self.visible_chains().len()
        ))
        .size(14);

        let search_input = text_input("Search…", &self.search_query)
            .on_input(Msg::SearchChanged)
            .on_submit(Msg::ToggleSearch)
            .width(180)
            .padding(5);

        let search_btn = button(if self.search_active { "✕" } else { "⌕" })
            .on_press(Msg::ToggleSearch)
            .padding(6);

        let theme_pick = pick_list(
            ThemeName::ALL,
            Some(self.theme_name),
            Msg::ThemeSelected,
        )
        .padding(4);

        let min_input = text_input("min", &self.min_chain_input)
            .on_input(Msg::MinChainChanged)
            .on_submit(Msg::ApplyMinChain)
            .width(50)
            .padding(5);
        let min_label = text("Min chain:").size(13);

        let tb = row![
            open_btn,
            title_text,
            horizontal_space(),
            chain_count,
            row![min_label, min_input].spacing(4).align_y(iced::Alignment::Center),
            row![search_input, search_btn].spacing(4),
            theme_pick,
        ]
        .spacing(12)
        .padding(8)
        .align_y(iced::Alignment::Center);

        container(tb).width(Length::Fill).into()
    }

    fn view_empty(&self) -> Element<Msg> {
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

    fn view_main(&self) -> Element<Msg> {
        let visible = self.visible_chains();

        // Chain list (left panel)
        let chain_items: Vec<Element<Msg>> = visible
            .iter()
            .map(|(real_idx, chain)| {
                let first = &chain[0];
                let dt = parse_date(&first.date)
                    .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| first.date.clone());
                let label = format!("({}) {}", chain.len(), dt);
                let is_selected = self.selected_chain == Some(*real_idx);
                let btn = button(text(label).size(13))
                    .on_press(Msg::SelectChain(*real_idx))
                    .width(Length::Fill)
                    .padding(6)
                    .style(if is_selected {
                        button::primary
                    } else {
                        button::secondary
                    });
                btn.into()
            })
            .collect();

        let chain_list = scrollable(
            Column::with_children(chain_items).spacing(2).width(280),
        )
        .height(Length::Fill);

        // Message detail (right panel)
        let detail = if let Some(idx) = self.selected_chain {
            if idx < self.all_chains.len() {
                self.view_chain(&self.all_chains[idx])
            } else {
                column![text("No chain selected")].into()
            }
        } else {
            column![text("Select a chain from the list")].into()
        };

        let detail_scroll = scrollable(container(detail).padding(10)).height(Length::Fill);

        row![
            container(chain_list).padding(6),
            container(detail_scroll).width(Length::Fill),
        ]
        .spacing(4)
        .height(Length::Fill)
        .into()
    }

    fn view_chain<'a>(&'a self, chain: &'a [Message]) -> Element<'a, Msg> {
        let q = if self.search_active {
            Some(self.search_query.to_lowercase())
        } else {
            None
        };

        let msgs: Vec<Element<Msg>> = chain
            .iter()
            .map(|msg| self.view_message(msg, q.as_deref()))
            .collect();

        Column::with_children(msgs).spacing(4).width(Length::Fill).into()
    }

    fn view_message<'a>(&'a self, msg: &'a Message, _search: Option<&str>) -> Element<'a, Msg> {
        let sender = msg.from.as_deref().unwrap_or("Unknown");
        let fwd = msg
            .forwarded_from
            .as_ref()
            .map(|f| format!(" (fwd: {})", f))
            .unwrap_or_default();

        let dt = parse_date(&msg.date)
            .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| msg.date.clone());

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
            .filter(|e| e.entity_type == "link")
            .map(|e| e.text.as_str())
            .collect();

        let mentions: Vec<&str> = msg
            .text_entities
            .iter()
            .filter(|e| e.entity_type == "mention")
            .map(|e| e.text.as_str())
            .collect();

        let mut parts: Vec<Element<Msg>> = vec![
            horizontal_rule(1).into(),
            row![
                text(format!("{}{}", sender, fwd)).size(14),
                horizontal_space(),
                text(dt.clone()).size(12),
            ]
            .into(),
        ];

        if !mentions.is_empty() {
            parts.push(text(format!("Mentions: {}", mentions.join(", "))).size(12).into());
        }
        if !links.is_empty() {
            parts.push(text(format!("Links: {}", links.join(" "))).size(12).into());
        }
        if !plain.is_empty() {
            parts.push(text(plain.clone()).size(14).into());
        }

        Column::with_children(parts)
            .spacing(3)
            .padding(6)
            .width(Length::Fill)
            .into()
    }
}

use chrono::NaiveDateTime;
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// A single Telegram message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: i64,
    #[serde(default)]
    pub date: String,
    #[serde(default, rename = "type")]
    pub msg_type: String,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub from_id: Option<String>,
    #[serde(default)]
    pub forwarded_from: Option<String>,
    #[serde(default)]
    pub forwarded_from_id: Option<String>,
    #[serde(default)]
    pub reply_to_message_id: Option<i64>,
    #[serde(default)]
    pub text_entities: Vec<TextEntity>,
    #[serde(default)]
    pub edited_unixtime: Option<String>,
    #[serde(default)]
    pub file_name: Option<String>,
    // Media fields
    #[serde(default)]
    pub photo: Option<String>,
    #[serde(default)]
    pub media_type: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub sticker_emoji: Option<String>,
    #[serde(default)]
    pub duration_seconds: Option<i64>,
    /// Keep any extra fields around.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl Message {
    /// Determine content-type tag for this message.
    pub fn content_tag(&self) -> Option<&'static str> {
        if self.photo.is_some() && self.media_type.is_none() {
            return Some("image");
        }
        if let Some(ref mt) = self.media_type {
            match mt.as_str() {
                "sticker" => return Some("sticker"),
                "voice_message" => return Some("voice"),
                "video_message" => return Some("video_circle"),
                "video_file" => return Some("video"),
                "audio_file" => return Some("audio"),
                "animation" => return Some("animation"),
                _ => {}
            }
        }
        if let Some(ref mime) = self.mime_type {
            if mime.starts_with("image/") {
                return Some("image");
            }
            if mime.starts_with("video/") {
                return Some("video");
            }
        }
        if self.file_name.is_some() {
            return Some("file");
        }
        // Check for links in text entities
        if self
            .text_entities
            .iter()
            .any(|e| e.entity_type == "link" || e.entity_type == "text_link")
        {
            return Some("link");
        }
        None
    }
}

/// Stats counters for content types.
#[derive(Debug, Clone, Default)]
pub struct ContentStats {
    pub links: usize,
    pub images: usize,
    pub videos: usize,
    pub files: usize,
    pub stickers: usize,
    pub voice: usize,
    pub video_circles: usize,
    pub reposts: usize,
}

impl ContentStats {
    pub fn from_messages(messages: &HashMap<String, Message>) -> Self {
        let mut stats = Self::default();
        for msg in messages.values() {
            if msg.forwarded_from.is_some() {
                stats.reposts += 1;
            }
            match msg.content_tag() {
                Some("link") => stats.links += 1,
                Some("image") => stats.images += 1,
                Some("video") => stats.videos += 1,
                Some("file") => stats.files += 1,
                Some("sticker") => stats.stickers += 1,
                Some("voice") => stats.voice += 1,
                Some("video_circle") => stats.video_circles += 1,
                _ => {}
            }
        }
        stats
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEntity {
    #[serde(default, rename = "type")]
    pub entity_type: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub language: Option<String>,
}

/// Top-level structure of a Telegram export JSON.
#[derive(Debug, Deserialize)]
pub struct TelegramExport {
    #[serde(default)]
    pub name: Option<String>,
    pub messages: Vec<Message>,
}

// ---------------------------------------------------------------------------
// Hashing helpers
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub fn calc_message_key(msg: &Message) -> String {
    let te_prefix: String = serde_json::to_string(&msg.text_entities)
        .unwrap_or_default()
        .chars()
        .take(20)
        .collect();
    let sample = format!(
        "{}{}{}{}{}{}{}",
        msg.msg_type,
        msg.from_id.as_deref().unwrap_or(""),
        msg.date,
        msg.forwarded_from.as_deref().unwrap_or(""),
        msg.edited_unixtime.as_deref().unwrap_or(""),
        te_prefix,
        msg.file_name.as_deref().unwrap_or(""),
    );
    let mut hasher = Md5::new();
    hasher.update(sample.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[allow(dead_code)]
pub fn calc_chain_id(chain: &[Message]) -> String {
    calc_message_key(&chain[0])
}

#[allow(dead_code)]
pub fn calc_chat_id(messages: &HashMap<String, Message>) -> String {
    let first = messages
        .values()
        .min_by_key(|m| m.id)
        .expect("messages must not be empty");
    calc_message_key(first)
}

// ---------------------------------------------------------------------------
// Loading & filtering
// ---------------------------------------------------------------------------

pub fn load_chat_history(path: &Path) -> (Option<String>, HashMap<String, Message>) {
    let data = fs::read_to_string(path).expect("cannot read file");
    let export: TelegramExport = serde_json::from_str(&data).expect("invalid JSON");
    let name = export.name;
    let dict: HashMap<String, Message> = export
        .messages
        .into_iter()
        .map(|m| (m.id.to_string(), m))
        .collect();
    (name, dict)
}

pub fn parse_date(s: &str) -> Option<NaiveDateTime> {
    // "2023-01-15T14:30:00" or "2023-01-15"
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Some(dt);
    }
    if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Some(d.and_hms_opt(0, 0, 0).unwrap());
    }
    None
}

pub fn filter_messages(
    messages: &HashMap<String, Message>,
    start: NaiveDateTime,
    end: NaiveDateTime,
    sender: Option<&str>,
) -> Vec<Message> {
    let mut out: Vec<Message> = messages
        .values()
        .filter(|m| {
            if let Some(dt) = parse_date(&m.date) {
                dt >= start
                    && dt <= end
                    && sender
                        .map(|s| m.from.as_deref() == Some(s))
                        .unwrap_or(true)
            } else {
                false
            }
        })
        .cloned()
        .collect();
    out.sort_by(|a, b| b.id.cmp(&a.id));
    out
}

// ---------------------------------------------------------------------------
// Chain discovery
// ---------------------------------------------------------------------------

pub fn find_all_chains(
    filtered: &[Message],
    messages: &HashMap<String, Message>,
    min_len: usize,
) -> Vec<Vec<Message>> {
    let by_id: HashMap<i64, &Message> = messages.values().map(|m| (m.id, m)).collect();
    let mut msg_to_chain: HashMap<i64, i64> = HashMap::new();
    let mut chain_by_id: HashMap<i64, Vec<Message>> = HashMap::new();

    for msg in filtered {
        if msg_to_chain.contains_key(&msg.id) {
            continue;
        }
        let mut chain = Vec::new();
        let mut visited = Vec::new();
        let mut cur = Some(msg.id);
        while let Some(id) = cur {
            if msg_to_chain.contains_key(&id) {
                break;
            }
            visited.push(id);
            if let Some(m) = by_id.get(&id) {
                chain.push((*m).clone());
                cur = m.reply_to_message_id;
            } else {
                break;
            }
        }
        if chain.len() < min_len {
            continue;
        }
        let chain_id = *visited.last().unwrap();
        for v in &visited {
            msg_to_chain.insert(*v, chain_id);
        }
        let entry = chain_by_id.entry(chain_id).or_default();
        // merge
        let mut merged: HashMap<i64, Message> = entry.iter().map(|m| (m.id, m.clone())).collect();
        for m in chain {
            merged.entry(m.id).or_insert(m);
        }
        let mut merged_vec: Vec<Message> = merged.into_values().collect();
        merged_vec.sort_by_key(|m| m.id);
        *entry = merged_vec;
    }

    let mut keys: Vec<i64> = chain_by_id.keys().copied().collect();
    keys.sort_unstable_by(|a, b| b.cmp(a));
    keys.into_iter()
        .map(|k| chain_by_id.remove(&k).unwrap())
        .collect()
}

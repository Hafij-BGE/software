//! Ask AI: an assistant beside every tab, answering from the document (or data) you're working
//! on through AI providers you choose, with your own keys.
//!
//! - Channels (Settings › Ask AI): OpenRouter, Google Gemini, Groq and Mistral, tried in order;
//!   a channel that hits a limit rests until the limit resets and the next one answers.
//! - The panel (the Ask AI button in the tab bar): a conversation per tab, which sees only that
//!   tab; suggested changes are applied to the tab when you click Apply, and undo with Ctrl+Z.
//! - The data analyser: statistics computed here, from a CSV/TSV file or the sheet; only these
//!   numbers go to the AI.
//! - Private (on by default) keeps the full text, speaker notes and data rows out.
//!
//! Everything lives in this folder; the rest of the suite only calls `logic`, `panel`,
//! `bar_button` and `settings_card`, behind the `ai` Cargo feature. Building with
//! `--no-default-features` leaves Ask AI out entirely; Settings › Ask AI turns it off.

pub mod channels;
mod client;
mod context;
mod keys;
mod markdown;
mod panel;
mod router;
mod settings_ui;
mod stats;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, Sender, channel};

pub use panel::{BUTTON_W, bar_button, panel};
pub use settings_ui::settings_card;

use crate::app::{Content, SuiteApp};
use crate::kind::Kind;
use channels::{Config, now};
use context::Step;
use router::Event;
use stats::Summary;

/// The conversation of the Home and Settings pages (no document).
pub const GENERAL: u64 = 0;

#[derive(Clone, Copy, PartialEq)]
pub enum Role {
    User,
    Assistant,
    /// From the suite: statistics, what was applied.
    Note,
    Error,
}

pub struct Msg {
    pub role: Role,
    pub text: String,
    /// Which provider and model answered.
    pub meta: Option<String>,
    /// Why channels were skipped or switched.
    pub notes: Vec<String>,
    pub steps: Vec<Step>,
    pub refused: Vec<String>,
    /// After Apply (or Dismiss): what happened.
    pub applied: Option<Result<String, String>>,
    /// A data analysis shown as a table.
    pub stats: Option<(String, Summary)>,
}

impl Msg {
    pub fn new(role: Role, text: impl Into<String>) -> Msg {
        Msg { role, text: text.into(), meta: None, notes: Vec::new(), steps: Vec::new(), refused: Vec::new(), applied: None, stats: None }
    }
}

pub struct Chat {
    pub kind: Kind,
    pub msgs: Vec<Msg>,
    pub input: String,
    /// A question on its way: set to cancel (Stop).
    pub pending: Option<Arc<AtomicBool>>,
    pub private: bool,
    /// Attached data: its name and statistics (sent with each question until removed).
    pub data: Option<(String, Summary)>,
    /// Scroll to the newest message.
    pub scroll: bool,
}

pub struct Ai {
    pub config: Config,
    saved: Config,
    pub keys: keys::Keys,
    pub open: bool,
    pub chats: HashMap<u64, Chat>,
    tx: Sender<Event>,
    rx: Receiver<Event>,
    /// Settings: Find models results and Test results per channel (None: running).
    pub models: HashMap<String, Result<Vec<String>, String>>,
    pub finding: HashSet<String>,
    pub tests: HashMap<String, Option<Result<String, String>>>,
    /// Settings: a key being typed, per channel.
    pub key_edit: HashMap<String, String>,
    pub confirm_remove: bool,
}

impl Ai {
    pub fn new() -> Ai {
        let config = Config::load();
        let (tx, rx) = channel();
        let keys = if config.enabled || !config.channels.is_empty() { keys::Keys::start() } else { keys::Keys::start_idle() };
        Ai {
            saved: config.clone(),
            config,
            keys,
            open: false,
            chats: HashMap::new(),
            tx,
            rx,
            models: HashMap::new(),
            finding: HashSet::new(),
            tests: HashMap::new(),
            key_edit: HashMap::new(),
            confirm_remove: false,
        }
    }

    pub fn chat(&mut self, id: u64, kind: Kind) -> &mut Chat {
        let private = self.config.private;
        let chat = self.chats.entry(id).or_insert_with(|| Chat { kind, msgs: Vec::new(), input: String::new(), pending: None, private, data: None, scroll: false });
        chat.kind = kind;
        chat
    }

    /// Channels that can be asked: on, with a key and a model, in order.
    pub fn usable(&self) -> Vec<(channels::Channel, String)> {
        self.config
            .channels
            .iter()
            .filter(|c| c.on && !c.model.trim().is_empty())
            .filter_map(|c| self.keys.get(&c.id).map(|k| (c.clone(), k.to_string())))
            .collect()
    }

    fn handle(&mut self, e: Event) {
        match e {
            Event::Answer { chat, text, provider, model, notes } => {
                if let Some(c) = self.chats.get_mut(&chat) {
                    c.pending = None;
                    let (text, steps, refused) = context::split_steps(c.kind, &text);
                    let mut m = Msg::new(Role::Assistant, text);
                    m.meta = Some(format!("{} · {model}", provider.label()));
                    m.notes = notes;
                    m.steps = steps;
                    m.refused = refused;
                    c.msgs.push(m);
                    c.scroll = true;
                }
            }
            Event::Failed { chat, message, notes } => {
                if let Some(c) = self.chats.get_mut(&chat) {
                    c.pending = None;
                    let mut m = Msg::new(Role::Error, message);
                    m.notes = notes;
                    c.msgs.push(m);
                    c.scroll = true;
                }
            }
            Event::Rest { channel, rest } => {
                self.config.rests.insert(channel, rest);
            }
            Event::Ready { channel } => {
                self.config.rests.remove(&channel);
            }
            Event::Models { channel, result } => {
                self.finding.remove(&channel);
                // A Gemini (or other) channel without a model takes a sensible first one.
                if let Ok(list) = &result
                    && let Some(ch) = self.config.channels.iter_mut().find(|c| c.id == channel)
                    && ch.model.trim().is_empty()
                {
                    let pick = list.iter().find(|m| m.contains("flash") && !m.contains("lite") && !m.contains("image") && !m.contains("preview")).or(list.first());
                    if let Some(m) = pick {
                        ch.model = m.clone();
                    }
                }
                self.models.insert(channel, result);
            }
            Event::Test { channel, result } => {
                self.tests.insert(channel, Some(result));
            }
        }
    }

    pub fn find_models(&mut self, ctx: &egui::Context, id: &str) {
        let Some(ch) = self.config.channels.iter().find(|c| c.id == id).cloned() else { return };
        let key = self.keys.get(id).unwrap_or("").to_string();
        self.finding.insert(id.to_string());
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        std::thread::spawn(move || router::find_models(ch, key, tx, move || ctx.request_repaint()));
    }

    pub fn test(&mut self, ctx: &egui::Context, id: &str) {
        let Some(ch) = self.config.channels.iter().find(|c| c.id == id).cloned() else { return };
        let Some(key) = self.keys.get(id).map(str::to_string) else {
            self.tests.insert(id.to_string(), Some(Err("Add the key first.".into())));
            return;
        };
        self.tests.insert(id.to_string(), None);
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        std::thread::spawn(move || router::test(ch, key, tx, move || ctx.request_repaint()));
    }

    /// Settings › Remove all AI data: keys, channels, chats and `ai.json`; Ask AI stays off.
    pub fn remove_all(&mut self) {
        for c in self.chats.values() {
            if let Some(p) = &c.pending {
                p.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
        self.keys.clear_all();
        self.chats.clear();
        self.models.clear();
        self.tests.clear();
        self.key_edit.clear();
        self.config = Config { enabled: false, ..Config::default() };
        self.open = false;
        Config::delete_file();
        self.saved = self.config.clone();
    }
}

/// Each frame: results from the background threads, saving `ai.json` when it changed.
pub fn logic(app: &mut SuiteApp, ctx: &egui::Context) {
    let ai = &mut app.ai;
    if ai.keys.poll() {
        ctx.request_repaint();
    }
    while let Ok(e) = ai.rx.try_recv() {
        ai.handle(e);
    }
    if !ai.config.enabled {
        ai.open = false;
    }
    // Conversations of closed tabs go with them.
    let ids: HashSet<u64> = app.tabs.iter().map(|t| t.id).collect();
    ai.chats.retain(|id, _| *id == GENERAL || ids.contains(id));
    ai.config.forget_old_rests(now());
    if ai.config != ai.saved {
        ai.config.save();
        ai.saved = ai.config.clone();
    }
}

/// The chat of the active tab: its id (GENERAL for Home and Settings) and kind.
pub fn active_chat(app: &SuiteApp) -> (u64, Kind) {
    match app.tab(app.active).map(|t| (&t.content, t.id)) {
        Some((Content::Doc(d), id)) => (id, d.kind()),
        _ => (GENERAL, Kind::Home),
    }
}

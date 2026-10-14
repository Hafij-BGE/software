//! Asking: the first channel that can answer does. On a limit, a refused key, a missing model or
//! an outage the question moves to the next channel; a channel that hit a limit rests until the
//! limit resets. Runs on a background thread; the window gets events (see `Event`).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use super::channels::{Channel, Provider, Rest, now, when};
use super::client::{self, Failure, Message};

/// Free OpenRouter models tried for one question before moving on.
const AUTO_TRIES: usize = 4;
/// How long OpenRouter's free model list is reused.
const FREE_LIST_SECS: u64 = 3600;

pub enum Event {
    Answer { chat: u64, text: String, provider: Provider, model: String, notes: Vec<String> },
    Failed { chat: u64, message: String, notes: Vec<String> },
    Rest { channel: String, rest: Rest },
    /// A channel answered: any rest it had is over.
    Ready { channel: String },
    Models { channel: String, result: Result<Vec<String>, String> },
    Test { channel: String, result: Result<String, String> },
}

/// What a question needs from the window: the channels in order with their keys and rests.
pub struct Job {
    pub chat: u64,
    pub channels: Vec<(Channel, String)>,
    pub rests: HashMap<String, Rest>,
    pub messages: Vec<Message>,
    pub cancel: Arc<AtomicBool>,
}

/// OpenRouter's free models, ranked, and when they were fetched.
fn free_models(key: &str) -> Result<Vec<String>, String> {
    static CACHE: Mutex<Option<(u64, Vec<String>)>> = Mutex::new(None);
    if let Ok(c) = CACHE.lock()
        && let Some((at, list)) = c.as_ref()
        && now() < at + FREE_LIST_SECS
        && !list.is_empty()
    {
        return Ok(list.clone());
    }
    let list = client::rank_free(&client::list_models(Provider::OpenRouter, key).map_err(|e| e.message)?);
    if list.is_empty() {
        return Err("OpenRouter lists no free models right now".into());
    }
    if let Ok(mut c) = CACHE.lock() {
        *c = Some((now(), list.clone()));
    }
    Ok(list)
}

fn label(ch: &Channel) -> String {
    ch.provider.label().to_string()
}

pub fn ask(job: Job, events: Sender<Event>, repaint: impl Fn()) {
    let mut notes = Vec::new();
    let cancelled = || job.cancel.load(Ordering::Relaxed);
    let send = |e: Event| {
        let _ = events.send(e);
        repaint();
    };
    if job.channels.is_empty() {
        send(Event::Failed { chat: job.chat, message: "No AI channel is ready. Add one, with its key, in Settings › Ask AI.".into(), notes });
        return;
    }
    for (ch, key) in &job.channels {
        let t = now();
        if let Some(rest) = job.rests.get(&ch.id).filter(|r| r.until > t) {
            notes.push(format!("{} skipped: resting until {} ({})", label(ch), when(rest.until, t), rest.reason));
            continue;
        }
        let models: Vec<String> = if ch.provider == Provider::OpenRouter && ch.model.trim() == "auto" {
            match free_models(key) {
                Ok(list) => list.into_iter().take(AUTO_TRIES).collect(),
                Err(e) => {
                    notes.push(format!("{}: {e}", label(ch)));
                    continue;
                }
            }
        } else {
            vec![ch.model.trim().to_string()]
        };
        for model in models {
            if cancelled() {
                return;
            }
            match client::chat(ch.provider, key, &model, &job.messages) {
                Ok(answer) => {
                    if cancelled() {
                        return;
                    }
                    send(Event::Ready { channel: ch.id.clone() });
                    send(Event::Answer { chat: job.chat, text: answer.text, provider: ch.provider, model: answer.model, notes });
                    return;
                }
                Err(e) => {
                    let who = format!("{} ({model})", label(ch));
                    match e.failure {
                        Failure::Busy { until } | Failure::Daily { until } => {
                            notes.push(format!("{who}: {}; resting until {}", e.message, when(until, now())));
                            send(Event::Rest { channel: ch.id.clone(), rest: Rest { until, reason: e.message.clone() } });
                            break;
                        }
                        Failure::BadKey => {
                            notes.push(format!("{who}: {}", e.message));
                            // Until the key is changed or tested again (a year: "until fixed").
                            send(Event::Rest { channel: ch.id.clone(), rest: Rest { until: now() + 365 * 86_400, reason: e.message.clone() } });
                            break;
                        }
                        Failure::Model | Failure::Other => notes.push(format!("{who}: {}", e.message)),
                    }
                }
            }
        }
    }
    if !cancelled() {
        send(Event::Failed { chat: job.chat, message: "No channel could answer.".into(), notes });
    }
}

/// Settings › Find models: the provider's models (OpenRouter: free ones first, "auto" on top).
pub fn find_models(ch: Channel, key: String, events: Sender<Event>, repaint: impl Fn()) {
    let result = client::list_models(ch.provider, &key).map_err(|e| e.message).map(|models| {
        if ch.provider == Provider::OpenRouter {
            let mut ids = vec!["auto".to_string()];
            ids.extend(client::rank_free(&models));
            ids.extend(models.iter().filter(|m| !m.free).map(|m| m.id.clone()));
            ids
        } else {
            models.into_iter().map(|m| m.id).collect()
        }
    });
    let _ = events.send(Event::Models { channel: ch.id, result });
    repaint();
}

/// Settings › Test this channel: a tiny question.
pub fn test(ch: Channel, key: String, events: Sender<Event>, repaint: impl Fn()) {
    let messages = vec![Message { role: "user", content: "Reply with the single word: ready".into() }];
    let models: Vec<String> = if ch.provider == Provider::OpenRouter && ch.model.trim() == "auto" {
        match free_models(&key) {
            Ok(l) => l.into_iter().take(AUTO_TRIES).collect(),
            Err(e) => {
                let _ = events.send(Event::Test { channel: ch.id, result: Err(e) });
                repaint();
                return;
            }
        }
    } else {
        vec![ch.model.trim().to_string()]
    };
    let mut last = String::from("no model to test");
    for model in models {
        match client::chat(ch.provider, &key, &model, &messages) {
            Ok(a) => {
                let _ = events.send(Event::Ready { channel: ch.id.clone() });
                let _ = events.send(Event::Test { channel: ch.id, result: Ok(format!("Works: {} answered.", a.model)) });
                repaint();
                return;
            }
            Err(e) => {
                last = format!("{model}: {}", e.message);
                match e.failure {
                    Failure::Busy { until } | Failure::Daily { until } => {
                        let _ = events.send(Event::Rest { channel: ch.id.clone(), rest: Rest { until, reason: e.message.clone() } });
                        break;
                    }
                    Failure::BadKey => break,
                    _ => {}
                }
            }
        }
    }
    let _ = events.send(Event::Test { channel: ch.id, result: Err(last) });
    repaint();
}

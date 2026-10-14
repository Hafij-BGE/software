//! AI channels: which provider, model and key answer questions, in which order, and which of
//! them are resting after a limit. Stored (without the keys, see `keys`) as `ai.json` beside
//! the suite's settings.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Provider {
    OpenRouter,
    Gemini,
    Groq,
    Mistral,
}

impl Provider {
    pub const ALL: [Provider; 4] = [Provider::OpenRouter, Provider::Gemini, Provider::Groq, Provider::Mistral];

    pub fn label(self) -> &'static str {
        match self {
            Provider::OpenRouter => "OpenRouter",
            Provider::Gemini => "Google Gemini",
            Provider::Groq => "Groq",
            Provider::Mistral => "Mistral",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Provider::OpenRouter => "openrouter",
            Provider::Gemini => "gemini",
            Provider::Groq => "groq",
            Provider::Mistral => "mistral",
        }
    }

    /// The provider's OpenAI-compatible API (chat/completions, models).
    pub fn base_url(self) -> String {
        if let Some(test) = test_base() {
            return format!("{test}/{}", self.slug());
        }
        match self {
            Provider::OpenRouter => "https://openrouter.ai/api/v1",
            Provider::Gemini => "https://generativelanguage.googleapis.com/v1beta/openai",
            Provider::Groq => "https://api.groq.com/openai/v1",
            Provider::Mistral => "https://api.mistral.ai/v1",
        }
        .to_string()
    }

    /// Where to make a key.
    pub fn keys_page(self) -> &'static str {
        match self {
            Provider::OpenRouter => "https://openrouter.ai/keys",
            Provider::Gemini => "https://aistudio.google.com/apikey",
            Provider::Groq => "https://console.groq.com/keys",
            Provider::Mistral => "https://console.mistral.ai/api-keys",
        }
    }

    /// A model to start with ("auto": OpenRouter's free models, strongest first; empty: choose
    /// with Find models).
    pub fn default_model(self) -> &'static str {
        match self {
            Provider::OpenRouter => "auto",
            // Gemini's model names change often: "Find models" picks one.
            Provider::Gemini => "",
            Provider::Groq => "openai/gpt-oss-120b",
            Provider::Mistral => "mistral-small-latest",
        }
    }

    /// When a daily limit resets: midnight Pacific time for Gemini (its documentation), midnight
    /// UTC for the others.
    pub fn next_daily_reset(self, now: u64) -> u64 {
        match self {
            Provider::Gemini => next_pacific_midnight(now),
            _ => (now / DAY + 1) * DAY,
        }
    }
}

/// Mock providers for tests: `CRAFT_SUITE_AI_TEST_BASE=http://127.0.0.1:PORT` sends every
/// request there (`/openrouter/...`, `/gemini/...`). Only addresses on this computer are
/// accepted, so a key never goes anywhere but its own provider or this computer.
pub fn test_base() -> Option<String> {
    let v = std::env::var("CRAFT_SUITE_AI_TEST_BASE").ok()?;
    let local = v.starts_with("http://127.0.0.1:") || v.starts_with("http://localhost:");
    local.then(|| v.trim_end_matches('/').to_string())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    /// Names the channel's key in the keyring.
    pub id: String,
    pub provider: Provider,
    pub model: String,
    pub on: bool,
}

/// A channel that hit a limit, until when, and why.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rest {
    pub until: u64,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Ask AI on or off (Settings › Ask AI). Off: no button, no panel, nothing sent anywhere.
    pub enabled: bool,
    /// New chats start Private: notes, speaker notes, full text and data rows stay out.
    pub private: bool,
    pub channels: Vec<Channel>,
    /// Channel id → resting until.
    pub rests: HashMap<String, Rest>,
    pub panel_width: f32,
}

impl Default for Config {
    fn default() -> Self {
        Config { enabled: true, private: true, channels: Vec::new(), rests: HashMap::new(), panel_width: 380.0 }
    }
}

fn path() -> Option<std::path::PathBuf> {
    if crate::settings::no_prefs() {
        return None;
    }
    Some(crate::settings::config_dir()?.join("craft-suite").join("ai.json"))
}

impl Config {
    pub fn load() -> Config {
        let mut c: Config = path()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).map_err(|e| log::warn!("ai.json: {e}")).ok())
            .unwrap_or_default();
        c.panel_width = c.panel_width.clamp(300.0, 900.0);
        c.forget_old_rests(now());
        c
    }

    pub fn save(&self) {
        let Some(p) = path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(bytes) = serde_json::to_vec_pretty(self)
            && let Err(e) = std::fs::write(&p, bytes)
        {
            log::warn!("{}: {e}", p.display());
        }
    }

    /// Deletes `ai.json` (Settings › Remove all AI data).
    pub fn delete_file() {
        if let Some(p) = path() {
            let _ = std::fs::remove_file(p);
        }
    }

    pub fn forget_old_rests(&mut self, now: u64) {
        self.rests.retain(|_, r| r.until > now);
    }

    pub fn resting(&self, id: &str, now: u64) -> Option<&Rest> {
        self.rests.get(id).filter(|r| r.until > now)
    }
}

pub fn new_id() -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{:x}{:04x}", nanos, std::process::id() & 0xffff)
}

pub const DAY: u64 = 86_400;

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Days since 1970-01-01 → (year, month, day) (Howard Hinnant's civil_from_days).
pub fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// (year, month, day) → days since 1970-01-01.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The `n`th Sunday (1-based) of a month, as days since 1970.
fn nth_sunday(y: i64, m: u32, n: i64) -> i64 {
    let first = days_from_civil(y, m, 1);
    // 1970-01-01 was a Thursday: (days + 4) % 7 == 0 on Sundays.
    let to_sunday = (7 - (first + 4).rem_euclid(7)) % 7;
    first + to_sunday + 7 * (n - 1)
}

/// US Pacific time's offset from UTC at `t`, in hours: -7 from the second Sunday of March 2:00
/// to the first Sunday of November 2:00, -8 otherwise.
fn pacific_offset(t: u64) -> i64 {
    let (y, _, _) = civil((t / DAY) as i64);
    let start = nth_sunday(y, 3, 2) * DAY as i64 + 10 * 3600; // 2:00 PST = 10:00 UTC
    let end = nth_sunday(y, 11, 1) * DAY as i64 + 9 * 3600; // 2:00 PDT = 9:00 UTC
    let t = t as i64;
    if t >= start && t < end { -7 } else { -8 }
}

pub fn next_pacific_midnight(now: u64) -> u64 {
    let local = now as i64 + pacific_offset(now) * 3600;
    let next_local_midnight = (local.div_euclid(DAY as i64) + 1) * DAY as i64;
    // The offset at the new day (it changes at 2:00, never at midnight).
    let guess = next_local_midnight - pacific_offset(now) * 3600;
    (next_local_midnight - pacific_offset(guess as u64) * 3600) as u64
}

/// "14:05" today, else "Tue 00:00", in the computer's local time when known, else UTC.
pub fn when(t: u64, now: u64) -> String {
    let (offset, zone) = match local_offset() {
        Some(o) => (o, ""),
        None => (0, " UTC"),
    };
    let (lt, ln) = (t as i64 + offset, now as i64 + offset);
    let day = DAY as i64;
    let (h, m) = (lt.rem_euclid(day) / 3600, lt.rem_euclid(3600) / 60);
    if lt.div_euclid(day) == ln.div_euclid(day) {
        format!("{h:02}:{m:02}{zone}")
    } else {
        const DAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
        format!("{} {h:02}:{m:02}{zone}", DAYS[lt.div_euclid(day).rem_euclid(7) as usize])
    }
}

/// The computer's UTC offset in seconds, from `date +%z` (asked once); None where unknown.
fn local_offset() -> Option<i64> {
    static OFFSET: std::sync::OnceLock<Option<i64>> = std::sync::OnceLock::new();
    *OFFSET.get_or_init(|| {
        if !cfg!(unix) {
            return None;
        }
        let out = std::process::Command::new("date").arg("+%z").output().ok()?;
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let (h, m) = (s.get(1..3)?.parse::<i64>().ok()?, s.get(3..5)?.parse::<i64>().ok()?);
        Some((h * 3600 + m * 60) * if s.starts_with('-') { -1 } else { 1 })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_round_trip() {
        for d in [-1000, 0, 19_000, 20_740] {
            let (y, m, dd) = civil(d);
            assert_eq!(days_from_civil(y, m, dd), d);
        }
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(days_from_civil(2026, 10, 14), 20_740);
    }

    #[test]
    fn daily_resets() {
        // 2026-10-14 17:00 UTC.
        let now = 20_740 * DAY + 17 * 3600;
        assert_eq!(Provider::Groq.next_daily_reset(now), 20_741 * DAY);
        // Pacific daylight time (UTC-7): midnight PDT on the 15th is 07:00 UTC.
        assert_eq!(Provider::Gemini.next_daily_reset(now), 20_741 * DAY + 7 * 3600);
        // In January (PST, UTC-8): 2026-01-10 03:00 UTC is still the 9th in California.
        let jan = days_from_civil(2026, 1, 10) as u64 * DAY + 3 * 3600;
        assert_eq!(next_pacific_midnight(jan), days_from_civil(2026, 1, 10) as u64 * DAY + 8 * 3600);
    }

    #[test]
    fn test_base_is_local_only() {
        // Not set in tests by default; the check itself:
        let ok = |v: &str| v.starts_with("http://127.0.0.1:") || v.starts_with("http://localhost:");
        assert!(ok("http://127.0.0.1:8080"));
        assert!(!ok("https://evil.example"));
        assert!(!ok("http://127.0.0.1.evil.example"));
    }
}

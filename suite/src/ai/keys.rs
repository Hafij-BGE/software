//! The AI channels' API keys: in the system keyring (the Secret Service of GNOME or KDE on Linux,
//! Credential Manager on Windows), as one entry holding every channel's key. Where there is no
//! keyring, in `ai-keys.json` beside the settings, readable only by you. Never in `ai.json`,
//! the settings, the program or the repository.
//!
//! The keyring is reached on a background thread (unlocking it may show the desktop's prompt).

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};

const SERVICE: &str = "craft-suite";
const USER: &str = "ai-keys";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Backend {
    Keyring,
    File,
    /// `CRAFT_SUITE_NO_PREFS`: kept in memory only.
    Memory,
}

impl Backend {
    pub fn label(self) -> String {
        match self {
            Backend::Keyring => {
                if cfg!(windows) { "Windows Credential Manager".into() } else { "the system keyring (Secret Service)".into() }
            }
            Backend::File => match file_path() {
                Some(p) => format!("a private file, {} (no system keyring found)", p.display()),
                None => "a private file (no system keyring found)".into(),
            },
            Backend::Memory => "memory only (CRAFT_SUITE_NO_PREFS)".into(),
        }
    }
}

pub enum State {
    /// Not read yet: nothing uses a key (Ask AI off, no channels); see `Keys::ensure_started`.
    Idle,
    Loading,
    Ready,
    Failed(String),
}

pub struct Keys {
    pub state: State,
    pub backend: Backend,
    map: HashMap<String, String>,
    rx: Option<Receiver<Result<(Backend, HashMap<String, String>), String>>>,
    saves: Option<Sender<(Backend, HashMap<String, String>)>>,
    /// The last save's outcome, from the background thread.
    saved_rx: Option<Receiver<Result<Backend, String>>>,
    pub save_error: Option<String>,
    /// Remove all AI data ran while the keys were loading: what loads afterwards is dropped.
    cleared: bool,
}

fn file_path() -> Option<std::path::PathBuf> {
    Some(crate::settings::config_dir()?.join("craft-suite").join("ai-keys.json"))
}

fn init_store() -> bool {
    use std::sync::OnceLock;
    static READY: OnceLock<bool> = OnceLock::new();
    *READY.get_or_init(|| {
        #[cfg(all(unix, not(target_os = "macos")))]
        let store = zbus_secret_service_keyring_store::Store::new().map(|s| s as std::sync::Arc<keyring_core::CredentialStore>);
        #[cfg(target_os = "windows")]
        let store = windows_native_keyring_store::Store::new().map(|s| s as std::sync::Arc<keyring_core::CredentialStore>);
        #[cfg(not(any(all(unix, not(target_os = "macos")), target_os = "windows")))]
        let store: keyring_core::Result<std::sync::Arc<keyring_core::CredentialStore>> =
            Err(keyring_core::Error::NotSupportedByStore("no keyring on this system".into()));
        match store {
            Ok(s) => {
                keyring_core::set_default_store(s);
                true
            }
            Err(e) => {
                log::info!("no system keyring: {e}");
                false
            }
        }
    })
}

fn entry() -> Option<keyring_core::Entry> {
    if !init_store() {
        return None;
    }
    keyring_core::Entry::new(SERVICE, USER).ok()
}

fn read_file() -> Option<HashMap<String, String>> {
    let bytes = std::fs::read(file_path()?).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_file(map: &HashMap<String, String>) -> Result<(), String> {
    let p = file_path().ok_or("no folder for settings")?;
    if map.is_empty() {
        let _ = std::fs::remove_file(&p);
        return Ok(());
    }
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec(map).map_err(|e| e.to_string())?;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    use std::io::Write;
    let mut f = opts.open(&p).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = f.set_permissions(std::fs::Permissions::from_mode(0o600));
    }
    f.write_all(&bytes).map_err(|e| e.to_string())
}

/// Reads the keys: the keyring first (moving keys from an older private file into it), else
/// the private file.
fn load() -> Result<(Backend, HashMap<String, String>), String> {
    if let Some(e) = entry() {
        match e.get_password() {
            Ok(json) => return Ok((Backend::Keyring, serde_json::from_str(&json).unwrap_or_default())),
            Err(keyring_core::Error::NoEntry) => {
                let from_file = read_file().unwrap_or_default();
                if !from_file.is_empty() {
                    if e.set_password(&serde_json::to_string(&from_file).unwrap_or_default()).is_ok() {
                        let _ = write_file(&HashMap::new());
                    } else {
                        return Ok((Backend::File, from_file));
                    }
                }
                return Ok((Backend::Keyring, from_file));
            }
            Err(err) => log::info!("system keyring unavailable ({err}); using the private file"),
        }
    }
    Ok((Backend::File, read_file().unwrap_or_default()))
}

fn store(backend: Backend, map: &HashMap<String, String>) -> Result<Backend, String> {
    // No keys left: none in the keyring and none in the file.
    if map.is_empty() {
        if let Some(e) = entry()
            && let Err(err) = e.delete_credential()
            && !matches!(err, keyring_core::Error::NoEntry)
        {
            log::warn!("system keyring: {err}");
        }
        write_file(map)?;
        return Ok(backend);
    }
    if backend == Backend::Keyring
        && let Some(e) = entry()
    {
        match e.set_password(&serde_json::to_string(map).unwrap_or_default()) {
            Ok(()) => return Ok(Backend::Keyring),
            Err(err) => log::warn!("system keyring: {err}; using the private file"),
        }
    }
    write_file(map).map(|()| Backend::File)
}

impl Keys {
    pub fn start() -> Keys {
        if crate::settings::no_prefs() {
            return Keys::start_memory();
        }
        let (tx, rx) = channel();
        std::thread::Builder::new()
            .name("ai-keys-load".into())
            .spawn(move || {
                let _ = tx.send(load());
            })
            .ok();
        // One thread writes, in order.
        let (save_tx, save_rx) = channel::<(Backend, HashMap<String, String>)>();
        let (done_tx, done_rx) = channel();
        std::thread::Builder::new()
            .name("ai-keys-save".into())
            .spawn(move || {
                while let Ok((mut job, mut map)) = save_rx.recv() {
                    // Only the newest of several waiting saves matters.
                    while let Ok(next) = save_rx.try_recv() {
                        (job, map) = next;
                    }
                    let _ = done_tx.send(store(job, &map));
                }
            })
            .ok();
        Keys { state: State::Loading, backend: Backend::Keyring, map: HashMap::new(), rx: Some(rx), saves: Some(save_tx), saved_rx: Some(done_rx), save_error: None, cleared: false }
    }

    /// Keys that aren't read until something needs them (no keyring prompt at start-up).
    pub fn start_idle() -> Keys {
        let mut k = Keys::start_memory();
        k.state = State::Idle;
        k
    }

    fn start_memory() -> Keys {
        Keys { state: State::Ready, backend: Backend::Memory, map: HashMap::new(), rx: None, saves: None, saved_rx: None, save_error: None, cleared: false }
    }

    pub fn ensure_started(&mut self) {
        if matches!(self.state, State::Idle) {
            let typed = std::mem::take(&mut self.map);
            *self = Keys::start();
            // Typed before the keyring was read: merged when it is (see `poll`).
            self.map = typed;
        }
    }

    /// Takes the background threads' results; true when something changed.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        if let Some(rx) = &self.rx
            && let Ok(r) = rx.try_recv()
        {
            match r {
                Ok((backend, map)) => {
                    // Keys typed while loading win.
                    let typed = std::mem::take(&mut self.map);
                    self.map = if self.cleared { HashMap::new() } else { map };
                    let had_typed = !typed.is_empty();
                    self.map.extend(typed);
                    self.backend = backend;
                    self.state = State::Ready;
                    if had_typed {
                        self.save();
                    }
                }
                Err(e) => self.state = State::Failed(e),
            }
            self.rx = None;
            changed = true;
        }
        if let Some(rx) = &self.saved_rx {
            while let Ok(r) = rx.try_recv() {
                match r {
                    Ok(b) => {
                        self.backend = b;
                        self.save_error = None;
                    }
                    Err(e) => self.save_error = Some(e),
                }
                changed = true;
            }
        }
        changed
    }

    pub fn ready(&self) -> bool {
        matches!(self.state, State::Ready)
    }

    pub fn get(&self, channel: &str) -> Option<&str> {
        self.map.get(channel).map(String::as_str).filter(|k| !k.is_empty())
    }

    pub fn set(&mut self, channel: &str, key: &str) {
        self.ensure_started();
        let key = key.trim();
        if key.is_empty() {
            self.map.remove(channel);
        } else {
            self.map.insert(channel.to_string(), key.to_string());
        }
        if self.ready() {
            self.save();
        }
    }

    pub fn remove(&mut self, channel: &str) {
        if self.map.remove(channel).is_some() && self.ready() {
            self.save();
        }
    }

    /// Settings › Remove all AI data: every key, from the keyring and the private file.
    pub fn clear_all(&mut self) {
        self.ensure_started();
        self.map.clear();
        self.cleared = !self.ready();
        if let Some(tx) = &self.saves {
            let _ = tx.send((self.backend, HashMap::new()));
        }
    }

    fn save(&mut self) {
        if let Some(tx) = &self.saves {
            let _ = tx.send((self.backend, self.map.clone()));
        }
    }
}

/// "••••abcd": enough to recognise a key, never the key.
pub fn masked(key: &str) -> String {
    let tail: String = key.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    format!("••••{tail}")
}

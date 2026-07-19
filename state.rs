use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct State {
    pub last_signature: Option<String>,
}

/// Loads state from disk. A missing or unreadable file is treated as "no
/// prior state" rather than an error — the first run on a fresh checkout
/// should just seed itself rather than fail.
pub fn load(path: &Path) -> State {
    match std::fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => State::default(),
    }
}

pub fn save(path: &Path, state: &State) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let contents = serde_json::to_string_pretty(state)?;
    std::fs::write(path, contents)?;
    Ok(())
}

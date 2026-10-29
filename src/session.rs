use std::fs;
use std::path::PathBuf;

use crate::storage;

const MAX_SESSION_TABS: usize = 128;
const MAX_SESSION_LINE_LEN: usize = 128 * 1024;
const MAX_SESSION_FILE_SIZE: usize = 16 * 1024 * 1024;
const MAX_WINDOW_DIMENSION: i32 = 16_384;

#[derive(Clone, Debug)]
pub struct SessionTab {
    pub url: String,
    /// Reading position as `block index + fraction of that block already scrolled past`.
    pub position: f64,
}

#[derive(Clone, Debug)]
pub struct Session {
    pub width: i32,
    pub height: i32,
    pub active: usize,
    pub sidebar_visible: bool,
    pub tree_bookmarks_open: bool,
    pub tree_contents_open: bool,
    pub tree_links_open: bool,
    pub tabs: Vec<SessionTab>,
    /// The appearance the user chose with Ctrl+Shift+D; None follows the desktop.
    pub dark: Option<bool>,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            width: 1100,
            height: 760,
            active: 0,
            sidebar_visible: true,
            tree_bookmarks_open: true,
            tree_contents_open: true,
            tree_links_open: true,
            tabs: Vec::new(),
            dark: None,
        }
    }
}

fn state_dir() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("XDG_STATE_HOME") {
        return Some(PathBuf::from(p).join("silkmark"));
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state/silkmark"))
}

fn path() -> Option<PathBuf> {
    state_dir().map(|p| p.join("session.tsv"))
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(ch),
        }
    }
    out
}

fn unesc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(x) => {
                out.push('\\');
                out.push(x);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn sanitize_scroll(value: f64) -> f64 {
    if value.is_finite() { value.max(0.0) } else { 0.0 }
}

fn parse_text(text: &str) -> Session {
    let mut session = Session::default();
    // Older files stored a pixel offset, which means nothing for a virtualized view.
    let mut block_positions = false;
    for line in text.lines().filter(|line| line.len() <= MAX_SESSION_LINE_LEN) {
        let mut parts = line.splitn(3, '\t');
        let Some(kind) = parts.next() else {
            continue;
        };
        match kind {
            "window" => {
                if let Some(width) = parts.next().and_then(|value| value.parse::<i32>().ok()) {
                    session.width = width.clamp(320, MAX_WINDOW_DIMENSION);
                }
                if let Some(height) = parts.next().and_then(|value| value.parse::<i32>().ok()) {
                    session.height = height.clamp(240, MAX_WINDOW_DIMENSION);
                }
            }
            "active" => {
                session.active = parts.next().and_then(|value| value.parse().ok()).unwrap_or(0);
            }
            "sidebar" => session.sidebar_visible = parts.next() != Some("0"),
            "trees" => {
                let values = parts.next().unwrap_or("");
                let mut values = values.split(',');
                session.tree_bookmarks_open = values.next() != Some("0");
                session.tree_contents_open = values.next() != Some("0");
                session.tree_links_open = values.next() != Some("0");
            }
            "theme" => {
                session.dark = parts.next().and_then(|v| match v {
                    "dark" => Some(true),
                    "light" => Some(false),
                    _ => None,
                })
            }
            "scroll" => block_positions = parts.next() == Some("blocks"),
            "tab" if session.tabs.len() < MAX_SESSION_TABS => {
                let position = parts.next().and_then(|value| value.parse::<f64>().ok()).map(sanitize_scroll).unwrap_or(0.0);
                let url = unesc(parts.next().unwrap_or(""));
                if !url.is_empty() {
                    session.tabs.push(SessionTab { url, position });
                }
            }
            _ => {}
        }
    }
    if !block_positions {
        for tab in &mut session.tabs {
            tab.position = 0.0;
        }
    }
    if session.tabs.is_empty() {
        session.active = 0;
    } else {
        session.active = session.active.min(session.tabs.len() - 1);
    }
    session
}

pub fn load() -> Option<Session> {
    let text = storage::read_text_limited(&path()?, MAX_SESSION_FILE_SIZE).ok()?;
    Some(parse_text(&text))
}

pub fn save(s: &Session) -> Result<(), String> {
    let dir = state_dir().ok_or("Cannot determine XDG state directory")?;
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create session directory: {e}"))?;
    let mut out = String::new();
    out.push_str(&format!(
        "window\t{}\t{}\n",
        s.width.clamp(320, MAX_WINDOW_DIMENSION),
        s.height.clamp(240, MAX_WINDOW_DIMENSION)
    ));
    out.push_str("scroll\tblocks\n");
    if let Some(dark) = s.dark {
        out.push_str(if dark { "theme\tdark\n" } else { "theme\tlight\n" });
    }
    out.push_str(&format!("active\t{}\n", s.active));
    out.push_str(&format!("sidebar\t{}\n", if s.sidebar_visible { 1 } else { 0 }));
    out.push_str(&format!(
        "trees\t{},{},{}\n",
        if s.tree_bookmarks_open { 1 } else { 0 },
        if s.tree_contents_open { 1 } else { 0 },
        if s.tree_links_open { 1 } else { 0 }
    ));
    for tab in s.tabs.iter().take(MAX_SESSION_TABS) {
        if !tab.url.is_empty() {
            out.push_str(&format!("tab\t{:.3}\t{}\n", sanitize_scroll(tab.position), esc(&tab.url)));
        }
    }
    storage::atomic_write(&dir.join("session.tsv"), out.as_bytes()).map_err(|e| format!("Cannot save session: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_corrupt_session_values() {
        let session = parse_text("window\t1\t999999999\nactive\t99\ntab\tNaN\thttps://example.org/a.md\n");
        assert_eq!(session.width, 320);
        assert_eq!(session.height, MAX_WINDOW_DIMENSION);
        assert_eq!(session.active, 0);
        assert_eq!(session.tabs[0].position, 0.0);
    }

    #[test]
    fn pixel_offsets_from_older_sessions_are_dropped() {
        let old = parse_text("tab\t164263.000\tfile:///tmp/a.md\n");
        assert_eq!(old.tabs[0].position, 0.0);
        let new = parse_text("scroll\tblocks\ntab\t42.250\tfile:///tmp/a.md\n");
        assert_eq!(new.tabs[0].position, 42.25);
    }

    #[test]
    fn remembers_the_chosen_appearance() {
        assert_eq!(parse_text("theme\tdark\n").dark, Some(true));
        assert_eq!(parse_text("theme\tlight\n").dark, Some(false));
        assert_eq!(parse_text("theme\tpurple\n").dark, None);
        assert_eq!(parse_text("").dark, None);
    }

    #[test]
    fn ignores_excess_session_tabs() {
        let mut text = String::new();
        for index in 0..(MAX_SESSION_TABS + 20) {
            text.push_str(&format!("tab\t0\tfile:///tmp/{index}.md\n"));
        }
        assert_eq!(parse_text(&text).tabs.len(), MAX_SESSION_TABS);
    }
}

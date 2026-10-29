use crate::bookmarks::{self, Bookmark};
use crate::ffi::*;
use crate::highlight;
use crate::md::{self, Document, Style, TableAlign};
use crate::net;
use crate::session;
use crate::{
    ICON_CLOSE, attach_zoom, c, connect, current_timeval, drop_bookmark_action_ctx, drop_bookmark_edit_ctx, drop_copy_code_ctx,
    drop_link_ctx, drop_tab_close_ctx, drop_tree_toggle_ctx, icon_button, on_bookmark_action, on_bookmark_manager_close,
    on_bookmark_rename, on_copy_code, on_inline_link, on_link_clicked, on_tab_close_clicked, on_tree_toggle, set_entry,
    set_label, set_picture_bytes, set_scaled_pixbuf, short, strip_pango_markup,
};
use std::collections::VecDeque;
use std::ffi::{CStr, c_void};
use std::ptr;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Instant;

const CACHE_PAGES: usize = 8;
const CACHE_IMAGES: usize = 16;
const MAX_IMAGE_CACHE_BYTES: usize = 32 * 1024 * 1024;
const MAX_CONCURRENT_IMAGE_FETCHES: usize = 4;

#[derive(Clone)]
pub(crate) struct CacheEntry {
    pub(crate) url: String,
    pub(crate) title: String,
    pub(crate) doc: Document,
}

/// A document fetched and parsed on a worker thread; only GTK work is left for the UI thread.
pub(crate) struct Fetched {
    pub(crate) final_url: String,
    pub(crate) doc: Document,
    pub(crate) source_bytes: usize,
    pub(crate) parse_elapsed: std::time::Duration,
}

pub(crate) struct Pending {
    pub(crate) tab: usize,
    pub(crate) requested: String,
    pub(crate) rx: Receiver<Result<Fetched, String>>,
}
pub(crate) struct PendingImage {
    pub(crate) page: *mut GtkWidget,
    pub(crate) picture: *mut GtkWidget,
    pub(crate) caption: *mut GtkWidget,
    pub(crate) url: String,
    pub(crate) label: String,
    pub(crate) rx: Receiver<Result<Vec<u8>, String>>,
}
pub(crate) struct QueuedImage {
    pub(crate) page: *mut GtkWidget,
    pub(crate) picture: *mut GtkWidget,
    pub(crate) caption: *mut GtkWidget,
    pub(crate) url: String,
    pub(crate) label: String,
}

pub(crate) struct ImageCacheEntry {
    pub(crate) url: String,
    pub(crate) bytes: Vec<u8>,
}
pub(crate) struct AnimatedImage {
    pub(crate) page: *mut GtkWidget,
    pub(crate) picture: *mut GtkWidget,
    pub(crate) loader: *mut GdkPixbufLoader,
    pub(crate) iter: *mut GdkPixbufAnimationIter,
}

pub(crate) struct Tab {
    pub(crate) title: String,
    pub(crate) url: String,
    pub(crate) back: Vec<String>,
    pub(crate) forward: Vec<String>,
    pub(crate) doc: Document,
    pub(crate) status: String,
    pub(crate) page: *mut GtkWidget,
    pub(crate) tree_scroll: *mut GtkWidget,
    pub(crate) tree_box: *mut GtkWidget,
    /// Contents buttons by heading anchor, valid until the sidebar is rebuilt.
    pub(crate) tree_items: Vec<(String, *mut GtkWidget)>,
    pub(crate) doc_view: *mut GtkWidget,
    pub(crate) doc_ctx: *mut docview::DocViewCtx,
    pub(crate) doc_scroll: *mut GtkWidget,
    pub(crate) restore_scroll: Option<f64>,
    pub(crate) tab_label: *mut GtkWidget,
    pub(crate) search: String,
    pub(crate) search_index: usize,
    pub(crate) search_hits: usize,
    pub(crate) search_spans: Vec<docview::HitSpan>,
}

#[derive(Clone, Copy)]
pub(crate) struct AppWidgets {
    pub(crate) window: *mut GtkWidget,
    pub(crate) notebook: *mut GtkWidget,
    pub(crate) entry: *mut GtkWidget,
    pub(crate) status: *mut GtkWidget,
    pub(crate) find_bar: *mut GtkWidget,
    pub(crate) find_entry: *mut GtkWidget,
    pub(crate) find_count: *mut GtkWidget,
}

pub(crate) struct App {
    pub(crate) window: *mut GtkWidget,
    pub(crate) notebook: *mut GtkWidget,
    pub(crate) entry: *mut GtkWidget,
    pub(crate) status: *mut GtkWidget,
    pub(crate) main_loop: *mut GMainLoop,
    pub(crate) tabs: Vec<Tab>,
    pub(crate) active: usize,
    pub(crate) pending: Vec<Pending>,
    pub(crate) pending_images: Vec<PendingImage>,
    pub(crate) queued_images: VecDeque<QueuedImage>,
    pub(crate) animations: Vec<AnimatedImage>,
    pub(crate) cache: VecDeque<CacheEntry>,
    pub(crate) image_cache: VecDeque<ImageCacheEntry>,
    pub(crate) image_cache_bytes: usize,
    pub(crate) sidebar_visible: bool,
    pub(crate) bookmarks: Vec<Bookmark>,
    pub(crate) verbose: bool,
    pub(crate) tree_bookmarks_open: bool,
    pub(crate) tree_contents_open: bool,
    pub(crate) tree_links_open: bool,
    pub(crate) find_bar: *mut GtkWidget,
    pub(crate) find_entry: *mut GtkWidget,
    pub(crate) find_count: *mut GtkWidget,
    pub(crate) closed_tabs: Vec<String>,
    pub(crate) bookmark_window: *mut GtkWidget,
    pub(crate) bookmark_box: *mut GtkWidget,
    pub(crate) completion_seed: String,
    pub(crate) completion_items: Vec<String>,
    pub(crate) completion_index: usize,
    pub(crate) g_pending: bool,
    pub(crate) reader_zoom: i32,
    pub(crate) stats: bool,
    /// Current appearance, and the user's explicit choice (None: following the desktop).
    pub(crate) dark: bool,
    pub(crate) dark_chosen: Option<bool>,
}

pub(crate) struct LinkCtx {
    pub(crate) app: *mut App,
    pub(crate) target: String,
}
pub(crate) struct TabCloseCtx {
    pub(crate) app: *mut App,
    pub(crate) page: *mut GtkWidget,
}
pub(crate) struct ZoomCtx {
    pub(crate) bytes: Vec<u8>,
    pub(crate) title: String,
}
pub(crate) struct ViewerCtx {
    pub(crate) picture: *mut GtkWidget,
    pub(crate) bytes: Vec<u8>,
    pub(crate) scale: f64,
    pub(crate) fit_scale: f64,
}
#[derive(Clone, Copy)]
pub(crate) enum TreeSection {
    Bookmarks,
    Contents,
    Links,
}
pub(crate) struct TreeToggleCtx {
    pub(crate) app: *mut App,
    pub(crate) section: TreeSection,
}
#[derive(Clone, Copy)]
pub(crate) enum BookmarkAction {
    Open,
    Up,
    Down,
    Delete,
}
pub(crate) struct BookmarkEditCtx {
    pub(crate) app: *mut App,
    pub(crate) url: String,
}
pub(crate) struct BookmarkActionCtx {
    pub(crate) app: *mut App,
    pub(crate) url: String,
    pub(crate) action: BookmarkAction,
}
pub(crate) struct CopyCodeCtx {
    pub(crate) app: *mut App,
    pub(crate) text: String,
}

mod bookmarks_ui;
mod cache;
mod core;
mod docview;
mod navigation;
mod render;

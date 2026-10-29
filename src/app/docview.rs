//! Virtualized document view.
//!
//! Every tab shows its document in a `GtkListView` with one row per `md::Block`. GTK only
//! creates widgets for the rows that are (nearly) visible and recycles them while scrolling,
//! so opening or searching a book-length document does not build thousands of widgets.
use super::*;
use crate::md::Block;
use std::collections::{HashMap, VecDeque};
use std::ffi::{c_char, c_int};

/// A run of search hits inside one block. `first` is the global index of its first hit.
#[derive(Clone, Copy)]
pub(crate) struct HitSpan {
    pub(crate) block: usize,
    pub(crate) first: usize,
    pub(crate) count: usize,
}

struct BoundRow {
    block: usize,
    item: *mut GtkListItem,
    widget: *mut GtkWidget,
    /// Picture widget of an image row, otherwise null.
    picture: *mut GtkWidget,
}

/// Per-tab state shared with the list view factory callbacks.
pub(crate) struct DocViewCtx {
    app: *mut App,
    page: *mut GtkWidget,
    bound: Vec<BoundRow>,
    /// Row widgets by block, kept (referenced) after their list item let go of them. GTK
    /// recycles list items constantly while scrolling; rebuilding the same rows each time made
    /// scrolling stutter, reusing them costs next to nothing.
    cache: HashMap<usize, (*mut GtkWidget, *mut GtkWidget)>,
    order: VecDeque<usize>,
}

const ROW_CACHE: usize = 1500;

impl DocViewCtx {
    /// The cached widget of a block, built on first use.
    unsafe fn row(&mut self, app: &mut App, tab: usize, block: usize) -> (*mut GtkWidget, *mut GtkWidget) {
        if let Some(&(widget, picture)) = self.cache.get(&block) {
            self.order.retain(|b| *b != block);
            self.order.push_back(block);
            return (widget, picture);
        }
        let (widget, picture) = app.build_row(tab, block);
        g_object_ref_sink(widget as *mut c_void);
        self.cache.insert(block, (widget, picture));
        self.order.push_back(block);
        self.evict(app);
        (widget, picture)
    }

    /// Drops the oldest cached rows that no list item is showing.
    unsafe fn evict(&mut self, app: &mut App) {
        let mut scanned = 0;
        while self.cache.len() > ROW_CACHE && scanned < self.order.len() {
            scanned += 1;
            let Some(block) = self.order.pop_front() else {
                break;
            };
            if self.bound.iter().any(|b| b.block == block) {
                self.order.push_back(block);
                continue;
            }
            self.forget(app, block);
        }
    }

    unsafe fn forget(&mut self, app: &mut App, block: usize) {
        if let Some((widget, picture)) = self.cache.remove(&block) {
            self.order.retain(|b| *b != block);
            if !picture.is_null() {
                app.release_row_picture(picture);
            }
            g_object_unref(widget as *mut c_void);
        }
    }

    /// Forgets every row that is not on screen, e.g. after the search highlight changed.
    unsafe fn forget_unbound(&mut self, app: &mut App) {
        let stale: Vec<usize> = self.cache.keys().copied().filter(|b| !self.bound.iter().any(|r| r.block == *b)).collect();
        for block in stale {
            self.forget(app, block);
        }
    }
}

struct GraphViewCtx {
    graph: crate::graph::Graph,
    area: *mut GtkWidget,
    scrolled: *mut GtkWidget,
    natural_w: i32,
    natural_h: i32,
    zoom: f64,
}

unsafe fn apply_graph_zoom(ctx: &mut GraphViewCtx, zoom: f64) {
    ctx.zoom = zoom.clamp(0.25, 2.0);
    let width = ((ctx.natural_w as f64) * ctx.zoom).round() as i32;
    let height = ((ctx.natural_h as f64) * ctx.zoom).round() as i32;
    let drawing_width = width.max(180);
    let drawing_height = height.max(100);
    gtk_widget_set_size_request(ctx.area, drawing_width, drawing_height);

    // GtkScrolledWindow tends to collapse to a very small natural height when its
    // child is a GtkDrawingArea. Give diagram viewports an explicit content-sized
    // height, capped so genuinely large diagrams still scroll inside the document.
    if !ctx.scrolled.is_null() {
        const MAX_DIAGRAM_VIEWPORT_HEIGHT: i32 = 720;
        const VIEWPORT_PADDING: i32 = 16;
        let viewport_height = drawing_height.saturating_add(VIEWPORT_PADDING).min(MAX_DIAGRAM_VIEWPORT_HEIGHT);
        gtk_widget_set_size_request(ctx.scrolled, -1, viewport_height);
    }

    gtk_widget_queue_draw(ctx.area);
}

unsafe extern "C" fn draw_graph(_area: *mut GtkDrawingArea, cr: *mut cairo_t, width: i32, height: i32, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let ctx = &*(data as *const GraphViewCtx);
    crate::graph::draw(&ctx.graph, cr, width, height);
}

unsafe extern "C" fn graph_zoom_out(_button: *mut GtkButton, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let ctx = &mut *(data as *mut GraphViewCtx);
    apply_graph_zoom(ctx, ctx.zoom / 1.25);
}

unsafe extern "C" fn graph_zoom_reset(_button: *mut GtkButton, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let ctx = &mut *(data as *mut GraphViewCtx);
    apply_graph_zoom(ctx, 1.0);
}

unsafe extern "C" fn graph_zoom_in(_button: *mut GtkButton, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let ctx = &mut *(data as *mut GraphViewCtx);
    apply_graph_zoom(ctx, ctx.zoom * 1.25);
}

unsafe extern "C" fn graph_zoom_fit(_button: *mut GtkButton, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let ctx = &mut *(data as *mut GraphViewCtx);
    let fit = (1000.0 / ctx.natural_w.max(1) as f64).min(700.0 / ctx.natural_h.max(1) as f64).min(1.0);
    apply_graph_zoom(ctx, fit);
}

unsafe extern "C" fn drop_graph_view(data: *mut c_void) {
    if !data.is_null() {
        drop(Box::from_raw(data as *mut GraphViewCtx));
    }
}

const ROW_SPACING: i32 = 2;
const ROW_TOP_GAP: f64 = 12.0;
const JUMP_TICK_MS: guint = 30;
const JUMP_MAX_TICKS: u32 = 100;
const DOC_TOP_MARGIN: i32 = 28;
const DOC_BOTTOM_MARGIN: i32 = 40;
const INDENT_PX: i32 = 22;
/// Tables whose narrowest layout is at most this wide wrap instead of scrolling sideways.
const TABLE_NO_SCROLL_MAX_WIDTH: i32 = 480;

unsafe extern "C" fn on_row_setup(_factory: *mut c_void, item: *mut GtkListItem, _data: *mut c_void) {
    gtk_list_item_set_activatable(item, 0);
    gtk_list_item_set_selectable(item, 0);
}

unsafe extern "C" fn on_row_bind(_factory: *mut c_void, item: *mut GtkListItem, data: *mut c_void) {
    let ctx = &mut *(data as *mut DocViewCtx);
    if ctx.app.is_null() {
        return;
    }
    let app = &mut *ctx.app;
    let Some(tab) = app.tabs.iter().position(|t| t.page == ctx.page) else {
        return;
    };
    let block = gtk_list_item_get_position(item) as usize;
    let (widget, picture) = ctx.row(app, tab, block);
    gtk_list_item_set_child(item, widget);
    ctx.bound.push(BoundRow { block, item, widget, picture });
}

unsafe extern "C" fn on_row_unbind(_factory: *mut c_void, item: *mut GtkListItem, data: *mut c_void) {
    let ctx = &mut *(data as *mut DocViewCtx);
    gtk_list_item_set_child(item, ptr::null_mut());
    let Some(pos) = ctx.bound.iter().position(|b| b.item == item) else {
        return;
    };
    // The cache still holds the widget; the list item only lets go of it.
    ctx.bound.swap_remove(pos);
}

struct JumpCtx {
    app: *mut App,
    page: *mut GtkWidget,
    block: usize,
    scrolled_past: Option<f64>,
    scrolled: bool,
    tries: u32,
}

unsafe extern "C" fn jump_tick(data: *mut c_void) -> gboolean {
    let ctx = &mut *(data as *mut JumpCtx);
    let app = &mut *ctx.app;
    ctx.tries += 1;
    let done = match app.tabs.iter().position(|t| t.page == ctx.page) {
        // The tab is gone, or the layout never came: give up.
        None => true,
        Some(_) if ctx.tries >= JUMP_MAX_TICKS => true,
        Some(idx) if !ctx.scrolled => {
            if gtk_widget_get_height(app.tabs[idx].doc_view) > 0 {
                app.scroll_to_block(idx, ctx.block);
                ctx.scrolled = true;
            }
            false
        }
        Some(idx) => app.align_bound_row(idx, ctx.block, ctx.scrolled_past),
    };
    if done {
        drop(Box::from_raw(data as *mut JumpCtx));
        0
    } else {
        1
    }
}

unsafe extern "C" fn drop_doc_view_ctx(data: *mut c_void, _closure: *mut c_void) {
    if !data.is_null() {
        drop(Box::from_raw(data as *mut DocViewCtx));
    }
}

/// Creates the list view of a tab. The returned context pointer is owned by the factory.
pub(crate) unsafe fn new_doc_view(app: *mut App, page: *mut GtkWidget) -> (*mut GtkWidget, *mut DocViewCtx) {
    let factory = gtk_signal_list_item_factory_new();
    let ctx = Box::into_raw(Box::new(DocViewCtx { app, page, bound: Vec::new(), cache: HashMap::new(), order: VecDeque::new() }));
    connect(factory as *mut GtkWidget, "setup", on_row_setup as *const c_void, ctx as *mut c_void, None);
    connect(factory as *mut GtkWidget, "bind", on_row_bind as *const c_void, ctx as *mut c_void, None);
    connect(factory as *mut GtkWidget, "unbind", on_row_unbind as *const c_void, ctx as *mut c_void, Some(drop_doc_view_ctx));
    let view = gtk_list_view_new(empty_model(0), factory);
    (view, ctx)
}

/// A model of `n` rows. The rows carry no data; the factory maps the position to a block.
unsafe fn empty_model(n: usize) -> *mut GtkNoSelection {
    let empty: *const c_char = c"".as_ptr();
    let mut items: Vec<*const c_char> = vec![empty; n];
    items.push(ptr::null());
    gtk_no_selection_new(gtk_string_list_new(items.as_ptr()) as *mut GListModel)
}

type ScrollToFn = unsafe extern "C" fn(*mut GtkListView, guint, c_int, *mut c_void);

unsafe extern "C" {
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

/// `gtk_list_view_scroll_to` needs GTK 4.12, older runtimes get the adjustment fallback.
unsafe fn list_view_scroll_to() -> Option<ScrollToFn> {
    let sym = dlsym(ptr::null_mut(), c"gtk_list_view_scroll_to".as_ptr());
    if sym.is_null() { None } else { Some(std::mem::transmute::<*mut c_void, ScrollToFn>(sym)) }
}

impl App {
    /// Installs the tab's document as the list view model and moves to the requested position.
    pub(crate) unsafe fn show_document(&mut self, idx: usize) {
        let blocks = self.tabs[idx].doc.index.blocks.len();
        gtk_list_view_set_model(self.tabs[idx].doc_view as *mut GtkListView, empty_model(blocks));
        // The old rows were unbound by the new model and belong to the previous document.
        let ctx = self.tabs[idx].doc_ctx;
        (*ctx).forget_unbound(self);
        // Binding happens lazily from the layout pass.
        self.recompute_search(idx);
        match self.tabs[idx].restore_scroll.take() {
            Some(position) if position.floor() < blocks as f64 => {
                self.jump_to_block(idx, position.floor() as usize, Some(position.fract()));
            }
            _ => self.scroll_to_target(idx),
        }
    }

    /// Current reading position as `block + fraction scrolled past`, see `session::SessionTab`.
    pub(crate) unsafe fn reading_position(&self, idx: usize) -> f64 {
        let tab = &self.tabs[idx];
        let bound = &(*tab.doc_ctx).bound;
        let x = gtk_widget_get_width(tab.doc_view) as f64 / 2.0;
        // GTK keeps recycled rows bound with stale positions, so ask what is actually on
        // screen near the top edge instead of trusting every bound row.
        for probe in [2.0, 10.0, 24.0, 48.0] {
            let mut widget = gtk_widget_pick(tab.doc_view, x, probe, 0);
            while !widget.is_null() {
                if let Some(row) = bound.iter().find(|b| b.widget == widget) {
                    let height = gtk_widget_get_height(row.widget) as f64;
                    let mut out = GraphenePoint::default();
                    let origin = GraphenePoint::default();
                    if height > 0.0 && gtk_widget_compute_point(row.widget, tab.doc_view, &origin, &mut out) != 0 {
                        return row.block as f64 + ((-out.y as f64) / height).clamp(0.0, 0.999);
                    }
                    break;
                }
                widget = gtk_widget_get_parent(widget);
            }
        }
        tab.restore_scroll.unwrap_or(0.0)
    }

    /// After a page loads the focus would otherwise stay in the address bar, where Space,
    /// Page Down and the arrow keys edit the URL instead of scrolling the page.
    pub(crate) unsafe fn focus_document_if_idle(&mut self, idx: usize) {
        if idx != self.active || gtk_widget_has_focus(self.entry) == 0 {
            return;
        }
        let text = gtk_editable_get_text(self.entry as *mut GtkEditable);
        let typed = if text.is_null() { String::new() } else { CStr::from_ptr(text).to_string_lossy().into_owned() };
        // An address that differs from the page's URL is being edited: leave it alone.
        if typed == md::display_url(&self.tabs[idx].url) || typed.is_empty() {
            gtk_widget_grab_focus(self.tabs[idx].doc_view);
        }
    }

    /// Scrolls to the current search hit, else to the heading named by the URL fragment.
    pub(crate) unsafe fn scroll_to_target(&mut self, idx: usize) {
        let tab = &self.tabs[idx];
        let hit_block = tab
            .search_spans
            .iter()
            .find(|s| tab.search_index >= s.first && tab.search_index < s.first + s.count)
            .map(|s| s.block);
        let anchor_block = md::fragment_id(&tab.url).and_then(|a| tab.doc.index.anchors.get(&a).copied());
        if let Some(block) = hit_block {
            self.scroll_to_block(idx, block);
        } else if let Some(block) = anchor_block {
            self.jump_to_block(idx, block, None);
        }
    }

    /// Brings a block to the top of the view once the list view has been laid out.
    /// `scroll_to` alone only makes a row visible, usually at the bottom edge, and is lost if it
    /// is issued before the first layout. So a short timer waits for the layout, scrolls, then
    /// waits for the row itself and moves it to the top: a section jump (`scrolled_past` None)
    /// shows the heading first, a restored position shows the row scrolled by that fraction.
    unsafe fn jump_to_block(&mut self, idx: usize, block: usize, scrolled_past: Option<f64>) {
        let ctx = Box::new(JumpCtx {
            app: self as *mut App,
            page: self.tabs[idx].page,
            block,
            scrolled_past,
            scrolled: false,
            tries: 0,
        });
        g_timeout_add(JUMP_TICK_MS, jump_tick, Box::into_raw(ctx) as *mut c_void);
    }

    /// Returns false while the row is not bound yet.
    unsafe fn align_bound_row(&mut self, idx: usize, block: usize, scrolled_past: Option<f64>) -> bool {
        let ctx = self.tabs[idx].doc_ctx;
        let Some(widget) = (*ctx).bound.iter().find(|b| b.block == block).map(|b| b.widget) else {
            return false;
        };
        // A row that has not been allocated yet reports height 0 and position 0; moving the
        // adjustment now would cancel the pending `scroll_to`.
        if gtk_widget_get_height(widget) == 0 {
            return false;
        }
        let mut out = GraphenePoint::default();
        let origin = GraphenePoint::default();
        if gtk_widget_compute_point(widget, self.tabs[idx].doc_view, &origin, &mut out) == 0 {
            return false;
        }
        let adj = gtk_scrolled_window_get_vadjustment(self.tabs[idx].doc_scroll as *mut GtkScrolledWindow);
        if !adj.is_null() {
            let shift = match scrolled_past {
                Some(fraction) => fraction * (gtk_widget_get_height(widget) as f64).max(1.0),
                None => -ROW_TOP_GAP,
            };
            let target = gtk_adjustment_get_value(adj) + out.y as f64 + shift;
            gtk_adjustment_set_value(adj, target.max(0.0));
        }
        true
    }

    pub(crate) unsafe fn scroll_to_block(&self, idx: usize, block: usize) {
        let view = self.tabs[idx].doc_view;
        if let Some(scroll_to) = list_view_scroll_to() {
            scroll_to(view as *mut GtkListView, block as guint, 0, ptr::null_mut());
            return;
        }
        let total = self.tabs[idx].doc.index.blocks.len().max(1);
        let adj = gtk_scrolled_window_get_vadjustment(self.tabs[idx].doc_scroll as *mut GtkScrolledWindow);
        if !adj.is_null() {
            let span = (gtk_adjustment_get_upper(adj) - gtk_adjustment_get_page_size(adj)).max(0.0);
            gtk_adjustment_set_value(adj, span * block as f64 / total as f64);
        }
    }

    /// Recounts search hits from the precomputed block text. No widgets are touched.
    pub(crate) unsafe fn recompute_search(&mut self, idx: usize) {
        let needle = self.tabs[idx].search.to_lowercase();
        let index = &self.tabs[idx].doc.index;
        let mut spans = Vec::new();
        let mut total = 0usize;
        if !needle.is_empty() {
            for (block, (kind, text)) in index.blocks.iter().zip(&index.search_text).enumerate() {
                let count = md::count_hits(text, &needle, matches!(kind, Block::Table { .. }));
                if count > 0 {
                    spans.push(HitSpan { block, first: total, count });
                    total += count;
                }
            }
        }
        let tab = &mut self.tabs[idx];
        tab.search_spans = spans;
        tab.search_hits = total;
        if total > 0 && tab.search_index >= total {
            tab.search_index = total - 1;
        }
    }

    /// Rebuilds the visible rows, e.g. after the search highlight moved.
    pub(crate) unsafe fn refresh_rows(&mut self, idx: usize) {
        let ctx = self.tabs[idx].doc_ctx;
        let rows: Vec<(*mut GtkListItem, usize)> = (*ctx).bound.iter().map(|b| (b.item, b.block)).collect();
        // Cached rows carry the old highlight: drop them, then rebuild what is on screen.
        (*ctx).forget_unbound(self);
        for (item, block) in rows {
            (*ctx).forget(self, block);
            let (widget, picture) = (*ctx).row(self, idx, block);
            gtk_list_item_set_child(item, widget);
            if let Some(row) = (*ctx).bound.iter_mut().find(|b| b.item == item) {
                row.picture = picture;
                row.widget = widget;
            }
        }
    }

    /// Stops loading and animating an image whose row was recycled.
    pub(crate) unsafe fn release_row_picture(&mut self, picture: *mut GtkWidget) {
        if let Some(pos) = self.queued_images.iter().position(|q| q.picture == picture)
            && let Some(q) = self.queued_images.remove(pos)
        {
            g_object_unref(q.picture as *mut c_void);
            g_object_unref(q.caption as *mut c_void);
        }
        if let Some(pos) = self.animations.iter().position(|a| a.picture == picture) {
            let a = self.animations.remove(pos);
            g_object_unref(a.picture as *mut c_void);
            g_object_unref(a.iter as *mut c_void);
            g_object_unref(a.loader as *mut c_void);
        }
    }

    fn hit_span(&self, idx: usize, block: usize) -> Option<HitSpan> {
        let spans = &self.tabs[idx].search_spans;
        spans.binary_search_by_key(&block, |s| s.block).ok().map(|i| spans[i])
    }

    /// Builds the widget of one block. The second value is the picture of an image row.
    unsafe fn build_row(&mut self, idx: usize, block: usize) -> (*mut GtkWidget, *mut GtkWidget) {
        let mut picture = ptr::null_mut();
        let widget = match self.tabs[idx].doc.index.blocks.get(block).copied() {
            Some(Block::Table { first, end }) => self.build_table(idx, block, first, end),
            Some(Block::Line(i)) => {
                let line = self.tabs[idx].doc.lines[i].clone();
                match line.style {
                    Style::Image => {
                        let (wrap, pic) = self.build_image(idx, &line);
                        picture = pic;
                        wrap
                    }
                    Style::Rule => build_rule(),
                    Style::Diagram => self.build_diagram(idx, block, &line),
                    Style::Code => self.build_code(idx, block, &line),
                    _ => self.build_text(idx, block, &line),
                }
            }
            None => gtk_box_new(GTK_ORIENTATION_VERTICAL, 0),
        };
        gtk_widget_set_margin_bottom(widget, ROW_SPACING);
        if block == 0 {
            gtk_widget_set_margin_top(widget, DOC_TOP_MARGIN);
        }
        if block + 1 == self.tabs[idx].doc.index.blocks.len() {
            gtk_widget_set_margin_bottom(widget, DOC_BOTTOM_MARGIN);
        }
        (widget, picture)
    }

    /// Marks a widget as a search hit and returns whether it is the current one.
    unsafe fn mark_hit(&self, idx: usize, widget: *mut GtkWidget, global_hit: usize) {
        gtk_widget_add_css_class(widget, c("search-hit").as_ptr());
        if global_hit == self.tabs[idx].search_index {
            gtk_widget_add_css_class(widget, c("search-current").as_ptr());
        }
    }

    unsafe fn build_table(&mut self, idx: usize, block: usize, first: usize, end: usize) -> *mut GtkWidget {
        let needle = self.tabs[idx].search.to_lowercase();
        let span_first = self.hit_span(idx, block).map(|s| s.first);
        let lines = &self.tabs[idx].doc.lines;
        let align = lines[first + 1].table.as_ref().map(|t| t.align.clone()).unwrap_or_default();
        let mut rows: Vec<Vec<String>> = vec![lines[first].table.as_ref().map(|t| t.cells.clone()).unwrap_or_default()];
        for line in &lines[first + 2..end] {
            rows.push(line.table.as_ref().map(|t| t.cells.clone()).unwrap_or_default());
        }

        let sc = gtk_scrolled_window_new();
        gtk_scrolled_window_set_policy(sc as *mut GtkScrolledWindow, GTK_POLICY_AUTOMATIC, GTK_POLICY_NEVER);
        gtk_widget_add_css_class(sc, c("md-table-scroll").as_ptr());
        let grid = gtk_grid_new();
        gtk_widget_add_css_class(grid, c("md-table").as_ptr());
        gtk_grid_set_row_spacing(grid as *mut GtkGrid, 1);
        gtk_grid_set_column_spacing(grid as *mut GtkGrid, 1);

        let mut local_hit = 0usize;
        for (r, cells) in rows.iter().enumerate() {
            for (col, cell) in cells.iter().enumerate() {
                let label = gtk_label_new(ptr::null());
                let (markup, _) = md::parse_inline_public(cell);
                gtk_label_set_markup(label as *mut GtkLabel, c(&markup).as_ptr());
                gtk_label_set_wrap(label as *mut GtkLabel, 1);
                gtk_label_set_selectable(label as *mut GtkLabel, 1);
                gtk_label_set_width_chars(label as *mut GtkLabel, 8);
                gtk_label_set_max_width_chars(label as *mut GtkLabel, 48);
                let xa = match align.get(col).copied().unwrap_or(TableAlign::Left) {
                    TableAlign::Left => 0.0,
                    TableAlign::Center => 0.5,
                    TableAlign::Right => 1.0,
                };
                gtk_label_set_xalign(label as *mut GtkLabel, xa);
                gtk_widget_set_hexpand(label, 1);
                gtk_widget_add_css_class(label, c(if r == 0 { "md-table-header" } else { "md-table-cell" }).as_ptr());
                connect(label, "activate-link", on_inline_link as *const c_void, self as *mut App as *mut c_void, None);
                if let Some(base) = span_first
                    && md::strip_inline_markup(cell).to_lowercase().contains(&needle)
                {
                    self.mark_hit(idx, label, base + local_hit);
                    local_hit += 1;
                }
                gtk_grid_attach(grid as *mut GtkGrid, label, col as i32, r as i32, 1, 1);
            }
        }
        gtk_scrolled_window_set_child(sc as *mut GtkScrolledWindow, grid);
        // A horizontally scrollable window asks its child for the height at the child's narrowest
        // width, far taller than the table needs, and the rest shows as a grey gap below the
        // last row. A table that fits when its cells wrap does not need to scroll sideways.
        let (mut narrowest, mut natural) = (0, 0);
        gtk_widget_measure(grid, GTK_ORIENTATION_HORIZONTAL, -1, &mut narrowest, &mut natural, ptr::null_mut(), ptr::null_mut());
        if narrowest <= TABLE_NO_SCROLL_MAX_WIDTH {
            gtk_scrolled_window_set_policy(sc as *mut GtkScrolledWindow, GTK_POLICY_NEVER, GTK_POLICY_NEVER);
        }
        sc
    }

    unsafe fn build_image(&mut self, idx: usize, line: &md::Line) -> (*mut GtkWidget, *mut GtkWidget) {
        let wrap = gtk_box_new(GTK_ORIENTATION_VERTICAL, 3);
        let Some(image) = line.image.clone() else {
            return (wrap, ptr::null_mut());
        };
        gtk_widget_add_css_class(wrap, c("md-image-wrap").as_ptr());
        let picture = gtk_picture_new();
        gtk_picture_set_can_shrink(picture as *mut GtkPicture, 1);
        gtk_picture_set_keep_aspect_ratio(picture as *mut GtkPicture, 1);
        gtk_widget_set_hexpand(picture, 1);
        gtk_widget_set_size_request(picture, 1, 1);
        gtk_picture_set_alternative_text(picture as *mut GtkPicture, c(&image.label).as_ptr());
        if let Some(tip) = &image.title {
            gtk_widget_set_tooltip_text(picture, c(tip).as_ptr());
        }
        let caption_text =
            if image.label.is_empty() { "Loading image…".to_string() } else { format!("Loading image: {}", image.label) };
        let caption = gtk_label_new(c(&caption_text).as_ptr());
        gtk_label_set_xalign(caption as *mut GtkLabel, 0.0);
        gtk_widget_add_css_class(caption, c("md-image-caption").as_ptr());
        gtk_box_append(wrap as *mut GtkBox, picture);
        gtk_box_append(wrap as *mut GtkBox, caption);
        let Some(url) = net::resolve_resource(&self.tabs[idx].url, &image.target, false) else {
            set_label(caption, "[unsupported image location]");
            return (wrap, ptr::null_mut());
        };
        if let Some(bytes) = self.image_cache_get(&url) {
            if self.verbose {
                eprintln!("[cache:image] {url}");
            }
            match set_picture_bytes(picture, &bytes) {
                Ok(Some((loader, iter))) => {
                    g_object_ref(picture as *mut c_void);
                    self.animations.push(AnimatedImage { page: self.tabs[idx].page, picture, loader, iter });
                    attach_zoom(picture, bytes, image.label.clone());
                    gtk_widget_set_visible(caption, 0);
                }
                Ok(None) => {
                    attach_zoom(picture, bytes, image.label.clone());
                    gtk_widget_set_visible(caption, 0);
                }
                Err(e) => set_label(caption, &format!("[image error: {e}]")),
            }
        } else {
            g_object_ref(picture as *mut c_void);
            g_object_ref(caption as *mut c_void);
            self.queued_images.push_back(QueuedImage { page: self.tabs[idx].page, picture, caption, url, label: image.label });
            if self.verbose {
                eprintln!("[queue:image] queued={} active={}", self.queued_images.len(), self.pending_images.len());
            }
            self.pump_image_queue();
        }
        (wrap, picture)
    }

    unsafe fn build_diagram(&mut self, idx: usize, block: usize, line: &md::Line) -> *mut GtkWidget {
        let frame = gtk_box_new(GTK_ORIENTATION_VERTICAL, 3);
        let Some(graph) = line.diagram.clone() else {
            return frame;
        };
        let source = line.code.as_ref().map(|b| b.text.clone()).unwrap_or_default();
        gtk_widget_add_css_class(frame, c("md-code-block").as_ptr());

        let head = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 6);
        gtk_widget_add_css_class(head, c("md-code-head").as_ptr());
        let diagram_title = match line.code.as_ref().map(|b| b.language.to_ascii_lowercase()).as_deref() {
            Some("dot") | Some("graphviz") | Some("gv") => "Graphviz/DOT graph",
            _ => "Mermaid flowchart",
        };
        let title = gtk_label_new(c(diagram_title).as_ptr());
        gtk_label_set_xalign(title as *mut GtkLabel, 0.0);
        gtk_widget_set_hexpand(title, 1);
        let copy = gtk_button_new_with_label(c("Copy source").as_ptr());
        let ctx = Box::new(CopyCodeCtx { app: self as *mut App, text: source.clone() });
        connect(copy, "clicked", on_copy_code as *const c_void, Box::into_raw(ctx) as *mut c_void, Some(drop_copy_code_ctx));
        gtk_box_append(head as *mut GtkBox, title);
        gtk_box_append(head as *mut GtkBox, copy);
        gtk_box_append(frame as *mut GtkBox, head);

        let zoom_out = gtk_button_new_with_label(c("−").as_ptr());
        let zoom_reset = gtk_button_new_with_label(c("100%").as_ptr());
        let zoom_in = gtk_button_new_with_label(c("+").as_ptr());
        let zoom_fit = gtk_button_new_with_label(c("Fit").as_ptr());
        gtk_box_append(head as *mut GtkBox, zoom_out);
        gtk_box_append(head as *mut GtkBox, zoom_reset);
        gtk_box_append(head as *mut GtkBox, zoom_in);
        gtk_box_append(head as *mut GtkBox, zoom_fit);

        let (natural_w, natural_h) = crate::graph::preferred_size(&graph);
        let area = gtk_drawing_area_new();
        let sc = gtk_scrolled_window_new();
        let initial_zoom = (1000.0 / natural_w.max(1) as f64).min(700.0 / natural_h.max(1) as f64).min(1.0);
        let graph_ctx = Box::new(GraphViewCtx { graph, area, scrolled: sc, natural_w, natural_h, zoom: initial_zoom });
        let graph_data = Box::into_raw(graph_ctx);
        apply_graph_zoom(&mut *graph_data, initial_zoom);
        connect(zoom_out, "clicked", graph_zoom_out as *const c_void, graph_data as *mut c_void, None);
        connect(zoom_reset, "clicked", graph_zoom_reset as *const c_void, graph_data as *mut c_void, None);
        connect(zoom_in, "clicked", graph_zoom_in as *const c_void, graph_data as *mut c_void, None);
        connect(zoom_fit, "clicked", graph_zoom_fit as *const c_void, graph_data as *mut c_void, None);
        gtk_drawing_area_set_draw_func(
            area as *mut GtkDrawingArea,
            Some(draw_graph),
            graph_data as *mut c_void,
            Some(drop_graph_view),
        );
        gtk_scrolled_window_set_policy(sc as *mut GtkScrolledWindow, GTK_POLICY_AUTOMATIC, GTK_POLICY_AUTOMATIC);
        gtk_scrolled_window_set_child(sc as *mut GtkScrolledWindow, area);
        gtk_box_append(frame as *mut GtkBox, sc);
        if line.indent > 0 {
            gtk_widget_set_margin_start(frame, (line.indent as i32) * INDENT_PX);
        }
        if let Some(span) = self.hit_span(idx, block) {
            self.mark_hit(idx, frame, span.first);
        }
        frame
    }

    unsafe fn build_code(&mut self, idx: usize, block: usize, line: &md::Line) -> *mut GtkWidget {
        let frame = gtk_box_new(GTK_ORIENTATION_VERTICAL, 3);
        let Some(code) = line.code.clone() else {
            return frame;
        };
        gtk_widget_add_css_class(frame, c("md-code-block").as_ptr());
        let head = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 6);
        gtk_widget_add_css_class(head, c("md-code-head").as_ptr());
        let lang = if code.language.is_empty() { "code".to_string() } else { code.language.clone() };
        let lang_label = gtk_label_new(c(&lang).as_ptr());
        if highlight::supported(&code.language) {
            gtk_widget_set_tooltip_text(lang_label, c("Built-in syntax highlighting enabled").as_ptr());
        }
        gtk_label_set_xalign(lang_label as *mut GtkLabel, 0.0);
        gtk_widget_set_hexpand(lang_label, 1);
        let copy = gtk_button_new_with_label(c("Copy").as_ptr());
        gtk_widget_set_tooltip_text(copy, c("Copy code block").as_ptr());
        let ctx = Box::new(CopyCodeCtx { app: self as *mut App, text: code.text.clone() });
        connect(copy, "clicked", on_copy_code as *const c_void, Box::into_raw(ctx) as *mut c_void, Some(drop_copy_code_ctx));
        gtk_box_append(head as *mut GtkBox, lang_label);
        gtk_box_append(head as *mut GtkBox, copy);
        gtk_box_append(frame as *mut GtkBox, head);

        let sc = gtk_scrolled_window_new();
        let code_lines = code.text.lines().count().max(1) as i32;
        // GtkScrolledWindow does not reliably grow to a GtkLabel's natural height
        // on every GTK4/theme combination. Size short code blocks explicitly so
        // documentation examples are visible without an inner vertical scrollbar.
        // Long blocks stay bounded and scroll vertically.
        const CODE_LINE_HEIGHT: i32 = 21;
        const CODE_VERTICAL_PADDING: i32 = 18;
        const CODE_MAX_HEIGHT: i32 = 640;
        let code_height = (code_lines * CODE_LINE_HEIGHT + CODE_VERTICAL_PADDING).min(CODE_MAX_HEIGHT);
        let long_code = code_lines * CODE_LINE_HEIGHT + CODE_VERTICAL_PADDING > CODE_MAX_HEIGHT;
        gtk_widget_set_size_request(sc, -1, code_height);
        gtk_scrolled_window_set_policy(
            sc as *mut GtkScrolledWindow,
            GTK_POLICY_AUTOMATIC,
            if long_code { GTK_POLICY_AUTOMATIC } else { GTK_POLICY_NEVER },
        );
        let label = gtk_label_new(ptr::null());
        gtk_label_set_xalign(label as *mut GtkLabel, 0.0);
        gtk_label_set_wrap(label as *mut GtkLabel, 0);
        gtk_label_set_selectable(label as *mut GtkLabel, 1);
        gtk_widget_add_css_class(label, c("md-code").as_ptr());
        let highlighted = highlight::render(&code.language, &code.text);
        gtk_label_set_markup(label as *mut GtkLabel, c(&format!("<tt>{}</tt>", highlighted)).as_ptr());
        gtk_scrolled_window_set_child(sc as *mut GtkScrolledWindow, label);
        gtk_box_append(frame as *mut GtkBox, sc);
        if line.indent > 0 {
            gtk_widget_set_margin_start(frame, (line.indent as i32) * INDENT_PX);
        }
        if let Some(span) = self.hit_span(idx, block) {
            self.mark_hit(idx, frame, span.first);
        }
        frame
    }

    unsafe fn build_text(&mut self, idx: usize, block: usize, line: &md::Line) -> *mut GtkWidget {
        let label = gtk_label_new(ptr::null());
        gtk_label_set_xalign(label as *mut GtkLabel, 0.0);
        gtk_label_set_wrap(label as *mut GtkLabel, 1);
        gtk_label_set_selectable(label as *mut GtkLabel, 1);
        let css = match line.style {
            Style::H1 => "md-h1",
            Style::H2 => "md-h2",
            Style::H3 => "md-h3",
            Style::H4 => "md-h4",
            Style::H5 => "md-h5",
            Style::H6 => "md-h6",
            Style::Bullet | Style::Ordered(_) => "md-bullet",
            Style::Task(_) => "md-task",
            Style::Quote => "md-quote",
            Style::TableRow => "md-table-row",
            Style::TableSep => "md-table-sep",
            Style::Math => "md-math",
            Style::Footnote => "md-footnote",
            _ => "md-text",
        };
        gtk_widget_add_css_class(label, c(css).as_ptr());
        let markup = match line.style {
            Style::Bullet => format!("•  {}", line.markup),
            Style::Ordered(n) => format!("{}.  {}", n, line.markup),
            Style::Task(done) => format!("{}  {}", if done { "☑" } else { "☐" }, line.markup),
            Style::Quote => format!("<i>›  {}</i>", line.markup),
            Style::TableSep => String::new(),
            Style::Math => format!("<span font_family=\"serif\" size=\"large\">{}</span>", line.markup),
            _ => line.markup.clone(),
        };
        gtk_label_set_markup(label as *mut GtkLabel, c(&markup).as_ptr());
        if let Some(tip) = &line.tooltip {
            gtk_widget_set_tooltip_text(label, c(tip).as_ptr());
        }
        if let Some(span) = self.hit_span(idx, block) {
            self.mark_hit(idx, label, span.first);
        }
        if line.indent > 0 {
            gtk_widget_set_margin_start(label, (line.indent as i32) * INDENT_PX);
        }
        connect(label, "activate-link", on_inline_link as *const c_void, self as *mut App as *mut c_void, None);
        label
    }
}

unsafe fn build_rule() -> *mut GtkWidget {
    let rule = gtk_label_new(c("────────────────────────────────────────────────────────").as_ptr());
    gtk_widget_add_css_class(rule, c("md-rule").as_ptr());
    gtk_label_set_xalign(rule as *mut GtkLabel, 0.0);
    rule
}

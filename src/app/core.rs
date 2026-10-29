use super::*;

impl App {
    pub(crate) unsafe fn new(widgets: AppWidgets, main_loop: *mut GMainLoop, verbose: bool, stats: bool) -> Self {
        Self {
            window: widgets.window,
            notebook: widgets.notebook,
            entry: widgets.entry,
            status: widgets.status,
            main_loop,
            tabs: Vec::new(),
            active: 0,
            pending: Vec::new(),
            pending_images: Vec::new(),
            queued_images: VecDeque::new(),
            animations: Vec::new(),
            cache: VecDeque::new(),
            image_cache: VecDeque::new(),
            image_cache_bytes: 0,
            sidebar_visible: true,
            bookmarks: bookmarks::load(),
            verbose,
            tree_bookmarks_open: true,
            tree_contents_open: true,
            tree_links_open: true,
            find_bar: widgets.find_bar,
            find_entry: widgets.find_entry,
            find_count: widgets.find_count,
            closed_tabs: Vec::new(),
            bookmark_window: ptr::null_mut(),
            bookmark_box: ptr::null_mut(),
            completion_seed: String::new(),
            completion_items: Vec::new(),
            completion_index: 0,
            g_pending: false,
            reader_zoom: 100,
            stats,
            dark: false,
            dark_chosen: None,
        }
    }
    pub(crate) unsafe fn add_tab(&mut self) {
        let paned = gtk_paned_new(GTK_ORIENTATION_HORIZONTAL);
        let tree_scroll = gtk_scrolled_window_new();
        gtk_widget_add_css_class(tree_scroll, c("sidebar").as_ptr());
        let tree_box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 4);
        gtk_widget_set_margin_start(tree_box, 8);
        gtk_widget_set_margin_end(tree_box, 8);
        gtk_widget_set_margin_top(tree_box, 8);
        gtk_widget_set_margin_bottom(tree_box, 8);
        // Long titles are ellipsized, so the sidebar never needs a horizontal scrollbar.
        gtk_scrolled_window_set_policy(tree_scroll as *mut GtkScrolledWindow, GTK_POLICY_NEVER, GTK_POLICY_AUTOMATIC);
        gtk_scrolled_window_set_child(tree_scroll as *mut GtkScrolledWindow, tree_box);
        gtk_widget_set_size_request(tree_scroll, 230, -1);

        let doc_scroll = gtk_scrolled_window_new();
        let (doc_view, doc_ctx) = docview::new_doc_view(self as *mut App, paned);
        gtk_widget_add_css_class(doc_view, c("document").as_ptr());
        gtk_widget_add_css_class(doc_view, c(&format!("zoom-{}", self.reader_zoom)).as_ptr());
        gtk_widget_set_margin_start(doc_view, 44);
        gtk_widget_set_margin_end(doc_view, 44);
        gtk_widget_set_hexpand(doc_scroll, 1);
        gtk_widget_set_vexpand(doc_scroll, 1);
        gtk_scrolled_window_set_child(doc_scroll as *mut GtkScrolledWindow, doc_view);

        gtk_paned_set_start_child(paned as *mut GtkPaned, tree_scroll);
        gtk_paned_set_end_child(paned as *mut GtkPaned, doc_scroll);
        gtk_paned_set_position(paned as *mut GtkPaned, 230);
        gtk_widget_set_visible(tree_scroll, if self.sidebar_visible { 1 } else { 0 });

        let tab_box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 4);
        gtk_widget_add_css_class(tab_box, c("tab-title").as_ptr());
        let tab_label = gtk_label_new(c("new").as_ptr());
        let tab_close = icon_button(ICON_CLOSE, "Close this tab");
        gtk_widget_add_css_class(tab_close, c("tab-close").as_ptr());
        gtk_box_append(tab_box as *mut GtkBox, tab_label);
        gtk_box_append(tab_box as *mut GtkBox, tab_close);
        let idx = gtk_notebook_append_page(self.notebook as *mut GtkNotebook, paned, tab_box);
        gtk_notebook_set_tab_reorderable(self.notebook as *mut GtkNotebook, paned, 1);
        let ctx = Box::new(TabCloseCtx { app: self as *mut App, page: paned });
        connect(
            tab_close,
            "clicked",
            on_tab_close_clicked as *const c_void,
            Box::into_raw(ctx) as *mut c_void,
            Some(drop_tab_close_ctx),
        );
        self.tabs.push(Tab {
            title: "new".into(),
            url: String::new(),
            back: vec![],
            forward: vec![],
            doc: Document::default(),
            status: "Type an HTTPS URL or local .md path and press Enter".into(),
            page: paned,
            tree_scroll,
            tree_box,
            tree_items: Vec::new(),
            doc_view,
            doc_ctx,
            doc_scroll,
            restore_scroll: None,
            tab_label,
            search: String::new(),
            search_index: 0,
            search_hits: 0,
            search_spans: Vec::new(),
        });
        self.active = idx.max(0) as usize;
        gtk_notebook_set_current_page(self.notebook as *mut GtkNotebook, idx);
        set_entry(self.entry, "");
        gtk_editable_select_region(self.entry as *mut GtkEditable, 0, -1);
        gtk_widget_grab_focus(self.entry);
        self.refresh_status();
    }
    pub(crate) unsafe fn save_session(&self) {
        let tabs = self
            .tabs
            .iter()
            .enumerate()
            .filter_map(|(i, t)| {
                if t.url.is_empty() {
                    None
                } else {
                    Some(session::SessionTab { url: t.url.clone(), position: self.reading_position(i) })
                }
            })
            .collect();
        let state = session::Session {
            width: gtk_widget_get_width(self.window),
            height: gtk_widget_get_height(self.window),
            active: self.active,
            sidebar_visible: self.sidebar_visible,
            tree_bookmarks_open: self.tree_bookmarks_open,
            tree_contents_open: self.tree_contents_open,
            tree_links_open: self.tree_links_open,
            tabs,
            dark: self.dark_chosen,
        };
        if let Err(e) = session::save(&state) {
            if self.verbose {
                eprintln!("[session:save-error] {e}");
            }
        } else if self.verbose {
            eprintln!("[session:save] {} tab(s)", state.tabs.len());
        }
    }
    pub(crate) unsafe fn sync_active(&mut self) {
        if self.active >= self.tabs.len() {
            return;
        }
        set_entry(self.entry, &md::display_url(&self.tabs[self.active].url));
        let title = format!("{} - SilkMark v{}", self.tabs[self.active].title, env!("CARGO_PKG_VERSION"));
        gtk_window_set_title(self.window as *mut GtkWindow, c(&title).as_ptr());
        set_entry(self.find_entry, &self.tabs[self.active].search);
        self.update_find_count();
        self.refresh_status();
    }
    pub(crate) unsafe fn apply_reader_zoom(&self) {
        const LEVELS: [i32; 9] = [80, 90, 100, 110, 120, 130, 140, 150, 160];
        for tab in &self.tabs {
            for z in LEVELS {
                gtk_widget_remove_css_class(tab.doc_view, c(&format!("zoom-{z}")).as_ptr());
            }
            gtk_widget_add_css_class(tab.doc_view, c(&format!("zoom-{}", self.reader_zoom)).as_ptr());
        }
    }
    pub(crate) unsafe fn change_reader_zoom(&mut self, delta: i32) {
        let next = if delta == 0 { 100 } else { (self.reader_zoom + delta).clamp(80, 160) };
        if next == self.reader_zoom {
            return;
        }
        self.reader_zoom = next;
        self.apply_reader_zoom();
        if self.active < self.tabs.len() {
            self.tabs[self.active].status = format!("Reader zoom {}%", self.reader_zoom);
        }
        self.refresh_status();
    }
    pub(crate) unsafe fn refresh_status(&self) {
        if self.active >= self.tabs.len() {
            return;
        }
        let s = format!(
            "{}   |   zoom {}%   |   pages {}/{}   |   images {}/{}   |   bookmarks {}",
            self.tabs[self.active].status,
            self.reader_zoom,
            self.cache.len(),
            CACHE_PAGES,
            self.image_cache.len(),
            CACHE_IMAGES,
            self.bookmarks.len()
        );
        set_label(self.status, &s);
    }
    pub(crate) unsafe fn toggle_dark(&mut self) {
        self.dark = !self.dark;
        self.dark_chosen = Some(self.dark);
        crate::theme::apply(self.dark);
        self.save_session();
    }
    pub(crate) unsafe fn toggle_sidebar(&mut self) {
        self.sidebar_visible = !self.sidebar_visible;
        for t in &self.tabs {
            gtk_widget_set_visible(t.tree_scroll, if self.sidebar_visible { 1 } else { 0 });
        }
    }
}

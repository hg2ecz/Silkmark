use super::*;

impl App {
    pub(crate) unsafe fn switch_tab_relative(&mut self, delta: isize) {
        if self.tabs.is_empty() {
            return;
        }
        let n = self.tabs.len() as isize;
        let next = (self.active as isize + delta).rem_euclid(n) as usize;
        gtk_notebook_set_current_page(self.notebook as *mut GtkNotebook, next as i32);
        self.active = next;
        self.sync_active();
    }
    pub(crate) unsafe fn switch_tab_number(&mut self, n: usize) {
        if n == 0 || n > self.tabs.len() {
            return;
        }
        let idx = n - 1;
        gtk_notebook_set_current_page(self.notebook as *mut GtkNotebook, idx as i32);
        self.active = idx;
        self.sync_active();
    }
    pub(crate) unsafe fn scroll_document_pages(&mut self, pages: f64) {
        self.scroll_document_by(|page_size| pages * page_size);
    }
    pub(crate) unsafe fn scroll_document_pixels(&mut self, pixels: f64) {
        self.scroll_document_by(|_| pixels);
    }
    unsafe fn scroll_document_by(&mut self, delta: impl Fn(f64) -> f64) {
        if self.active >= self.tabs.len() {
            return;
        }
        let adj = gtk_scrolled_window_get_vadjustment(self.tabs[self.active].doc_scroll as *mut GtkScrolledWindow);
        if adj.is_null() {
            return;
        }
        let max = (gtk_adjustment_get_upper(adj) - gtk_adjustment_get_page_size(adj)).max(0.0);
        let next = gtk_adjustment_get_value(adj) + delta(gtk_adjustment_get_page_size(adj));
        gtk_adjustment_set_value(adj, next.clamp(0.0, max));
    }
    pub(crate) unsafe fn scroll_document_edge(&mut self, bottom: bool) {
        if self.active >= self.tabs.len() {
            return;
        }
        let adj = gtk_scrolled_window_get_vadjustment(self.tabs[self.active].doc_scroll as *mut GtkScrolledWindow);
        if adj.is_null() {
            return;
        }
        if bottom {
            let upper = gtk_adjustment_get_upper(adj);
            let page = gtk_adjustment_get_page_size(adj);
            gtk_adjustment_set_value(adj, (upper - page).max(0.0));
        } else {
            gtk_adjustment_set_value(adj, 0.0);
        }
    }
    pub(crate) unsafe fn close_tab(&mut self) {
        if self.tabs.len() <= 1 {
            let page = self.tabs[0].page;
            self.clear_animations_for_page(page);
            let t = &mut self.tabs[0];
            t.title = "new".into();
            t.url = String::new();
            t.back.clear();
            t.forward.clear();
            t.doc = Document::default();
            t.status = "Type an HTTPS URL or local .md path and press Enter".into();
            self.render_tab(0);
            self.active = 0;
            set_entry(self.entry, "");
            return;
        }
        let page = self.tabs[self.active].page;
        let closed_url = self.tabs[self.active].url.clone();
        if !closed_url.is_empty() {
            self.closed_tabs.push(closed_url);
            if self.closed_tabs.len() > 16 {
                self.closed_tabs.remove(0);
            }
        }
        self.clear_animations_for_page(page);
        gtk_notebook_remove_page(self.notebook as *mut GtkNotebook, self.active as i32);
        self.tabs.remove(self.active);
        self.active = self.active.min(self.tabs.len() - 1);
        gtk_notebook_set_current_page(self.notebook as *mut GtkNotebook, self.active as i32);
        self.sync_active();
    }
    pub(crate) unsafe fn close_page(&mut self, page: *mut GtkWidget) {
        if let Some(idx) = self.tabs.iter().position(|t| t.page == page) {
            self.clear_animations_for_page(page);
            self.active = idx;
            self.close_tab();
        }
    }
    pub(crate) unsafe fn update_find_count(&self) {
        if self.active >= self.tabs.len() {
            return;
        }
        let t = &self.tabs[self.active];
        let text = if t.search.is_empty() {
            String::new()
        } else if t.search_hits == 0 {
            "0 matches".into()
        } else {
            format!("{} / {}", t.search_index + 1, t.search_hits)
        };
        set_label(self.find_count, &text);
    }
    pub(crate) unsafe fn show_find(&mut self) {
        if self.active >= self.tabs.len() {
            return;
        }
        gtk_widget_set_visible(self.find_bar, 1);
        set_entry(self.find_entry, &self.tabs[self.active].search);
        gtk_widget_grab_focus(self.find_entry);
        gtk_editable_select_region(self.find_entry as *mut GtkEditable, 0, -1);
        self.update_find_count();
    }
    pub(crate) unsafe fn hide_find(&mut self) {
        gtk_widget_set_visible(self.find_bar, 0);
    }
    pub(crate) unsafe fn find_changed(&mut self) {
        if self.active >= self.tabs.len() {
            return;
        }
        let p = gtk_editable_get_text(self.find_entry as *mut GtkEditable);
        if p.is_null() {
            return;
        }
        let q = CStr::from_ptr(p).to_string_lossy().into_owned();
        let tab = self.active;
        self.tabs[tab].search = q;
        self.tabs[tab].search_index = 0;
        self.recompute_search(tab);
        self.refresh_rows(tab);
        self.scroll_to_target(tab);
        self.update_find_count();
    }
    pub(crate) unsafe fn find_step(&mut self, forward: bool) {
        if self.active >= self.tabs.len() {
            return;
        }
        let hits = self.tabs[self.active].search_hits;
        if hits == 0 {
            self.update_find_count();
            return;
        }
        if forward {
            self.tabs[self.active].search_index = (self.tabs[self.active].search_index + 1) % hits;
        } else {
            self.tabs[self.active].search_index = (self.tabs[self.active].search_index + hits - 1) % hits;
        }
        let tab = self.active;
        self.refresh_rows(tab);
        self.scroll_to_target(tab);
        self.update_find_count();
    }
    pub(crate) unsafe fn copy_text(&mut self, text: &str) {
        let display = gdk_display_get_default();
        if display.is_null() {
            self.tabs[self.active].status = "Clipboard unavailable".into();
            self.refresh_status();
            return;
        }
        let clipboard = gdk_display_get_clipboard(display);
        if clipboard.is_null() {
            self.tabs[self.active].status = "Clipboard unavailable".into();
            self.refresh_status();
            return;
        }
        let z = c(text);
        gdk_clipboard_set_text(clipboard, z.as_ptr());
    }
    pub(crate) unsafe fn copy_current_section(&mut self) {
        if self.active >= self.tabs.len() || self.tabs[self.active].url.is_empty() {
            return;
        }
        let url = md::canonicalize_url_fragment(&self.tabs[self.active].url);
        self.copy_text(&url);
        self.tabs[self.active].status = if url.contains('#') {
            "Section link copied".into()
        } else {
            "Document link copied (no active section fragment)".into()
        };
        self.refresh_status();
        if self.verbose {
            eprintln!("[copy:link] {url}");
        }
    }
    pub(crate) unsafe fn complete_address(&mut self) {
        if self.active >= self.tabs.len() {
            return;
        }
        let p = gtk_editable_get_text(self.entry as *mut GtkEditable);
        if p.is_null() {
            return;
        }
        let input = CStr::from_ptr(p).to_string_lossy().into_owned();
        if input != self.completion_seed || self.completion_items.is_empty() {
            self.completion_seed = input.clone();
            self.completion_items.clear();
            self.completion_index = 0;
            if let Some(hash) = input.rfind('#') {
                let typed = md::fragment_id(&format!("x#{}", &input[hash + 1..])).unwrap_or_default();
                let base = if hash == 0 { md::document_url(&self.tabs[self.active].url) } else { input[..hash].to_string() };
                for line in &self.tabs[self.active].doc.lines {
                    if let Some(anchor) = &line.anchor {
                        if typed.is_empty() || anchor.starts_with(&typed) || anchor.contains(&typed) {
                            self.completion_items.push(format!("{}#{}", base, md::percent_encode_fragment(anchor)));
                        }
                    }
                }
            } else {
                let mut sources = Vec::<String>::new();
                sources.extend(self.bookmarks.iter().map(|b| b.url.clone()));
                for t in &self.tabs {
                    if !t.url.is_empty() {
                        sources.push(t.url.clone());
                    }
                    sources.extend(t.back.iter().rev().cloned());
                    sources.extend(t.forward.iter().rev().cloned());
                }
                for u in sources {
                    if !u.is_empty()
                        && (input.is_empty() || u.starts_with(&input))
                        && !self.completion_items.iter().any(|x| x == &u)
                    {
                        self.completion_items.push(u);
                        if self.completion_items.len() >= 32 {
                            break;
                        }
                    }
                }
            }
        } else if !self.completion_items.is_empty() {
            self.completion_index = (self.completion_index + 1) % self.completion_items.len();
        }
        if let Some(v) = self.completion_items.get(self.completion_index).cloned() {
            set_entry(self.entry, &v);
            gtk_editable_select_region(self.entry as *mut GtkEditable, -1, -1);
            self.tabs[self.active].status =
                format!("Completion {}/{}  (Ctrl+Space cycles)", self.completion_index + 1, self.completion_items.len());
            self.refresh_status();
        }
    }
    pub(crate) unsafe fn reopen_closed_tab(&mut self) {
        if let Some(url) = self.closed_tabs.pop() {
            self.add_tab();
            let i = self.active;
            self.navigate(i, url, false, true);
        }
    }
    pub(crate) unsafe fn sync_tab_order_from_page(&mut self, page: *mut GtkWidget, new_pos: usize) {
        let Some(old_pos) = self.tabs.iter().position(|t| t.page == page) else {
            return;
        };
        if old_pos == new_pos || new_pos >= self.tabs.len() {
            self.active = new_pos.min(self.tabs.len().saturating_sub(1));
            return;
        }
        let tab = self.tabs.remove(old_pos);
        self.tabs.insert(new_pos, tab);
        self.active = new_pos;
        self.sync_active();
    }
    pub(crate) unsafe fn navigate(&mut self, tab: usize, url: String, push: bool, use_cache: bool) {
        if tab >= self.tabs.len() {
            return;
        }
        let Some(url) = net::normalize_location(url.trim()) else {
            self.tabs[tab].status = "Use an allowed HTTP(S) URL, file:// URL, or local Markdown path".into();
            self.refresh_status();
            return;
        };
        let url = md::canonicalize_url_fragment(&url);
        if self.verbose {
            eprintln!("[navigate] {url}");
        }
        if push {
            let old = self.tabs[tab].url.clone();
            if !old.is_empty() && old != url {
                self.tabs[tab].back.push(old);
            }
            self.tabs[tab].forward.clear();
        }

        // A fragment-only move inside the already loaded document needs no HTTP request.
        if md::document_url(&self.tabs[tab].url) == md::document_url(&url) && !self.tabs[tab].doc.lines.is_empty() {
            self.tabs[tab].url = url.clone();
            self.tabs[tab].status = match md::fragment_id(&url) {
                Some(id) => format!("Section #{id}"),
                None => "Document top".into(),
            };
            self.scroll_to_target(tab);
            self.update_tree_current(tab);
            if tab == self.active {
                self.sync_active();
            }
            return;
        }

        if use_cache && self.cache_apply(tab, &url) {
            if self.verbose {
                eprintln!("[cache:document] {url}");
            }
            return;
        }
        self.tabs[tab].url = url.clone();
        self.tabs[tab].status = "Loading...".into();
        if tab == self.active {
            set_entry(self.entry, &md::display_url(&url));
            self.refresh_status();
        }
        let (tx, rx) = mpsc::channel();
        let requested = url.clone();
        let fetch_url = md::document_url(&url);
        thread::spawn(move || {
            // Parsing is pure Rust, so it runs here and the UI thread only receives the finished model.
            let result = net::fetch(&fetch_url).map(|(final_url, src)| {
                let parse_started = Instant::now();
                let doc = md::parse(&src);
                Fetched { final_url, doc, source_bytes: src.len(), parse_elapsed: parse_started.elapsed() }
            });
            let _ = tx.send(result);
        });
        self.pending.push(Pending { tab, requested, rx });
    }
    pub(crate) unsafe fn back(&mut self) {
        let tab = self.active;
        if tab >= self.tabs.len() {
            return;
        }
        if let Some(u) = self.tabs[tab].back.pop() {
            let cur = self.tabs[tab].url.clone();
            self.tabs[tab].forward.push(cur);
            self.navigate(tab, u, false, true);
        }
    }
    pub(crate) unsafe fn forward(&mut self) {
        let tab = self.active;
        if tab >= self.tabs.len() {
            return;
        }
        if let Some(u) = self.tabs[tab].forward.pop() {
            let cur = self.tabs[tab].url.clone();
            self.tabs[tab].back.push(cur);
            self.navigate(tab, u, false, true);
        }
    }
    pub(crate) unsafe fn reload(&mut self) {
        let tab = self.active;
        if tab < self.tabs.len() {
            let u = self.tabs[tab].url.clone();
            self.navigate(tab, u, false, false);
        }
    }
    pub(crate) unsafe fn open_link(&mut self, target: String) {
        let tab = self.active;
        if tab >= self.tabs.len() {
            return;
        }
        if let Some(u) = net::resolve_resource(&self.tabs[tab].url, &target, true) {
            self.navigate(tab, md::canonicalize_url_fragment(&u), true, true);
        } else {
            self.tabs[tab].status = format!("Unsupported or invalid link: {target}");
            self.refresh_status();
        }
    }
}

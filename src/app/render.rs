use super::*;

impl App {
    pub(crate) unsafe fn render_tab(&mut self, idx: usize) {
        if idx >= self.tabs.len() {
            return;
        }
        let render_started = Instant::now();
        let page = self.tabs[idx].page;
        self.clear_animations_for_page(page);
        set_label(self.tabs[idx].tab_label, &short(&self.tabs[idx].title, 24));
        gtk_widget_set_tooltip_text(self.tabs[idx].tab_label, c(&self.tabs[idx].title).as_ptr());

        // A link/button inside the current document can still own keyboard focus
        // when navigation completes. Removing that focused subtree immediately can
        // leave GTK with a stale focus-chain pointer until its next main-loop
        // update. Clear window focus first, but only when the focused widget is
        // actually inside this document subtree.
        let focused = gtk_window_get_focus(self.window as *mut GtkWindow);
        if !focused.is_null() {
            let mut node = focused;
            let mut inside_document = false;
            while !node.is_null() {
                if node == self.tabs[idx].doc_view {
                    inside_document = true;
                    break;
                }
                node = gtk_widget_get_parent(node);
            }
            if inside_document {
                gtk_window_set_focus(self.window as *mut GtkWindow, ptr::null_mut());
            }
        }

        self.show_document(idx);
        self.render_tree(idx);
        if self.stats {
            eprintln!(
                "[stats:render] tab={} blocks={} queued_images={} active_images={} elapsed_ms={:.3}",
                idx + 1,
                self.tabs[idx].doc.index.blocks.len(),
                self.queued_images.len(),
                self.pending_images.len(),
                render_started.elapsed().as_secs_f64() * 1000.0
            );
        }
    }
    /// A sidebar button: left-aligned, long titles wrap onto further lines instead of being cut.
    unsafe fn tree_button(&self, text: &str, tooltip: Option<&str>, class: &str, indent: i32) -> *mut GtkWidget {
        let btn = gtk_button_new_with_label(c(text).as_ptr());
        for name in class.split_whitespace() {
            gtk_widget_add_css_class(btn, c(name).as_ptr());
        }
        let label = gtk_button_get_child(btn as *mut GtkButton);
        if !label.is_null() {
            gtk_label_set_xalign(label as *mut GtkLabel, 0.0);
            gtk_label_set_wrap(label as *mut GtkLabel, 1);
            gtk_label_set_wrap_mode(label as *mut GtkLabel, PANGO_WRAP_WORD_CHAR);
            gtk_widget_set_hexpand(label, 1);
            gtk_widget_set_halign(label, GTK_ALIGN_START);
        }
        gtk_widget_set_tooltip_text(btn, c(tooltip.unwrap_or(text)).as_ptr());
        if indent > 0 {
            gtk_widget_set_margin_start(btn, indent);
        }
        btn
    }

    unsafe fn tree_heading(&mut self, idx: usize, text: &str, section: TreeSection, top_margin: i32) {
        let btn = self.tree_button(text, None, "tree-heading-button", 0);
        gtk_widget_set_margin_top(btn, top_margin);
        let ctx = Box::new(TreeToggleCtx { app: self as *mut App, section });
        connect(btn, "clicked", on_tree_toggle as *const c_void, Box::into_raw(ctx) as *mut c_void, Some(drop_tree_toggle_ctx));
        gtk_box_append(self.tabs[idx].tree_box as *mut GtkBox, btn);
    }

    unsafe fn tree_note(&self, idx: usize, text: &str) {
        let note = gtk_label_new(c(text).as_ptr());
        gtk_label_set_xalign(note as *mut GtkLabel, 0.0);
        gtk_widget_add_css_class(note, c("tree-empty").as_ptr());
        gtk_box_append(self.tabs[idx].tree_box as *mut GtkBox, note);
    }

    unsafe fn tree_link_button(
        &mut self,
        idx: usize,
        text: &str,
        tooltip: Option<&str>,
        class: &str,
        indent: i32,
        target: String,
    ) -> *mut GtkWidget {
        let btn = self.tree_button(text, tooltip, class, indent);
        let ctx = Box::new(LinkCtx { app: self as *mut App, target });
        connect(btn, "clicked", on_link_clicked as *const c_void, Box::into_raw(ctx) as *mut c_void, Some(drop_link_ctx));
        gtk_box_append(self.tabs[idx].tree_box as *mut GtkBox, btn);
        btn
    }

    /// Marks the contents entry of the current section. Cheap: no sidebar rebuild.
    pub(crate) unsafe fn update_tree_current(&mut self, idx: usize) {
        let current = md::fragment_id(&self.tabs[idx].url);
        for (anchor, btn) in &self.tabs[idx].tree_items {
            if current.as_deref() == Some(anchor.as_str()) {
                gtk_widget_add_css_class(*btn, c("tree-current").as_ptr());
            } else {
                gtk_widget_remove_css_class(*btn, c("tree-current").as_ptr());
            }
        }
    }

    pub(crate) unsafe fn render_tree(&mut self, idx: usize) {
        if idx >= self.tabs.len() {
            return;
        }
        // Rebuilding the sidebar would otherwise throw the user back to its top.
        let adj = gtk_scrolled_window_get_vadjustment(self.tabs[idx].tree_scroll as *mut GtkScrolledWindow);
        let saved_scroll = if adj.is_null() { 0.0 } else { gtk_adjustment_get_value(adj) };
        loop {
            let child = gtk_widget_get_first_child(self.tabs[idx].tree_box);
            if child.is_null() {
                break;
            }
            gtk_box_remove(self.tabs[idx].tree_box as *mut GtkBox, child);
        }
        self.tabs[idx].tree_items.clear();

        let arrow = |open: bool| if open { "▾  " } else { "▸  " };
        self.tree_heading(idx, &format!("{}Bookmarks", arrow(self.tree_bookmarks_open)), TreeSection::Bookmarks, 0);
        if self.tree_bookmarks_open && self.bookmarks.is_empty() {
            self.tree_note(idx, "Ctrl+D adds the current page");
        } else if self.tree_bookmarks_open {
            for item in self.bookmarks.clone().into_iter().take(100) {
                let label = md::percent_decode_fragment(if item.title.is_empty() { &item.url } else { &item.title });
                self.tree_link_button(idx, &format!("★  {label}"), Some(&item.url), "tree-link", 0, item.url.clone());
            }
        }

        self.tree_heading(idx, &format!("{}Contents", arrow(self.tree_contents_open)), TreeSection::Contents, 12);
        let mut toc_count = 0usize;
        if self.tree_contents_open {
            for line in self.tabs[idx].doc.lines.clone() {
                let level = match line.style {
                    Style::H1 => 0,
                    Style::H2 => 1,
                    Style::H3 => 2,
                    Style::H4 => 3,
                    Style::H5 => 4,
                    Style::H6 => 5,
                    _ => continue,
                };
                let Some(anchor) = line.anchor else {
                    continue;
                };
                let title = strip_pango_markup(&line.markup).replace('¶', "").trim().to_string();
                let class = if level <= 1 { "tree-link tree-top" } else { "tree-link" };
                let target = format!("#{}", md::percent_encode_fragment(&anchor));
                let btn = self.tree_link_button(idx, &title, None, class, level * TREE_INDENT_PX, target);
                self.tabs[idx].tree_items.push((anchor, btn));
                toc_count += 1;
                if toc_count >= TREE_MAX_HEADINGS {
                    break;
                }
            }
        }
        if self.tree_contents_open && toc_count == 0 {
            self.tree_note(idx, "No headings");
        }

        self.tree_heading(idx, &format!("{}Markdown links", arrow(self.tree_links_open)), TreeSection::Links, 12);
        if self.tree_links_open {
            let links = self.tabs[idx].doc.links.clone();
            if links.is_empty() {
                self.tree_note(idx, "No links");
            }
            for link in links.into_iter().take(200) {
                self.tree_link_button(idx, &link.label, Some(&link.target), "tree-link", 0, link.target.clone());
            }
        }
        self.update_tree_current(idx);

        if saved_scroll > 0.0 {
            let ctx = Box::new(TreeScrollCtx { app: self as *mut App, page: self.tabs[idx].page, value: saved_scroll });
            g_timeout_add(TREE_SCROLL_RESTORE_MS, restore_tree_scroll, Box::into_raw(ctx) as *mut c_void);
        }
    }
}

const TREE_INDENT_PX: i32 = 14;
const TREE_MAX_HEADINGS: usize = 1000;
const TREE_SCROLL_RESTORE_MS: guint = 30;

struct TreeScrollCtx {
    app: *mut App,
    page: *mut GtkWidget,
    value: f64,
}

/// The new sidebar content is laid out a moment after the rebuild; only then can the old offset apply.
unsafe extern "C" fn restore_tree_scroll(data: *mut c_void) -> gboolean {
    let ctx = Box::from_raw(data as *mut TreeScrollCtx);
    let app = &*ctx.app;
    if let Some(tab) = app.tabs.iter().find(|t| t.page == ctx.page) {
        let adj = gtk_scrolled_window_get_vadjustment(tab.tree_scroll as *mut GtkScrolledWindow);
        if !adj.is_null() {
            gtk_adjustment_set_value(adj, ctx.value);
        }
    }
    0
}

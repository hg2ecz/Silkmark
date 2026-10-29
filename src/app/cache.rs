use super::*;

impl App {
    pub(crate) fn cache_put(&mut self, url: String, title: String, doc: Document) {
        let key = md::document_url(&url);
        self.cache.retain(|e| e.url != key);
        self.cache.push_front(CacheEntry { url: key, title, doc });
        while self.cache.len() > CACHE_PAGES {
            self.cache.pop_back();
        }
    }
    pub(crate) unsafe fn cache_apply(&mut self, tab: usize, url: &str) -> bool {
        let key = md::document_url(url);
        let Some(pos) = self.cache.iter().position(|e| e.url == key) else {
            return false;
        };
        let Some(e) = self.cache.remove(pos) else {
            return false;
        };
        if tab >= self.tabs.len() {
            return false;
        }
        self.tabs[tab].url = md::with_fragment_from(&e.url, url);
        self.tabs[tab].title = e.title.clone();
        self.tabs[tab].doc = e.doc.clone();
        self.tabs[tab].status = "Loaded from 8-page cache".into();
        self.cache.push_front(e);
        self.render_tab(tab);
        if tab == self.active {
            self.sync_active();
            self.focus_document_if_idle(tab);
        }
        true
    }
    pub(crate) fn image_cache_get(&mut self, url: &str) -> Option<Vec<u8>> {
        let pos = self.image_cache.iter().position(|e| e.url == url)?;
        let e = self.image_cache.remove(pos)?;
        let bytes = e.bytes.clone();
        self.image_cache.push_front(e);
        Some(bytes)
    }
    pub(crate) fn image_cache_put(&mut self, url: String, bytes: Vec<u8>) {
        if let Some(pos) = self.image_cache.iter().position(|e| e.url == url) {
            if let Some(old) = self.image_cache.remove(pos) {
                self.image_cache_bytes = self.image_cache_bytes.saturating_sub(old.bytes.len());
            }
        }
        self.image_cache_bytes = self.image_cache_bytes.saturating_add(bytes.len());
        self.image_cache.push_front(ImageCacheEntry { url, bytes });
        while self.image_cache.len() > CACHE_IMAGES || self.image_cache_bytes > MAX_IMAGE_CACHE_BYTES {
            if let Some(old) = self.image_cache.pop_back() {
                self.image_cache_bytes = self.image_cache_bytes.saturating_sub(old.bytes.len());
            } else {
                break;
            }
        }
    }
    pub(crate) unsafe fn clear_animations_for_page(&mut self, page: *mut GtkWidget) {
        // A render can replace the whole widget subtree while image downloads are
        // still in flight. Drop our strong references to those stale widgets now;
        // the worker thread only owns the channel sender, so its eventual send will
        // simply fail after the receiver is dropped. This prevents late GTK calls on
        // widgets that no longer belong to the current document tree.
        let mut p = 0;
        while p < self.pending_images.len() {
            if self.pending_images[p].page == page {
                let image = self.pending_images.remove(p);
                g_object_unref(image.picture as *mut c_void);
                g_object_unref(image.caption as *mut c_void);
            } else {
                p += 1;
            }
        }

        let mut q = 0;
        while q < self.queued_images.len() {
            if self.queued_images[q].page == page {
                if let Some(img) = self.queued_images.remove(q) {
                    g_object_unref(img.picture as *mut c_void);
                    g_object_unref(img.caption as *mut c_void);
                }
            } else {
                q += 1;
            }
        }
        let mut i = 0;
        while i < self.animations.len() {
            if self.animations[i].page == page {
                let a = self.animations.remove(i);
                g_object_unref(a.picture as *mut c_void);
                g_object_unref(a.iter as *mut c_void);
                g_object_unref(a.loader as *mut c_void);
            } else {
                i += 1;
            }
        }
    }
    pub(crate) unsafe fn poll(&mut self) {
        let mut i = 0;
        while i < self.pending.len() {
            match self.pending[i].rx.try_recv() {
                Ok(result) => {
                    let p = self.pending.remove(i);
                    if p.tab >= self.tabs.len() || self.tabs[p.tab].url != p.requested {
                        continue;
                    }
                    match result {
                        Ok(Fetched { final_url, doc, source_bytes, parse_elapsed }) => {
                            if self.stats {
                                let heading_count = doc
                                    .lines
                                    .iter()
                                    .filter(|l| {
                                        matches!(l.style, Style::H1 | Style::H2 | Style::H3 | Style::H4 | Style::H5 | Style::H6)
                                    })
                                    .count();
                                eprintln!(
                                    "[stats:parse] bytes={} blocks={} links={} headings={} elapsed_ms={:.3}",
                                    source_bytes,
                                    doc.lines.len(),
                                    doc.links.len(),
                                    heading_count,
                                    parse_elapsed.as_secs_f64() * 1000.0
                                );
                            }
                            let final_with_fragment = md::with_fragment_from(&final_url, &p.requested);
                            let title = document_title(&doc, &final_url);
                            self.tabs[p.tab].url = final_with_fragment;
                            self.tabs[p.tab].title = title.clone();
                            self.tabs[p.tab].doc = doc.clone();
                            self.tabs[p.tab].status = format!("{} lines, {} links", doc.lines.len(), doc.links.len());
                            self.cache_put(final_url, title, doc);
                            self.render_tab(p.tab);
                            if p.tab == self.active {
                                self.sync_active();
                                self.focus_document_if_idle(p.tab);
                            }
                        }
                        Err(e) => {
                            self.tabs[p.tab].status = e;
                            if p.tab == self.active {
                                self.refresh_status();
                            }
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => i += 1,
                Err(_) => {
                    self.pending.remove(i);
                }
            }
        }

        let mut j = 0;
        while j < self.pending_images.len() {
            match self.pending_images[j].rx.try_recv() {
                Ok(result) => {
                    let p = self.pending_images.remove(j);
                    // A document may have been re-rendered while the worker was
                    // loading the image. Strong refs keep these objects alive, but
                    // detached widgets must not receive GTK operations.
                    if gtk_widget_get_root(p.picture).is_null() || gtk_widget_get_root(p.caption).is_null() {
                        g_object_unref(p.picture as *mut c_void);
                        g_object_unref(p.caption as *mut c_void);
                        continue;
                    }
                    match result {
                        Ok(bytes) => {
                            self.image_cache_put(p.url.clone(), bytes.clone());
                            match set_picture_bytes(p.picture, &bytes) {
                                Ok(Some((loader, iter))) => {
                                    g_object_ref(p.picture as *mut c_void);
                                    self.animations.push(AnimatedImage { page: p.page, picture: p.picture, loader, iter });
                                    attach_zoom(p.picture, bytes, p.label);
                                    gtk_widget_set_visible(p.caption, 0);
                                }
                                Ok(None) => {
                                    attach_zoom(p.picture, bytes, p.label);
                                    gtk_widget_set_visible(p.caption, 0);
                                }
                                Err(e) => {
                                    set_label(p.caption, &format!("[image error: {e}]"));
                                    gtk_widget_set_visible(p.caption, 1);
                                }
                            }
                        }
                        Err(e) => {
                            set_label(p.caption, &format!("[image error: {e}]"));
                            gtk_widget_set_visible(p.caption, 1);
                        }
                    }
                    g_object_unref(p.picture as *mut c_void);
                    g_object_unref(p.caption as *mut c_void);
                }
                Err(mpsc::TryRecvError::Empty) => j += 1,
                Err(_) => {
                    let p = self.pending_images.remove(j);
                    g_object_unref(p.picture as *mut c_void);
                    g_object_unref(p.caption as *mut c_void);
                }
            }
        }

        self.pump_image_queue();

        let now = current_timeval();
        for a in &self.animations {
            if gtk_widget_get_root(a.picture).is_null() {
                continue;
            }
            if gdk_pixbuf_animation_iter_advance(a.iter, &now) != 0 {
                let pix = gdk_pixbuf_animation_iter_get_pixbuf(a.iter);
                if !pix.is_null() {
                    let _ = set_scaled_pixbuf(a.picture, pix);
                }
            }
        }
    }
    pub(crate) unsafe fn pump_image_queue(&mut self) {
        while self.pending_images.len() < MAX_CONCURRENT_IMAGE_FETCHES {
            let Some(q) = self.queued_images.pop_front() else {
                break;
            };
            let (tx, rx) = mpsc::channel();
            let fetch_url = q.url.clone();
            if self.verbose {
                eprintln!(
                    "[queue:image-start] {} active={} queued={}",
                    fetch_url,
                    self.pending_images.len() + 1,
                    self.queued_images.len()
                );
            }
            thread::spawn(move || {
                let _ = tx.send(net::fetch_image(&fetch_url));
            });
            self.pending_images.push(PendingImage {
                page: q.page,
                picture: q.picture,
                caption: q.caption,
                url: q.url,
                label: q.label,
                rx,
            });
        }
    }
}

/// Tab title: the document's first heading, else its decoded file name.
fn document_title(doc: &Document, url: &str) -> String {
    let heading = doc
        .lines
        .iter()
        .find(|l| matches!(l.style, Style::H1))
        .map(|l| strip_pango_markup(&l.markup).replace('¶', "").trim().to_string())
        .filter(|t| !t.is_empty());
    heading.unwrap_or_else(|| {
        let name = md::document_url(url).rsplit('/').next().map(md::percent_decode_fragment).unwrap_or_default();
        if name.is_empty() { "index.md".to_string() } else { name }
    })
}

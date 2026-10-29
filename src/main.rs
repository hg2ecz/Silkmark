#![allow(unsafe_op_in_unsafe_fn)]
// Direct GTK/libcurl FFI: unsafe operations are intentionally grouped inside unsafe boundary functions.

mod app;
mod bookmarks;
mod ffi;
mod graph;
mod graphviz;
mod highlight;
mod math;
mod md;
mod mermaid;
mod net;
mod session;
mod storage;
mod theme;

use ffi::*;
use std::ffi::{CStr, CString, c_void};
use std::ptr;

use app::*;

fn c(s: &str) -> CString {
    CString::new(s.replace('\0', "")).expect("interior NUL removed")
}

unsafe fn set_entry(entry: *mut GtkWidget, text: &str) {
    let text = c(text);
    gtk_editable_set_text(entry as *mut GtkEditable, text.as_ptr());
}

unsafe fn set_label(label: *mut GtkWidget, text: &str) {
    let text = c(text);
    gtk_label_set_text(label as *mut GtkLabel, text.as_ptr());
}

fn strip_pango_markup(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    let mut entity = String::new();
    for ch in s.chars() {
        if in_tag {
            if ch == '>' {
                in_tag = false;
            }
            continue;
        }
        if ch == '<' {
            in_tag = true;
            continue;
        }
        if !entity.is_empty() {
            entity.push(ch);
            if ch == ';' {
                match entity.as_str() {
                    "&amp;" => out.push('&'),
                    "&lt;" => out.push('<'),
                    "&gt;" => out.push('>'),
                    "&quot;" => out.push('"'),
                    "&apos;" => out.push('\''),
                    _ => out.push_str(&entity),
                }
                entity.clear();
            }
            continue;
        }
        if ch == '&' {
            entity.push(ch);
        } else {
            out.push(ch);
        }
    }
    if !entity.is_empty() {
        out.push_str(&entity);
    }
    out
}

fn short(s: &str, max: usize) -> String {
    let mut v: String = s.chars().take(max).collect();
    if s.chars().count() > max {
        v.push('…');
    }
    v
}

unsafe fn connect(
    widget: *mut GtkWidget,
    signal: &str,
    handler: *const c_void,
    data: *mut c_void,
    destroy: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
) {
    let sig = c(signal);
    g_signal_connect_data(widget as *mut c_void, sig.as_ptr(), handler, data, destroy, 0);
}

unsafe extern "C" fn drop_tree_toggle_ctx(data: *mut c_void, _closure: *mut c_void) {
    if !data.is_null() {
        drop(Box::from_raw(data as *mut TreeToggleCtx));
    }
}
unsafe extern "C" fn on_tree_toggle(_button: *mut GtkButton, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let ctx = &*(data as *mut TreeToggleCtx);
    if ctx.app.is_null() {
        return;
    }
    let app = &mut *ctx.app;
    match ctx.section {
        TreeSection::Bookmarks => app.tree_bookmarks_open = !app.tree_bookmarks_open,
        TreeSection::Contents => app.tree_contents_open = !app.tree_contents_open,
        TreeSection::Links => app.tree_links_open = !app.tree_links_open,
    }
    for i in 0..app.tabs.len() {
        app.render_tree(i);
    }
}

unsafe extern "C" fn drop_link_ctx(data: *mut c_void, _closure: *mut c_void) {
    if !data.is_null() {
        drop(Box::from_raw(data as *mut LinkCtx));
    }
}
unsafe extern "C" fn drop_tab_close_ctx(data: *mut c_void, _closure: *mut c_void) {
    if !data.is_null() {
        drop(Box::from_raw(data as *mut TabCloseCtx));
    }
}
unsafe extern "C" fn on_tab_close_clicked(_button: *mut GtkButton, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let ctx = &*(data as *mut TabCloseCtx);
    if !ctx.app.is_null() {
        (*ctx.app).close_page(ctx.page);
    }
}
unsafe extern "C" fn on_link_clicked(_button: *mut GtkButton, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let ctx = &*(data as *mut LinkCtx);
    if !ctx.app.is_null() {
        (*ctx.app).open_link(ctx.target.clone());
    }
}
unsafe extern "C" fn on_inline_link(_label: *mut GtkLabel, uri: *const i8, data: *mut c_void) -> gboolean {
    if uri.is_null() || data.is_null() {
        return 0;
    }
    let target = CStr::from_ptr(uri).to_string_lossy().to_string();
    (*(data as *mut App)).open_link(target);
    1
}
unsafe extern "C" fn on_back(_b: *mut GtkButton, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    (*(data as *mut App)).back();
}
unsafe extern "C" fn on_forward(_b: *mut GtkButton, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    (*(data as *mut App)).forward();
}
unsafe extern "C" fn on_reload(_b: *mut GtkButton, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    (*(data as *mut App)).reload();
}
unsafe extern "C" fn on_new_tab(_b: *mut GtkButton, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    (*(data as *mut App)).add_tab();
}
unsafe extern "C" fn on_close_tab(_b: *mut GtkButton, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    (*(data as *mut App)).close_tab();
}
unsafe extern "C" fn on_sidebar(_b: *mut GtkButton, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    (*(data as *mut App)).toggle_sidebar();
}
unsafe extern "C" fn on_bookmark(_b: *mut GtkButton, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    (*(data as *mut App)).toggle_bookmark();
}
unsafe extern "C" fn on_bookmark_manager(_b: *mut GtkButton, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    (*(data as *mut App)).show_bookmark_manager();
}
unsafe extern "C" fn drop_copy_code_ctx(data: *mut c_void, _closure: *mut c_void) {
    if !data.is_null() {
        drop(Box::from_raw(data as *mut CopyCodeCtx));
    }
}
unsafe extern "C" fn on_copy_code(_b: *mut GtkButton, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let ctx = &*(data as *mut CopyCodeCtx);
    if ctx.app.is_null() {
        return;
    }
    let app = &mut *ctx.app;
    app.copy_text(&ctx.text);
    if app.active < app.tabs.len() {
        app.tabs[app.active].status = "Code copied".into();
        app.refresh_status();
    }
}
unsafe extern "C" fn on_copy_section(_b: *mut GtkButton, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    (*(data as *mut App)).copy_current_section();
}
unsafe extern "C" fn on_about(_b: *mut GtkButton, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    show_about_window((*(data as *mut App)).window);
}
unsafe extern "C" fn on_page_reordered(_n: *mut GtkNotebook, child: *mut GtkWidget, page_num: u32, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    (*(data as *mut App)).sync_tab_order_from_page(child, page_num as usize);
}
unsafe extern "C" fn on_bookmark_rename(entry: *mut GtkEntry, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let ctx = &mut *(data as *mut BookmarkEditCtx);
    let url = ctx.url.clone();
    let app = ctx.app;
    if app.is_null() {
        return;
    }
    let p = gtk_editable_get_text(entry as *mut GtkEditable);
    if !p.is_null() {
        let title = CStr::from_ptr(p).to_string_lossy().into_owned();
        (&mut *app).bookmark_rename(&url, title);
    }
}
unsafe extern "C" fn on_bookmark_action(_b: *mut GtkButton, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let ctx = &mut *(data as *mut BookmarkActionCtx);
    let url = ctx.url.clone();
    let action = ctx.action;
    let app = ctx.app;
    if app.is_null() {
        return;
    }
    (&mut *app).bookmark_action(&url, action);
}
unsafe extern "C" fn on_bookmark_manager_close(_w: *mut GtkWindow, data: *mut c_void) -> gboolean {
    if data.is_null() {
        return 0;
    }
    let app = &mut *(data as *mut App);
    gtk_widget_set_visible(app.bookmark_window, 0);
    1
}
unsafe extern "C" fn drop_bookmark_edit_ctx(data: *mut c_void, _closure: *mut c_void) {
    if !data.is_null() {
        drop(Box::from_raw(data as *mut BookmarkEditCtx));
    }
}
unsafe extern "C" fn drop_bookmark_action_ctx(data: *mut c_void, _closure: *mut c_void) {
    if !data.is_null() {
        drop(Box::from_raw(data as *mut BookmarkActionCtx));
    }
}
unsafe extern "C" fn on_entry_activate(entry: *mut GtkEntry, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let app = &mut *(data as *mut App);
    let p = gtk_editable_get_text(entry as *mut GtkEditable);
    if p.is_null() {
        return;
    }
    let u = CStr::from_ptr(p).to_string_lossy().trim().to_string();
    let tab = app.active;
    app.navigate(tab, u, true, true);
}
unsafe extern "C" fn on_switch_page(_n: *mut GtkNotebook, _page: *mut GtkWidget, page_num: u32, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let app = &mut *(data as *mut App);
    if (page_num as usize) < app.tabs.len() {
        app.active = page_num as usize;
        app.sync_active();
    }
}
unsafe extern "C" fn on_close_request(_w: *mut GtkWindow, data: *mut c_void) -> gboolean {
    if data.is_null() {
        return 0;
    }
    let app = &mut *(data as *mut App);
    app.save_session();
    g_main_loop_quit(app.main_loop);
    1
}
unsafe extern "C" fn on_tick(data: *mut c_void) -> gboolean {
    if data.is_null() {
        return 0;
    }
    let app = &mut *(data as *mut App);
    app.poll();
    1
}
unsafe extern "C" fn on_find_changed(_w: *mut GtkWidget, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    (&mut *(data as *mut App)).find_changed();
}
unsafe extern "C" fn on_find_next(_w: *mut GtkWidget, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    (&mut *(data as *mut App)).find_step(true);
}
unsafe extern "C" fn on_find_prev(_w: *mut GtkWidget, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    (&mut *(data as *mut App)).find_step(false);
}
unsafe extern "C" fn on_find_close(_w: *mut GtkWidget, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    (&mut *(data as *mut App)).hide_find();
}

/// Space/Shift+Space scroll by this fraction of a page, j/k by this many pixels.
const SCROLL_PAGE: f64 = 0.9;
const SCROLL_LINE: f64 = 60.0;

unsafe extern "C" fn on_key(
    _controller: *mut GtkEventControllerKey,
    keyval: u32,
    _keycode: u32,
    state: u32,
    data: *mut c_void,
) -> gboolean {
    if data.is_null() {
        return 0;
    }
    let app = &mut *(data as *mut App);
    let ctrl = state & GDK_CONTROL_MASK != 0;
    let shift = state & GDK_SHIFT_MASK != 0;
    let alt = state & GDK_ALT_MASK != 0;
    if ctrl && shift && (keyval == 0x0064 || keyval == 0x0044) {
        app.toggle_dark();
        return 1;
    }
    if ctrl && shift && (keyval == 0x0074 || keyval == 0x0054) {
        app.reopen_closed_tab();
        return 1;
    }
    if ctrl && shift && (keyval == 0x0063 || keyval == 0x0043) {
        app.copy_current_section();
        return 1;
    }
    if ctrl && keyval == 0x0020 {
        app.complete_address();
        app.g_pending = false;
        return 1;
    }
    if ctrl && keyval == 0xff55 {
        app.switch_tab_relative(-1);
        app.g_pending = false;
        return 1;
    }
    if ctrl && keyval == 0xff56 {
        app.switch_tab_relative(1);
        app.g_pending = false;
        return 1;
    }
    if alt && (0x0031..=0x0039).contains(&keyval) {
        app.switch_tab_number((keyval - 0x0030) as usize);
        app.g_pending = false;
        return 1;
    }
    if ctrl && (keyval == 0x002b || keyval == 0x003d || keyval == 0xffab) {
        app.change_reader_zoom(10);
        return 1;
    }
    if ctrl && (keyval == 0x002d || keyval == 0xffad) {
        app.change_reader_zoom(-10);
        return 1;
    }
    if ctrl && keyval == 0x0030 {
        app.change_reader_zoom(0);
        return 1;
    }
    if ctrl {
        match keyval {
            0x006c | 0x004c => {
                gtk_widget_grab_focus(app.entry);
                gtk_editable_select_region(app.entry as *mut GtkEditable, 0, -1);
                return 1;
            }
            0x0074 | 0x0054 => {
                app.add_tab();
                return 1;
            }
            0x0077 | 0x0057 => {
                app.close_tab();
                return 1;
            }
            0x0072 | 0x0052 => {
                app.reload();
                return 1;
            }
            0x0062 | 0x0042 => {
                app.toggle_sidebar();
                return 1;
            }
            0x0064 | 0x0044 => {
                app.toggle_bookmark();
                return 1;
            }
            0x006d | 0x004d => {
                app.show_bookmark_manager();
                return 1;
            }
            0x0066 | 0x0046 => {
                app.show_find();
                return 1;
            }
            0x0067 | 0x0047 => {
                app.find_step(true);
                return 1;
            }
            _ => {}
        }
    }
    if keyval == 0xff1b {
        app.hide_find();
        app.g_pending = false;
        return 1;
    }
    let editing_text = gtk_widget_has_focus(app.entry) != 0 || gtk_widget_has_focus(app.find_entry) != 0;
    if alt && keyval == GDK_KEY_LEFT {
        app.back();
        app.g_pending = false;
        return 1;
    }
    if alt && keyval == GDK_KEY_RIGHT {
        app.forward();
        app.g_pending = false;
        return 1;
    }
    if !editing_text && !ctrl && !alt && keyval == 0x002f {
        app.show_find();
        app.g_pending = false;
        return 1;
    }
    if !editing_text && !ctrl && !alt {
        let handled = match keyval {
            0x0020 => {
                app.scroll_document_pages(if shift { -SCROLL_PAGE } else { SCROLL_PAGE });
                true
            }
            0x006a => {
                app.scroll_document_pixels(SCROLL_LINE);
                true
            }
            0x006b => {
                app.scroll_document_pixels(-SCROLL_LINE);
                true
            }
            _ => false,
        };
        if handled {
            app.g_pending = false;
            return 1;
        }
    }
    if !editing_text && !ctrl && !alt && keyval == 0x0047 {
        app.scroll_document_edge(true);
        app.g_pending = false;
        return 1;
    }
    if !editing_text && !ctrl && !alt && keyval == 0x0067 {
        if app.g_pending {
            app.scroll_document_edge(false);
            app.g_pending = false;
        } else {
            app.g_pending = true;
        }
        return 1;
    }
    app.g_pending = false;
    0
}

const ICON_BACK: usize = 1;
const ICON_FORWARD: usize = 2;
const ICON_RELOAD: usize = 3;
const ICON_SIDEBAR: usize = 4;
const ICON_PLUS: usize = 5;
const ICON_CLOSE: usize = 6;
const ICON_BOOKMARK: usize = 7;

unsafe extern "C" fn draw_icon(area: *mut GtkDrawingArea, cr: *mut cairo_t, width: i32, height: i32, data: *mut c_void) {
    let kind = data as usize;
    let w = width as f64;
    let h = height as f64;
    let cx = w * 0.5;
    let cy = h * 0.5;
    // Draw in the button's text color so the icons follow the light and dark appearance.
    let mut color = GdkRGBA { red: 0.30, green: 0.32, blue: 0.35, alpha: 1.0 };
    gtk_widget_get_color(area as *mut GtkWidget, &mut color);
    cairo_set_source_rgba(cr, color.red as f64, color.green as f64, color.blue as f64, color.alpha as f64);
    cairo_set_line_width(cr, 1.8);
    match kind {
        ICON_BACK | ICON_FORWARD => {
            let dir = if kind == ICON_BACK { -1.0 } else { 1.0 };
            cairo_move_to(cr, cx + 5.0 * dir, cy - 6.0);
            cairo_line_to(cr, cx - 2.0 * dir, cy);
            cairo_line_to(cr, cx + 5.0 * dir, cy + 6.0);
            cairo_move_to(cr, cx - 2.0 * dir, cy);
            cairo_line_to(cr, cx - 7.0 * dir, cy);
        }
        ICON_RELOAD => {
            cairo_arc(cr, cx, cy, 6.2, 0.45, 5.55);
            cairo_move_to(cr, cx + 5.5, cy - 5.5);
            cairo_line_to(cr, cx + 7.0, cy - 0.8);
            cairo_line_to(cr, cx + 2.3, cy - 1.8);
        }
        ICON_SIDEBAR => {
            cairo_rectangle(cr, cx - 7.0, cy - 6.0, 14.0, 12.0);
            cairo_move_to(cr, cx - 2.2, cy - 6.0);
            cairo_line_to(cr, cx - 2.2, cy + 6.0);
        }
        ICON_PLUS => {
            cairo_move_to(cr, cx - 6.0, cy);
            cairo_line_to(cr, cx + 6.0, cy);
            cairo_move_to(cr, cx, cy - 6.0);
            cairo_line_to(cr, cx, cy + 6.0);
        }
        ICON_CLOSE => {
            cairo_move_to(cr, cx - 5.0, cy - 5.0);
            cairo_line_to(cr, cx + 5.0, cy + 5.0);
            cairo_move_to(cr, cx + 5.0, cy - 5.0);
            cairo_line_to(cr, cx - 5.0, cy + 5.0);
        }
        ICON_BOOKMARK => {
            let pts = [
                (0.0, -7.0),
                (2.1, -2.3),
                (7.0, -2.1),
                (3.2, 1.0),
                (4.4, 6.0),
                (0.0, 3.2),
                (-4.4, 6.0),
                (-3.2, 1.0),
                (-7.0, -2.1),
                (-2.1, -2.3),
                (0.0, -7.0),
            ];
            cairo_move_to(cr, cx + pts[0].0, cy + pts[0].1);
            for &(x, y) in &pts[1..] {
                cairo_line_to(cr, cx + x, cy + y);
            }
        }
        _ => {}
    }
    cairo_stroke(cr);
}

unsafe fn icon_button(kind: usize, tooltip: &str) -> *mut GtkWidget {
    let button = gtk_button_new();
    gtk_widget_add_css_class(button, c("icon-button").as_ptr());
    let area = gtk_drawing_area_new();
    gtk_widget_set_size_request(area, 20, 20);
    gtk_drawing_area_set_draw_func(area as *mut GtkDrawingArea, Some(draw_icon), kind as *mut c_void, None);
    gtk_button_set_child(button as *mut GtkButton, area);
    gtk_widget_set_tooltip_text(button, c(tooltip).as_ptr());
    button
}

unsafe fn install_embedded_css() {
    const CSS: &str = r#"
        window { background: @theme_bg_color; }
        .toolbar { padding: 2px 4px; }
        .icon-button { min-width: 30px; min-height: 30px; padding: 2px; border-radius: 7px; }
        .tab-close { min-width: 22px; min-height: 22px; padding: 0; margin: 0; }
        .tab-title { margin: 0 2px; }
        .urlbar { border-radius: 8px; padding: 4px 8px; }
        .document { font-family: Sans; font-size: 11pt; color: @theme_fg_color; }
        .md-text, .md-bullet, .md-task, .md-footnote { margin: 4px 0; }
        .md-quote { margin: 7px 0; }
        .md-math { margin: 8px 18px; padding: 8px 12px; font-family: serif; font-size: 15pt; }
        .md-h1 { font-size: 22pt; font-weight: 700; margin-top: 22px; margin-bottom: 10px; }
        .md-h2 { font-size: 17pt; font-weight: 700; margin-top: 19px; margin-bottom: 8px; }
        .md-h3 { font-size: 13pt; font-weight: 700; margin-top: 15px; margin-bottom: 6px; }
        .md-h4 { font-size: 11.5pt; font-weight: 700; margin-top: 12px; margin-bottom: 5px; }
        .md-h5 { font-size: 10.8pt; font-weight: 700; margin-top: 10px; margin-bottom: 4px; }
        .md-h6 { font-size: 10.2pt; font-weight: 700; margin-top: 9px; margin-bottom: 4px; opacity: 0.88; }
        .md-quote { opacity: 0.86; padding: 5px 10px 5px 13px; border-left: 3px solid alpha(@theme_selected_bg_color, 0.48); background: alpha(@theme_fg_color, 0.025); }
        .md-footnote { opacity: 0.88; font-size: 0.92em; }
        .md-code { font-family: Monospace; background: alpha(@theme_fg_color, 0.055); padding: 5px 8px; border-radius: 5px; }
        .md-table-scroll { margin: 12px 0; border: 1px solid alpha(@theme_fg_color, 0.18); border-radius: 5px; }
        .md-table { background: alpha(@theme_fg_color, 0.10); }
        .md-table-header { font-weight: bold; background: alpha(@theme_fg_color, 0.10); padding: 7px 9px; }
        .md-table-cell { background: @theme_base_color; padding: 6px 9px; }
        .md-table-row { padding: 3px 7px; }
        .md-table-sep { min-height: 1px; opacity: 0.25; }
        .document > row, .document > row:hover, .document > row:focus-within { padding: 0; min-height: 0; background: none; outline: none; }
        .md-rule { opacity: 0.25; margin: 14px 0; }
        .md-image-wrap { margin: 12px 0 18px 0; }
        .md-image-caption { opacity: 0.62; font-size: 9pt; }
        .tree-heading { font-weight: 700; margin: 4px 5px 7px 5px; }
        .tree-heading-button { font-weight: 700; padding: 5px 7px; border: none; box-shadow: none; background: transparent; }
        .tree-heading-button:hover { background: alpha(@theme_selected_bg_color, 0.10); }
        .tree-empty { opacity: 0.58; font-size: 9pt; margin: 0 8px 4px 8px; }
        .sidebar { background: alpha(@theme_fg_color, 0.035); }
        .tree-link { padding: 4px 8px; border: none; box-shadow: none; background: transparent; }
        .tree-link:hover { background: alpha(@theme_selected_bg_color, 0.14); }
        .tree-top { font-weight: 700; }
        .tree-current { background: alpha(@theme_selected_bg_color, 0.22); }
        .searchbar { padding: 2px 8px; }
        .search-hit { background: alpha(#f6c344, 0.20); border-radius: 4px; }
        .search-current { background: alpha(#f6c344, 0.48); }
        .status { opacity: 0.72; font-size: 9pt; }
        .about-title { font-size: 20pt; font-weight: 700; }
        .about-license-title { font-weight: 700; margin-top: 8px; }
        .md-code-block { background: alpha(@theme_fg_color, 0.052); padding: 8px 10px; border-radius: 8px; margin: 10px 0; border: 1px solid alpha(@theme_fg_color, 0.10); }
        .md-code-head { opacity: 0.78; font-size: 9pt; }
        .zoom-80 .md-text, .zoom-80 .md-bullet, .zoom-80 .md-task, .zoom-80 .md-quote, .zoom-80 .md-table-row, .zoom-80 .md-footnote { font-size: 8.8pt; }
        .zoom-90 .md-text, .zoom-90 .md-bullet, .zoom-90 .md-task, .zoom-90 .md-quote, .zoom-90 .md-table-row, .zoom-90 .md-footnote { font-size: 9.9pt; }
        .zoom-110 .md-text, .zoom-110 .md-bullet, .zoom-110 .md-task, .zoom-110 .md-quote, .zoom-110 .md-table-row, .zoom-110 .md-footnote { font-size: 12.1pt; }
        .zoom-120 .md-text, .zoom-120 .md-bullet, .zoom-120 .md-task, .zoom-120 .md-quote, .zoom-120 .md-table-row, .zoom-120 .md-footnote { font-size: 13.2pt; }
        .zoom-130 .md-text, .zoom-130 .md-bullet, .zoom-130 .md-task, .zoom-130 .md-quote, .zoom-130 .md-table-row, .zoom-130 .md-footnote { font-size: 14.3pt; }
        .zoom-140 .md-text, .zoom-140 .md-bullet, .zoom-140 .md-task, .zoom-140 .md-quote, .zoom-140 .md-table-row, .zoom-140 .md-footnote { font-size: 15.4pt; }
        .zoom-150 .md-text, .zoom-150 .md-bullet, .zoom-150 .md-task, .zoom-150 .md-quote, .zoom-150 .md-table-row, .zoom-150 .md-footnote { font-size: 16.5pt; }
        .zoom-160 .md-text, .zoom-160 .md-bullet, .zoom-160 .md-task, .zoom-160 .md-quote, .zoom-160 .md-table-row, .zoom-160 .md-footnote { font-size: 17.6pt; }
        .zoom-80 .md-h1 { font-size:17.6pt; } .zoom-90 .md-h1 { font-size:19.8pt; } .zoom-110 .md-h1 { font-size:24.2pt; } .zoom-120 .md-h1 { font-size:26.4pt; } .zoom-130 .md-h1 { font-size:28.6pt; } .zoom-140 .md-h1 { font-size:30.8pt; } .zoom-150 .md-h1 { font-size:33pt; } .zoom-160 .md-h1 { font-size:35.2pt; }
        .zoom-80 .md-h2 { font-size:13.6pt; } .zoom-90 .md-h2 { font-size:15.3pt; } .zoom-110 .md-h2 { font-size:18.7pt; } .zoom-120 .md-h2 { font-size:20.4pt; } .zoom-130 .md-h2 { font-size:22.1pt; } .zoom-140 .md-h2 { font-size:23.8pt; } .zoom-150 .md-h2 { font-size:25.5pt; } .zoom-160 .md-h2 { font-size:27.2pt; }
        .zoom-80 .md-h3 { font-size:10.4pt; } .zoom-90 .md-h3 { font-size:11.7pt; } .zoom-110 .md-h3 { font-size:14.3pt; } .zoom-120 .md-h3 { font-size:15.6pt; } .zoom-130 .md-h3 { font-size:16.9pt; } .zoom-140 .md-h3 { font-size:18.2pt; } .zoom-150 .md-h3 { font-size:19.5pt; } .zoom-160 .md-h3 { font-size:20.8pt; }
        .zoom-80 .md-h4 { font-size:9.2pt; } .zoom-90 .md-h4 { font-size:10.4pt; } .zoom-110 .md-h4 { font-size:12.7pt; } .zoom-120 .md-h4 { font-size:13.8pt; } .zoom-130 .md-h4 { font-size:15pt; } .zoom-140 .md-h4 { font-size:16.1pt; } .zoom-150 .md-h4 { font-size:17.3pt; } .zoom-160 .md-h4 { font-size:18.4pt; }
        .zoom-80 .md-code { font-size:8.8pt; } .zoom-90 .md-code { font-size:9.9pt; } .zoom-110 .md-code { font-size:12.1pt; } .zoom-120 .md-code { font-size:13.2pt; } .zoom-130 .md-code { font-size:14.3pt; } .zoom-140 .md-code { font-size:15.4pt; } .zoom-150 .md-code { font-size:16.5pt; } .zoom-160 .md-code { font-size:17.6pt; }
        .zoom-80 .md-math { font-size:12pt; } .zoom-90 .md-math { font-size:13.5pt; } .zoom-110 .md-math { font-size:16.5pt; } .zoom-120 .md-math { font-size:18pt; } .zoom-130 .md-math { font-size:19.5pt; } .zoom-140 .md-math { font-size:21pt; } .zoom-150 .md-math { font-size:22.5pt; } .zoom-160 .md-math { font-size:24pt; }

    "#;
    let display = gdk_display_get_default();
    if display.is_null() {
        return;
    }
    let provider = gtk_css_provider_new();
    gtk_css_provider_load_from_data(provider, CSS.as_ptr() as *const i8, CSS.len() as isize);
    gtk_style_context_add_provider_for_display(display, provider as *mut c_void, GTK_STYLE_PROVIDER_PRIORITY_APPLICATION);
    g_object_unref(provider as *mut c_void);
}

unsafe fn set_scaled_pixbuf(picture: *mut GtkWidget, pix: *mut GdkPixbuf) -> Result<(i32, i32), String> {
    if pix.is_null() {
        return Err("null pixbuf".into());
    }
    let w = gdk_pixbuf_get_width(pix);
    let h = gdk_pixbuf_get_height(pix);
    if w <= 0 || h <= 0 {
        return Err("image decoder returned invalid dimensions".into());
    }
    let max_w = 1200;
    let max_h = 900;
    let mut shown = pix;
    let mut shown_w = w;
    let mut shown_h = h;
    let mut scaled: *mut GdkPixbuf = ptr::null_mut();
    if w > max_w || h > max_h {
        let sx = max_w as f64 / w.max(1) as f64;
        let sy = max_h as f64 / h.max(1) as f64;
        let scale = sx.min(sy);
        shown_w = ((w as f64 * scale).round() as i32).max(1);
        shown_h = ((h as f64 * scale).round() as i32).max(1);
        scaled = gdk_pixbuf_scale_simple(pix, shown_w, shown_h, 2);
        if !scaled.is_null() {
            shown = scaled;
        } else {
            shown_w = w;
            shown_h = h;
        }
    }

    // GtkPicture is paintable-native in GTK4. Convert the decoded pixbuf into a
    // GdkTexture instead of relying on the compatibility gtk_picture_set_pixbuf path.
    let texture = gdk_texture_new_for_pixbuf(shown);
    if texture.is_null() {
        if !scaled.is_null() {
            g_object_unref(scaled as *mut c_void);
        }
        return Err("GdkTexture creation failed".into());
    }
    gtk_picture_set_paintable(picture as *mut GtkPicture, texture as *mut GdkPaintable);
    gtk_widget_set_size_request(picture, shown_w, shown_h);
    g_object_unref(texture as *mut c_void);
    if !scaled.is_null() {
        g_object_unref(scaled as *mut c_void);
    }
    Ok((shown_w, shown_h))
}

fn current_timeval() -> GTimeVal {
    unsafe {
        let us = g_get_real_time();
        GTimeVal { tv_sec: (us / 1_000_000) as _, tv_usec: (us % 1_000_000) as _ }
    }
}

/// Decode PNG/JPEG/GIF. Animated GIFs return the loader+iterator so the caller
/// can keep them alive and advance frames from the GTK timer.
unsafe fn set_picture_bytes(
    picture: *mut GtkWidget,
    bytes: &[u8],
) -> Result<Option<(*mut GdkPixbufLoader, *mut GdkPixbufAnimationIter)>, String> {
    let loader = gdk_pixbuf_loader_new();
    if loader.is_null() {
        return Err("gdk-pixbuf loader creation failed".into());
    }
    let mut err: *mut GError = ptr::null_mut();
    if gdk_pixbuf_loader_write(loader, bytes.as_ptr(), bytes.len(), &mut err) == 0 {
        g_object_unref(loader as *mut c_void);
        return Err("unsupported or corrupt PNG/JPEG/GIF".into());
    }
    if gdk_pixbuf_loader_close(loader, &mut err) == 0 {
        g_object_unref(loader as *mut c_void);
        return Err("incomplete or corrupt image".into());
    }

    let animation = gdk_pixbuf_loader_get_animation(loader);
    if !animation.is_null() && gdk_pixbuf_animation_is_static_image(animation) == 0 {
        let now = current_timeval();
        let iter = gdk_pixbuf_animation_get_iter(animation, &now);
        if !iter.is_null() {
            let pix = gdk_pixbuf_animation_iter_get_pixbuf(iter);
            if !pix.is_null() {
                set_scaled_pixbuf(picture, pix)?;
            }
            return Ok(Some((loader, iter)));
        }
    }

    let pix = gdk_pixbuf_loader_get_pixbuf(loader);
    if pix.is_null() {
        g_object_unref(loader as *mut c_void);
        return Err("image decoder returned no pixels".into());
    }
    set_scaled_pixbuf(picture, pix)?;
    g_object_unref(loader as *mut c_void);
    Ok(None)
}

unsafe fn attach_zoom(picture: *mut GtkWidget, bytes: Vec<u8>, title: String) {
    let gesture = gtk_gesture_click_new();
    let ctx = Box::new(ZoomCtx { bytes, title });
    connect(
        gesture as *mut GtkWidget,
        "released",
        on_image_zoom as *const c_void,
        Box::into_raw(ctx) as *mut c_void,
        Some(drop_zoom_ctx),
    );
    gtk_widget_add_controller(picture, gesture as *mut GtkEventController);
    gtk_widget_set_tooltip_text(picture, c("Click to open image viewer").as_ptr());
}

unsafe extern "C" fn drop_zoom_ctx(data: *mut c_void, _closure: *mut c_void) {
    if !data.is_null() {
        drop(Box::from_raw(data as *mut ZoomCtx));
    }
}

unsafe fn viewer_render(ctx: &mut ViewerCtx) {
    let loader = gdk_pixbuf_loader_new();
    if loader.is_null() {
        return;
    }
    let mut err: *mut GError = ptr::null_mut();
    if gdk_pixbuf_loader_write(loader, ctx.bytes.as_ptr(), ctx.bytes.len(), &mut err) == 0
        || gdk_pixbuf_loader_close(loader, &mut err) == 0
    {
        g_object_unref(loader as *mut c_void);
        return;
    }
    let pix = gdk_pixbuf_loader_get_pixbuf(loader);
    if pix.is_null() {
        g_object_unref(loader as *mut c_void);
        return;
    }
    let w = gdk_pixbuf_get_width(pix).max(1);
    let h = gdk_pixbuf_get_height(pix).max(1);
    let tw = ((w as f64 * ctx.scale).round() as i32).max(1);
    let th = ((h as f64 * ctx.scale).round() as i32).max(1);
    let shown = if tw == w && th == h {
        g_object_ref(pix as *mut c_void) as *mut GdkPixbuf
    } else {
        gdk_pixbuf_scale_simple(pix, tw, th, 2)
    };
    if !shown.is_null() {
        let texture = gdk_texture_new_for_pixbuf(shown);
        if !texture.is_null() {
            gtk_picture_set_paintable(ctx.picture as *mut GtkPicture, texture as *mut GdkPaintable);
            gtk_widget_set_size_request(ctx.picture, tw, th);
            g_object_unref(texture as *mut c_void);
        }
        g_object_unref(shown as *mut c_void);
    }
    g_object_unref(loader as *mut c_void);
}

unsafe extern "C" fn viewer_fit(_button: *mut GtkButton, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let ctx = &mut *(data as *mut ViewerCtx);
    ctx.scale = ctx.fit_scale;
    viewer_render(ctx);
}
unsafe extern "C" fn viewer_one(_button: *mut GtkButton, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let ctx = &mut *(data as *mut ViewerCtx);
    ctx.scale = 1.0;
    viewer_render(ctx);
}
unsafe extern "C" fn viewer_minus(_button: *mut GtkButton, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let ctx = &mut *(data as *mut ViewerCtx);
    ctx.scale = (ctx.scale / 1.25).max(0.05);
    viewer_render(ctx);
}
unsafe extern "C" fn viewer_plus(_button: *mut GtkButton, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let ctx = &mut *(data as *mut ViewerCtx);
    ctx.scale = (ctx.scale * 1.25).min(8.0);
    viewer_render(ctx);
}
unsafe extern "C" fn viewer_destroy(_window: *mut GtkWindow, _data: *mut c_void) {}
unsafe extern "C" fn drop_viewer_ctx(data: *mut c_void, _closure: *mut c_void) {
    if !data.is_null() {
        drop(Box::from_raw(data as *mut ViewerCtx));
    }
}

unsafe extern "C" fn on_image_zoom(_gesture: *mut GtkGestureClick, _n_press: i32, _x: f64, _y: f64, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let z = &*(data as *mut ZoomCtx);
    let window = gtk_window_new();
    let title = if z.title.is_empty() { "Image" } else { &z.title };
    gtk_window_set_title(window as *mut GtkWindow, c(title).as_ptr());
    gtk_window_set_default_size(window as *mut GtkWindow, 1000, 760);

    let root = gtk_box_new(GTK_ORIENTATION_VERTICAL, 4);
    let tools = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 4);
    gtk_widget_set_margin_start(tools, 6);
    gtk_widget_set_margin_end(tools, 6);
    gtk_widget_set_margin_top(tools, 6);
    let fit = gtk_button_new_with_label(c("Fit").as_ptr());
    let one = gtk_button_new_with_label(c("1:1").as_ptr());
    let minus = gtk_button_new_with_label(c("−").as_ptr());
    let plus = gtk_button_new_with_label(c("+").as_ptr());
    for b in [fit, one, minus, plus] {
        gtk_box_append(tools as *mut GtkBox, b);
    }
    gtk_box_append(root as *mut GtkBox, tools);

    let scroll = gtk_scrolled_window_new();
    gtk_widget_set_hexpand(scroll, 1);
    gtk_widget_set_vexpand(scroll, 1);
    let picture = gtk_picture_new();
    gtk_picture_set_can_shrink(picture as *mut GtkPicture, 0);
    gtk_picture_set_keep_aspect_ratio(picture as *mut GtkPicture, 1);
    gtk_scrolled_window_set_child(scroll as *mut GtkScrolledWindow, picture);
    gtk_box_append(root as *mut GtkBox, scroll);
    gtk_window_set_child(window as *mut GtkWindow, root);

    let (w, h) = image_dimensions(&z.bytes).unwrap_or((900, 700));
    let fit_scale = (900.0 / w.max(1) as f64).min(650.0 / h.max(1) as f64).min(1.0).max(0.05);
    let ctx = Box::new(ViewerCtx { picture, bytes: z.bytes.clone(), scale: fit_scale, fit_scale });
    let ctxp = Box::into_raw(ctx);
    connect(fit, "clicked", viewer_fit as *const c_void, ctxp as *mut c_void, None);
    connect(one, "clicked", viewer_one as *const c_void, ctxp as *mut c_void, None);
    connect(minus, "clicked", viewer_minus as *const c_void, ctxp as *mut c_void, None);
    connect(plus, "clicked", viewer_plus as *const c_void, ctxp as *mut c_void, None);
    connect(window, "destroy", viewer_destroy as *const c_void, ctxp as *mut c_void, Some(drop_viewer_ctx));
    viewer_render(&mut *ctxp);
    gtk_window_present(window as *mut GtkWindow);
}

pub(crate) unsafe fn show_about_window(_parent: *mut GtkWidget) {
    let window = gtk_window_new();
    gtk_window_set_title(window as *mut GtkWindow, c("About SilkMark").as_ptr());
    gtk_window_set_default_size(window as *mut GtkWindow, 520, 360);

    let root = gtk_box_new(GTK_ORIENTATION_VERTICAL, 10);
    gtk_widget_set_margin_start(root, 24);
    gtk_widget_set_margin_end(root, 24);
    gtk_widget_set_margin_top(root, 22);
    gtk_widget_set_margin_bottom(root, 22);
    gtk_window_set_child(window as *mut GtkWindow, root);

    let about_title = format!("SilkMark {}", env!("CARGO_PKG_VERSION"));
    let title = gtk_label_new(c(&about_title).as_ptr());
    gtk_widget_add_css_class(title, c("about-title").as_ptr());
    gtk_label_set_xalign(title as *mut GtkLabel, 0.0);
    gtk_box_append(root as *mut GtkBox, title);

    let tagline = gtk_label_new(c("A lightweight native Markdown browser.").as_ptr());
    gtk_label_set_xalign(tagline as *mut GtkLabel, 0.0);
    gtk_box_append(root as *mut GtkBox, tagline);

    let author = gtk_label_new(c("Created by Zsolt Krüpl").as_ptr());
    gtk_label_set_xalign(author as *mut GtkLabel, 0.0);
    gtk_box_append(root as *mut GtkBox, author);

    let tech = gtk_label_new(c("GTK 4 via direct C ABI/FFI • system libcurl • zero crates.io dependencies").as_ptr());
    gtk_label_set_wrap(tech as *mut GtkLabel, 1);
    gtk_label_set_xalign(tech as *mut GtkLabel, 0.0);
    gtk_box_append(root as *mut GtkBox, tech);

    let license_title = gtk_label_new(c("License: MIT License").as_ptr());
    gtk_widget_add_css_class(license_title, c("about-license-title").as_ptr());
    gtk_label_set_xalign(license_title as *mut GtkLabel, 0.0);
    gtk_box_append(root as *mut GtkBox, license_title);

    let license = gtk_label_new(c("Copyright (c) 2026 Zsolt Krüpl\n\nPermission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files to deal in the Software without restriction, subject to the conditions of the MIT License.\n\nTHE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED.").as_ptr());
    gtk_label_set_wrap(license as *mut GtkLabel, 1);
    gtk_label_set_selectable(license as *mut GtkLabel, 1);
    gtk_label_set_xalign(license as *mut GtkLabel, 0.0);
    gtk_box_append(root as *mut GtkBox, license);

    let close = gtk_button_new_with_label(c("Close").as_ptr());
    gtk_widget_set_tooltip_text(close, c("Close About window").as_ptr());
    gtk_box_append(root as *mut GtkBox, close);
    connect(close, "clicked", on_about_close as *const c_void, window as *mut c_void, None);
    // The selectable license label would otherwise take the initial focus and show up selected.
    gtk_window_set_focus(window as *mut GtkWindow, close);

    gtk_window_present(window as *mut GtkWindow);
}

unsafe extern "C" fn on_about_close(_b: *mut GtkButton, data: *mut c_void) {
    if !data.is_null() {
        gtk_window_destroy(data as *mut GtkWindow);
    }
}

unsafe fn image_dimensions(bytes: &[u8]) -> Option<(i32, i32)> {
    let loader = gdk_pixbuf_loader_new();
    if loader.is_null() {
        return None;
    }
    let mut err: *mut GError = ptr::null_mut();
    if gdk_pixbuf_loader_write(loader, bytes.as_ptr(), bytes.len(), &mut err) == 0
        || gdk_pixbuf_loader_close(loader, &mut err) == 0
    {
        g_object_unref(loader as *mut c_void);
        return None;
    }
    let pix = gdk_pixbuf_loader_get_pixbuf(loader);
    let out = if pix.is_null() { None } else { Some((gdk_pixbuf_get_width(pix), gdk_pixbuf_get_height(pix))) };
    g_object_unref(loader as *mut c_void);
    out
}

fn main() {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    if raw_args.iter().any(|a| a == "-h" || a == "--help") {
        println!(
            "SilkMark v{}
usage: silkmark [options] [URL-or-file ...]",
            env!("CARGO_PKG_VERSION")
        );
        println!("

-v, --verbose              print opened resources, redirects and cache information to stderr
--stats                     print parse/render/image-queue performance statistics to stderr
--disk-cache               enable persistent HTTP document/image cache
--offline                  do not use the network; network resources must be cached
--clear-cache              remove the persistent SilkMark HTTP cache and exit
--no-restore               do not restore the previous tabs/session on startup
--theme MODE              appearance: light, dark or system (default: follow the desktop; Ctrl+Shift+D toggles and is remembered)
--system-theme             alias for --theme system
--max-document-mib N       maximum Markdown document size (default: 4 MiB)
--max-image-mib N          maximum individual image size (default: 12 MiB)
--max-redirects N          maximum HTTPS redirects (default: 8, maximum: 64)
--connect-timeout N        connection timeout in seconds (default: 10)
--timeout N                total request timeout in seconds (default: 30)
--allow-http               allow plain HTTP URLs in addition to HTTPS (insecure; intended for trusted LANs)
--allow-host HOST          allow only this HTTP(S) host; repeatable. A leading dot also permits subdomains

Examples:
  silkmark --allow-host example.org https://example.org/README.md
  silkmark --allow-host .example.org --max-image-mib 8 https://docs.example.org/index.md
  silkmark --allow-http --allow-host server http://server:8080/index.md

HTTPS and local file:// resources are supported by default. Plain HTTP is accepted only with --allow-http. ftp:, data: and javascript: targets remain rejected. Redirects are restricted to the protocols allowed for the run and, when --allow-host is used, to the same allowlist.

Each URL or local Markdown path opens in its own tab. Relative links and images are resolved against the current document. Ctrl+D toggles a bookmark. Ctrl+M opens the bookmark manager. Ctrl+F searches. Ctrl+Space completes the URL from bookmarks/history or section headings. Ctrl+Shift+C copies the current document/section link. Ctrl+PageUp/PageDown switches tabs, Alt+1..9 selects a tab, / opens search, g g jumps to the top, and G jumps to the bottom. Tabs can be dragged to reorder. Ctrl+Shift+T reopens the last closed tab.");
        return;
    }

    let mut verbose = false;
    let mut stats = false;
    let mut theme = String::from("system");
    let mut theme_given = false;
    let mut no_restore = false;
    let mut disk_cache = false;
    let mut offline = false;
    let mut allow_http = false;
    let mut clear_cache = false;
    let mut document_mib = 4usize;
    let mut image_mib = 12usize;
    let mut max_redirects = 8usize;
    let mut connect_timeout = 10usize;
    let mut total_timeout = 30usize;
    let mut allow_hosts: Vec<String> = Vec::new();
    let mut start_urls: Vec<String> = Vec::new();
    let mut i = 0usize;
    let parse_num = |name: &str, value: &str| -> usize {
        match value.parse::<usize>() {
            Ok(v) => v,
            Err(_) => {
                eprintln!("Invalid value for {name}: {value}");
                std::process::exit(2);
            }
        }
    };
    while i < raw_args.len() {
        match raw_args[i].as_str() {
            "-v" | "--verbose" => verbose = true,
            "--stats" => stats = true,
            "--system-theme" => {
                theme = String::from("system");
                theme_given = true;
            }
            "--no-restore" => no_restore = true,
            "--disk-cache" => disk_cache = true,
            "--offline" => {
                offline = true;
                disk_cache = true;
            }
            "--allow-http" => allow_http = true,
            "--clear-cache" => clear_cache = true,
            "--theme" | "--max-document-mib" | "--max-image-mib" | "--max-redirects" | "--connect-timeout" | "--timeout"
            | "--allow-host" => {
                if i + 1 >= raw_args.len() {
                    eprintln!("Missing value after {}", raw_args[i]);
                    std::process::exit(2);
                }
                let name = raw_args[i].clone();
                i += 1;
                let value = &raw_args[i];
                match name.as_str() {
                    "--theme" => {
                        theme = value.to_ascii_lowercase();
                        theme_given = true;
                    }
                    "--max-document-mib" => document_mib = parse_num(&name, value),
                    "--max-image-mib" => image_mib = parse_num(&name, value),
                    "--max-redirects" => max_redirects = parse_num(&name, value),
                    "--connect-timeout" => connect_timeout = parse_num(&name, value),
                    "--timeout" => total_timeout = parse_num(&name, value),
                    "--allow-host" => allow_hosts.push(value.clone()),
                    _ => unreachable!(),
                }
            }
            x if x.starts_with('-') => {
                eprintln!("Unknown option: {x}");
                eprintln!("Try --help");
                std::process::exit(2);
            }
            _ => start_urls.push(raw_args[i].clone()),
        }
        i += 1;
    }
    if clear_cache {
        match net::clear_disk_cache() {
            Ok(()) => println!("SilkMark disk cache cleared."),
            Err(e) => eprintln!("{e}"),
        }
        return;
    }
    if document_mib == 0 || document_mib > 1024 {
        eprintln!("--max-document-mib must be between 1 and 1024");
        std::process::exit(2);
    }
    if image_mib == 0 || image_mib > 1024 {
        eprintln!("--max-image-mib must be between 1 and 1024");
        std::process::exit(2);
    }
    if max_redirects > 64 {
        eprintln!("--max-redirects cannot exceed 64");
        std::process::exit(2);
    }
    if connect_timeout == 0 || connect_timeout > 86_400 || total_timeout == 0 || total_timeout > 86_400 {
        eprintln!("timeouts must be between 1 and 86400 seconds");
        std::process::exit(2);
    }
    if total_timeout < connect_timeout {
        eprintln!("--timeout must be greater than or equal to --connect-timeout");
        std::process::exit(2);
    }
    net::set_verbose(verbose);
    net::set_disk_cache(disk_cache);
    net::set_offline(offline);
    net::set_allow_http(allow_http);
    net::set_limits(document_mib, image_mib, max_redirects, connect_timeout, total_timeout);
    net::set_allowed_hosts(allow_hosts);
    let restore_state = if !no_restore && start_urls.is_empty() { session::load() } else { None };
    if !matches!(theme.as_str(), "light" | "dark" | "system") {
        eprintln!("--theme must be light, dark or system");
        std::process::exit(2);
    }
    unsafe {
        gtk_init();
        // A flag wins, then the choice remembered by Ctrl+Shift+D, then the desktop's preference.
        let saved_dark = session::load().and_then(|s| s.dark);
        let remembered = if theme_given { None } else { saved_dark };
        let dark = match (theme.as_str(), remembered) {
            ("light", _) => false,
            ("dark", _) => true,
            (_, Some(dark)) => dark,
            _ => theme::system_prefers_dark(),
        };
        theme::apply(dark);
        install_embedded_css();
        let main_loop = g_main_loop_new(ptr::null_mut(), 0);
        let window = gtk_window_new();
        let window_title = format!("SilkMark v{} / GTK4", env!("CARGO_PKG_VERSION"));
        gtk_window_set_title(window as *mut GtkWindow, c(&window_title).as_ptr());
        let (win_w, win_h) = restore_state.as_ref().map(|s| (s.width, s.height)).unwrap_or((1100, 760));
        gtk_window_set_default_size(window as *mut GtkWindow, win_w.max(320), win_h.max(240));
        let root = gtk_box_new(GTK_ORIENTATION_VERTICAL, 6);
        gtk_window_set_child(window as *mut GtkWindow, root);
        let toolbar = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 6);
        gtk_widget_add_css_class(toolbar, c("toolbar").as_ptr());
        gtk_widget_set_margin_start(toolbar, 8);
        gtk_widget_set_margin_end(toolbar, 8);
        gtk_widget_set_margin_top(toolbar, 8);
        gtk_box_append(root as *mut GtkBox, toolbar);

        let back = icon_button(ICON_BACK, "Back (Alt+Left)");
        let forward = icon_button(ICON_FORWARD, "Forward (Alt+Right)");
        let reload = icon_button(ICON_RELOAD, "Reload (Ctrl+R)");
        let sidebar = icon_button(ICON_SIDEBAR, "Toggle sidebar (Ctrl+B)");
        let entry = gtk_entry_new();
        gtk_widget_add_css_class(entry, c("urlbar").as_ptr());
        gtk_widget_set_hexpand(entry, 1);
        let bookmark = icon_button(ICON_BOOKMARK, "Add/remove bookmark (Ctrl+D)");
        let bookmark_manager = gtk_button_new_with_label(c("Bookmarks").as_ptr());
        gtk_widget_set_tooltip_text(bookmark_manager, c("Manage bookmarks (Ctrl+M)").as_ptr());
        let copy_section = gtk_button_new_with_label(c("Copy link").as_ptr());
        gtk_widget_set_tooltip_text(copy_section, c("Copy current document/section link (Ctrl+Shift+C)").as_ptr());
        let about = gtk_button_new_with_label(c("About").as_ptr());
        gtk_widget_set_tooltip_text(about, c("About SilkMark").as_ptr());
        let new_tab = icon_button(ICON_PLUS, "New tab (Ctrl+T)");
        let close_tab = icon_button(ICON_CLOSE, "Close active tab (Ctrl+W)");
        for w in [back, forward, reload, sidebar, entry, bookmark, bookmark_manager, copy_section, about, new_tab, close_tab] {
            gtk_box_append(toolbar as *mut GtkBox, w);
        }

        let find_bar = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 6);
        gtk_widget_add_css_class(find_bar, c("searchbar").as_ptr());
        gtk_widget_set_visible(find_bar, 0);
        let find_entry = gtk_entry_new();
        gtk_widget_set_hexpand(find_entry, 1);
        gtk_widget_set_tooltip_text(find_entry, c("Find in document (Ctrl+F)").as_ptr());
        let find_prev = gtk_button_new_with_label(c("↑ Previous").as_ptr());
        let find_next = gtk_button_new_with_label(c("↓ Next").as_ptr());
        let find_count = gtk_label_new(c("").as_ptr());
        let find_close = gtk_button_new_with_label(c("×").as_ptr());
        for w in [find_entry, find_prev, find_next, find_count, find_close] {
            gtk_box_append(find_bar as *mut GtkBox, w);
        }
        gtk_box_append(root as *mut GtkBox, find_bar);

        let notebook = gtk_notebook_new();
        gtk_notebook_set_scrollable(notebook as *mut GtkNotebook, 1);
        gtk_widget_set_hexpand(notebook, 1);
        gtk_widget_set_vexpand(notebook, 1);
        gtk_box_append(root as *mut GtkBox, notebook);
        let status = gtk_label_new(c("Ready").as_ptr());
        gtk_widget_add_css_class(status, c("status").as_ptr());
        gtk_label_set_xalign(status as *mut GtkLabel, 0.0);
        gtk_label_set_selectable(status as *mut GtkLabel, 1);
        gtk_widget_set_margin_start(status, 10);
        gtk_widget_set_margin_end(status, 10);
        gtk_widget_set_margin_bottom(status, 8);
        gtk_box_append(root as *mut GtkBox, status);

        let controller = gtk_event_controller_key_new();
        // Capture phase: the document list view would otherwise consume Space and similar keys first.
        gtk_event_controller_set_propagation_phase(controller, GTK_PHASE_CAPTURE);
        gtk_widget_add_controller(window, controller);
        let widgets = AppWidgets { window, notebook, entry, status, find_bar, find_entry, find_count };
        let app = Box::new(App::new(widgets, main_loop, verbose, stats));
        let app_ptr = Box::into_raw(app);
        (*app_ptr).dark = dark;
        (*app_ptr).dark_chosen = remembered;
        if let Some(state) = restore_state {
            (*app_ptr).sidebar_visible = state.sidebar_visible;
            (*app_ptr).tree_bookmarks_open = state.tree_bookmarks_open;
            (*app_ptr).tree_contents_open = state.tree_contents_open;
            (*app_ptr).tree_links_open = state.tree_links_open;
            if state.tabs.is_empty() {
                (*app_ptr).add_tab();
            } else {
                for st in state.tabs {
                    (*app_ptr).add_tab();
                    let tab = (*app_ptr).active;
                    (&mut (*app_ptr).tabs)[tab].restore_scroll = Some(st.position);
                    (*app_ptr).navigate(tab, st.url, false, true);
                }
                (*app_ptr).active = state.active.min((*app_ptr).tabs.len() - 1);
                gtk_notebook_set_current_page(notebook as *mut GtkNotebook, (*app_ptr).active as i32);
                (*app_ptr).sync_active();
                if verbose {
                    eprintln!("[session:restore] {} tab(s), active={}", (*app_ptr).tabs.len(), (*app_ptr).active);
                }
            }
        } else if start_urls.is_empty() {
            (*app_ptr).add_tab();
        } else {
            for url in start_urls {
                (*app_ptr).add_tab();
                let tab = (*app_ptr).active;
                (*app_ptr).navigate(tab, url, false, true);
            }
        }
        connect(back, "clicked", on_back as *const c_void, app_ptr as *mut c_void, None);
        connect(forward, "clicked", on_forward as *const c_void, app_ptr as *mut c_void, None);
        connect(reload, "clicked", on_reload as *const c_void, app_ptr as *mut c_void, None);
        connect(sidebar, "clicked", on_sidebar as *const c_void, app_ptr as *mut c_void, None);
        connect(bookmark, "clicked", on_bookmark as *const c_void, app_ptr as *mut c_void, None);
        connect(bookmark_manager, "clicked", on_bookmark_manager as *const c_void, app_ptr as *mut c_void, None);
        connect(copy_section, "clicked", on_copy_section as *const c_void, app_ptr as *mut c_void, None);
        connect(about, "clicked", on_about as *const c_void, app_ptr as *mut c_void, None);
        connect(new_tab, "clicked", on_new_tab as *const c_void, app_ptr as *mut c_void, None);
        connect(close_tab, "clicked", on_close_tab as *const c_void, app_ptr as *mut c_void, None);
        connect(entry, "activate", on_entry_activate as *const c_void, app_ptr as *mut c_void, None);
        connect(find_entry, "changed", on_find_changed as *const c_void, app_ptr as *mut c_void, None);
        connect(find_entry, "activate", on_find_next as *const c_void, app_ptr as *mut c_void, None);
        connect(find_prev, "clicked", on_find_prev as *const c_void, app_ptr as *mut c_void, None);
        connect(find_next, "clicked", on_find_next as *const c_void, app_ptr as *mut c_void, None);
        connect(find_close, "clicked", on_find_close as *const c_void, app_ptr as *mut c_void, None);
        connect(notebook, "switch-page", on_switch_page as *const c_void, app_ptr as *mut c_void, None);
        connect(notebook, "page-reordered", on_page_reordered as *const c_void, app_ptr as *mut c_void, None);
        connect(window, "close-request", on_close_request as *const c_void, app_ptr as *mut c_void, None);
        connect(controller as *mut GtkWidget, "key-pressed", on_key as *const c_void, app_ptr as *mut c_void, None);
        g_timeout_add(40, on_tick, app_ptr as *mut c_void);

        gtk_window_present(window as *mut GtkWindow);
        g_main_loop_run(main_loop);
        while let Some(a) = (*app_ptr).animations.pop() {
            g_object_unref(a.picture as *mut c_void);
            g_object_unref(a.iter as *mut c_void);
            g_object_unref(a.loader as *mut c_void);
        }
        let _ = Box::from_raw(app_ptr);
        g_main_loop_unref(main_loop);
    }
}

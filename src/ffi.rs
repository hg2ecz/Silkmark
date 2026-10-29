#![allow(non_camel_case_types)]
use std::ffi::{c_char, c_double, c_int, c_long, c_uint, c_void};

pub type gboolean = c_int;
pub type guint = c_uint;
pub type gpointer = *mut c_void;

pub enum GtkWidget {}
pub enum GtkWindow {}
pub enum GtkRoot {}
pub enum GtkBox {}
pub enum GtkButton {}
pub enum GtkEntry {}
pub enum GtkEditable {}
pub enum GtkNotebook {}
pub enum GtkLabel {}
pub enum GtkGrid {}
pub enum GtkScrolledWindow {}
pub enum GtkAdjustment {}
pub enum GtkPaned {}
pub enum GtkEventController {}
pub enum GtkEventControllerKey {}
pub enum GtkGestureClick {}
pub enum GtkDrawingArea {}
pub enum GtkCssProvider {}
pub enum GtkPicture {}
pub enum GtkListView {}
pub enum GtkListItem {}
pub enum GtkListItemFactory {}
pub enum GtkNoSelection {}
pub enum GtkStringList {}
pub enum GListModel {}
pub enum GdkPixbufLoader {}
pub enum GdkPixbuf {}
pub enum GdkPixbufAnimation {}
pub enum GdkPixbufAnimationIter {}
pub enum GdkTexture {}
pub enum GdkPaintable {}
pub enum GError {}
pub enum GdkDisplay {}
pub enum GdkClipboard {}
pub enum GMainLoop {}
pub enum cairo_t {}

#[repr(C)]
pub struct CairoTextExtents {
    pub x_bearing: c_double,
    pub y_bearing: c_double,
    pub width: c_double,
    pub height: c_double,
    pub x_advance: c_double,
    pub y_advance: c_double,
}

#[repr(C)]
pub struct GTimeVal {
    pub tv_sec: c_long,
    pub tv_usec: c_long,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GdkRGBA {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct GraphenePoint {
    pub x: f32,
    pub y: f32,
}

pub const GTK_ORIENTATION_HORIZONTAL: c_int = 0;
pub const GTK_ORIENTATION_VERTICAL: c_int = 1;
pub const GTK_PHASE_CAPTURE: c_int = 1;
pub const GTK_ALIGN_START: c_int = 1;
pub const PANGO_WRAP_WORD_CHAR: c_int = 2;
pub const GTK_POLICY_AUTOMATIC: c_int = 1;
pub const GTK_POLICY_NEVER: c_int = 2;
pub const GTK_STYLE_PROVIDER_PRIORITY_APPLICATION: c_uint = 600;
pub const GDK_SHIFT_MASK: c_uint = 1 << 0;
pub const GDK_CONTROL_MASK: c_uint = 1 << 2;
pub const GDK_ALT_MASK: c_uint = 1 << 3;
pub const GDK_KEY_LEFT: c_uint = 0xff51;
pub const GDK_KEY_RIGHT: c_uint = 0xff53;

#[link(name = "gtk-4")]
unsafe extern "C" {
    pub fn gtk_init();
    pub fn gtk_window_new() -> *mut GtkWidget;
    pub fn gtk_window_set_title(window: *mut GtkWindow, title: *const c_char);
    pub fn gtk_window_set_default_size(window: *mut GtkWindow, width: c_int, height: c_int);
    pub fn gtk_window_set_child(window: *mut GtkWindow, child: *mut GtkWidget);
    pub fn gtk_window_present(window: *mut GtkWindow);
    pub fn gtk_window_destroy(window: *mut GtkWindow);
    pub fn gtk_window_get_focus(window: *mut GtkWindow) -> *mut GtkWidget;
    pub fn gtk_window_set_focus(window: *mut GtkWindow, focus: *mut GtkWidget);

    pub fn gtk_box_new(orientation: c_int, spacing: c_int) -> *mut GtkWidget;
    pub fn gtk_box_append(box_: *mut GtkBox, child: *mut GtkWidget);
    pub fn gtk_box_remove(box_: *mut GtkBox, child: *mut GtkWidget);

    pub fn gtk_button_new() -> *mut GtkWidget;
    pub fn gtk_button_new_with_label(label: *const c_char) -> *mut GtkWidget;
    pub fn gtk_button_set_child(button: *mut GtkButton, child: *mut GtkWidget);

    pub fn gtk_entry_new() -> *mut GtkWidget;
    pub fn gtk_editable_get_text(editable: *mut GtkEditable) -> *const c_char;
    pub fn gtk_editable_set_text(editable: *mut GtkEditable, text: *const c_char);
    pub fn gtk_editable_select_region(editable: *mut GtkEditable, start_pos: c_int, end_pos: c_int);

    pub fn gtk_notebook_new() -> *mut GtkWidget;
    pub fn gtk_notebook_append_page(notebook: *mut GtkNotebook, child: *mut GtkWidget, tab_label: *mut GtkWidget) -> c_int;
    pub fn gtk_notebook_set_current_page(notebook: *mut GtkNotebook, page_num: c_int);
    pub fn gtk_notebook_remove_page(notebook: *mut GtkNotebook, page_num: c_int);
    pub fn gtk_notebook_set_scrollable(notebook: *mut GtkNotebook, scrollable: gboolean);
    pub fn gtk_notebook_set_tab_reorderable(notebook: *mut GtkNotebook, child: *mut GtkWidget, reorderable: gboolean);

    pub fn gtk_label_new(text: *const c_char) -> *mut GtkWidget;
    pub fn gtk_label_set_text(label: *mut GtkLabel, text: *const c_char);
    pub fn gtk_label_set_markup(label: *mut GtkLabel, markup: *const c_char);
    pub fn gtk_label_set_wrap(label: *mut GtkLabel, wrap: gboolean);
    pub fn gtk_label_set_xalign(label: *mut GtkLabel, xalign: f32);
    pub fn gtk_label_set_selectable(label: *mut GtkLabel, selectable: gboolean);
    pub fn gtk_label_set_width_chars(label: *mut GtkLabel, n_chars: c_int);
    pub fn gtk_label_set_max_width_chars(label: *mut GtkLabel, n_chars: c_int);

    pub fn gtk_grid_new() -> *mut GtkWidget;
    pub fn gtk_grid_attach(grid: *mut GtkGrid, child: *mut GtkWidget, column: c_int, row: c_int, width: c_int, height: c_int);
    pub fn gtk_grid_set_row_spacing(grid: *mut GtkGrid, spacing: c_uint);
    pub fn gtk_grid_set_column_spacing(grid: *mut GtkGrid, spacing: c_uint);

    pub fn gtk_picture_new() -> *mut GtkWidget;
    pub fn gtk_picture_set_paintable(picture: *mut GtkPicture, paintable: *mut GdkPaintable);
    pub fn gtk_picture_set_can_shrink(picture: *mut GtkPicture, can_shrink: gboolean);
    pub fn gtk_picture_set_keep_aspect_ratio(picture: *mut GtkPicture, keep_aspect_ratio: gboolean);
    pub fn gtk_picture_set_alternative_text(picture: *mut GtkPicture, alternative_text: *const c_char);

    pub fn gtk_scrolled_window_new() -> *mut GtkWidget;
    pub fn gtk_scrolled_window_set_child(scrolled: *mut GtkScrolledWindow, child: *mut GtkWidget);
    pub fn gtk_scrolled_window_set_policy(scrolled: *mut GtkScrolledWindow, hscrollbar_policy: c_int, vscrollbar_policy: c_int);
    pub fn gtk_scrolled_window_get_vadjustment(scrolled: *mut GtkScrolledWindow) -> *mut GtkAdjustment;
    pub fn gtk_adjustment_get_value(adjustment: *mut GtkAdjustment) -> c_double;
    pub fn gtk_adjustment_set_value(adjustment: *mut GtkAdjustment, value: c_double);
    pub fn gtk_adjustment_get_upper(adjustment: *mut GtkAdjustment) -> c_double;
    pub fn gtk_adjustment_get_page_size(adjustment: *mut GtkAdjustment) -> c_double;

    pub fn gtk_paned_new(orientation: c_int) -> *mut GtkWidget;
    pub fn gtk_paned_set_start_child(paned: *mut GtkPaned, child: *mut GtkWidget);
    pub fn gtk_paned_set_end_child(paned: *mut GtkPaned, child: *mut GtkWidget);
    pub fn gtk_paned_set_position(paned: *mut GtkPaned, position: c_int);

    pub fn gtk_drawing_area_new() -> *mut GtkWidget;
    pub fn gtk_drawing_area_set_draw_func(
        area: *mut GtkDrawingArea,
        draw_func: Option<unsafe extern "C" fn(*mut GtkDrawingArea, *mut cairo_t, c_int, c_int, gpointer)>,
        user_data: gpointer,
        destroy: Option<unsafe extern "C" fn(gpointer)>,
    );

    pub fn gtk_css_provider_new() -> *mut GtkCssProvider;
    pub fn gtk_css_provider_load_from_data(provider: *mut GtkCssProvider, data: *const c_char, length: isize);
    pub fn gtk_style_context_add_provider_for_display(display: *mut GdkDisplay, provider: *mut c_void, priority: c_uint);

    pub fn gtk_widget_set_hexpand(widget: *mut GtkWidget, expand: gboolean);
    pub fn gtk_widget_set_vexpand(widget: *mut GtkWidget, expand: gboolean);
    pub fn gtk_widget_set_size_request(widget: *mut GtkWidget, width: c_int, height: c_int);
    pub fn gtk_widget_queue_draw(widget: *mut GtkWidget);
    pub fn gtk_widget_set_margin_start(widget: *mut GtkWidget, margin: c_int);
    pub fn gtk_widget_set_margin_end(widget: *mut GtkWidget, margin: c_int);
    pub fn gtk_widget_set_margin_top(widget: *mut GtkWidget, margin: c_int);
    pub fn gtk_widget_set_margin_bottom(widget: *mut GtkWidget, margin: c_int);
    pub fn gtk_widget_set_visible(widget: *mut GtkWidget, visible: gboolean);
    pub fn gtk_widget_get_first_child(widget: *mut GtkWidget) -> *mut GtkWidget;
    pub fn gtk_widget_get_parent(widget: *mut GtkWidget) -> *mut GtkWidget;
    pub fn gtk_widget_get_root(widget: *mut GtkWidget) -> *mut GtkRoot;
    pub fn gtk_widget_get_width(widget: *mut GtkWidget) -> c_int;
    pub fn gtk_widget_get_height(widget: *mut GtkWidget) -> c_int;
    pub fn gtk_widget_has_focus(widget: *mut GtkWidget) -> gboolean;
    pub fn gtk_widget_grab_focus(widget: *mut GtkWidget) -> gboolean;
    pub fn gtk_widget_add_controller(widget: *mut GtkWidget, controller: *mut GtkEventController);
    pub fn gtk_widget_add_css_class(widget: *mut GtkWidget, css_class: *const c_char);
    pub fn gtk_widget_remove_css_class(widget: *mut GtkWidget, css_class: *const c_char);
    pub fn gtk_widget_set_tooltip_text(widget: *mut GtkWidget, text: *const c_char);

    pub fn gtk_event_controller_key_new() -> *mut GtkEventController;
    pub fn gtk_event_controller_set_propagation_phase(controller: *mut GtkEventController, phase: c_int);
    pub fn gtk_gesture_click_new() -> *mut GtkGestureClick;
    pub fn gdk_display_get_default() -> *mut GdkDisplay;
    pub fn gdk_display_get_clipboard(display: *mut GdkDisplay) -> *mut GdkClipboard;
    pub fn gdk_clipboard_set_text(clipboard: *mut GdkClipboard, text: *const c_char);
}

#[link(name = "gobject-2.0")]
unsafe extern "C" {
    pub fn g_signal_connect_data(
        instance: gpointer,
        detailed_signal: *const c_char,
        c_handler: *const c_void,
        data: gpointer,
        destroy_data: Option<unsafe extern "C" fn(gpointer, gpointer)>,
        connect_flags: c_uint,
    ) -> u64;
    pub fn g_object_unref(object: gpointer);
    pub fn g_object_ref(object: gpointer) -> gpointer;
    pub fn g_object_ref_sink(object: gpointer) -> gpointer;
}

#[link(name = "glib-2.0")]
unsafe extern "C" {
    pub fn g_timeout_add(interval: guint, function: unsafe extern "C" fn(gpointer) -> gboolean, data: gpointer) -> guint;
    pub fn g_main_loop_new(context: gpointer, is_running: gboolean) -> *mut GMainLoop;
    pub fn g_main_loop_run(loop_: *mut GMainLoop);
    pub fn g_main_loop_quit(loop_: *mut GMainLoop);
    pub fn g_main_loop_unref(loop_: *mut GMainLoop);
    pub fn g_get_real_time() -> i64;
}

#[link(name = "cairo")]
unsafe extern "C" {
    pub fn cairo_set_line_width(cr: *mut cairo_t, width: c_double);
    pub fn cairo_set_source_rgba(cr: *mut cairo_t, r: c_double, g: c_double, b: c_double, a: c_double);
    pub fn cairo_move_to(cr: *mut cairo_t, x: c_double, y: c_double);
    pub fn cairo_line_to(cr: *mut cairo_t, x: c_double, y: c_double);
    pub fn cairo_rectangle(cr: *mut cairo_t, x: c_double, y: c_double, width: c_double, height: c_double);
    pub fn cairo_arc(cr: *mut cairo_t, xc: c_double, yc: c_double, radius: c_double, angle1: c_double, angle2: c_double);
    pub fn cairo_stroke(cr: *mut cairo_t);
    pub fn cairo_fill_preserve(cr: *mut cairo_t);
    pub fn cairo_close_path(cr: *mut cairo_t);
    pub fn cairo_save(cr: *mut cairo_t);
    pub fn cairo_restore(cr: *mut cairo_t);
    pub fn cairo_translate(cr: *mut cairo_t, tx: c_double, ty: c_double);
    pub fn cairo_scale(cr: *mut cairo_t, sx: c_double, sy: c_double);
    pub fn cairo_select_font_face(cr: *mut cairo_t, family: *const c_char, slant: c_int, weight: c_int);
    pub fn cairo_set_font_size(cr: *mut cairo_t, size: c_double);
    pub fn cairo_show_text(cr: *mut cairo_t, utf8: *const c_char);
    pub fn cairo_text_extents(cr: *mut cairo_t, utf8: *const c_char, extents: *mut CairoTextExtents);
}

#[link(name = "gtk-4")]
unsafe extern "C" {
    pub fn gtk_list_view_new(model: *mut GtkNoSelection, factory: *mut GtkListItemFactory) -> *mut GtkWidget;
    pub fn gtk_list_view_set_model(list_view: *mut GtkListView, model: *mut GtkNoSelection);
    pub fn gtk_signal_list_item_factory_new() -> *mut GtkListItemFactory;
    pub fn gtk_no_selection_new(model: *mut GListModel) -> *mut GtkNoSelection;
    pub fn gtk_string_list_new(strings: *const *const c_char) -> *mut GtkStringList;
    pub fn gtk_list_item_set_child(item: *mut GtkListItem, child: *mut GtkWidget);
    pub fn gtk_list_item_get_position(item: *mut GtkListItem) -> guint;
    pub fn gtk_list_item_set_activatable(item: *mut GtkListItem, activatable: gboolean);
    pub fn gtk_button_get_child(button: *mut GtkButton) -> *mut GtkWidget;
    pub fn gtk_label_set_wrap_mode(label: *mut GtkLabel, mode: c_int);
    pub fn gtk_widget_set_halign(widget: *mut GtkWidget, align: c_int);
    pub fn gtk_list_item_set_selectable(item: *mut GtkListItem, selectable: gboolean);
    pub fn gtk_widget_pick(widget: *mut GtkWidget, x: c_double, y: c_double, flags: c_int) -> *mut GtkWidget;
    pub fn gtk_widget_measure(
        widget: *mut GtkWidget,
        orientation: c_int,
        for_size: c_int,
        minimum: *mut c_int,
        natural: *mut c_int,
        minimum_baseline: *mut c_int,
        natural_baseline: *mut c_int,
    );
    pub fn gtk_widget_get_color(widget: *mut GtkWidget, color: *mut GdkRGBA);
    pub fn gtk_widget_compute_point(
        widget: *mut GtkWidget,
        target: *mut GtkWidget,
        point: *const GraphenePoint,
        out_point: *mut GraphenePoint,
    ) -> gboolean;
}

#[link(name = "gtk-4")]
unsafe extern "C" {
    pub fn gdk_texture_new_for_pixbuf(pixbuf: *mut GdkPixbuf) -> *mut GdkTexture;
}

#[link(name = "gdk_pixbuf-2.0")]
unsafe extern "C" {
    pub fn gdk_pixbuf_loader_new() -> *mut GdkPixbufLoader;
    pub fn gdk_pixbuf_loader_write(
        loader: *mut GdkPixbufLoader,
        buf: *const u8,
        count: usize,
        error: *mut *mut GError,
    ) -> gboolean;
    pub fn gdk_pixbuf_loader_close(loader: *mut GdkPixbufLoader, error: *mut *mut GError) -> gboolean;
    pub fn gdk_pixbuf_loader_get_pixbuf(loader: *mut GdkPixbufLoader) -> *mut GdkPixbuf;
    pub fn gdk_pixbuf_loader_get_animation(loader: *mut GdkPixbufLoader) -> *mut GdkPixbufAnimation;
    pub fn gdk_pixbuf_animation_is_static_image(animation: *mut GdkPixbufAnimation) -> gboolean;
    pub fn gdk_pixbuf_animation_get_iter(
        animation: *mut GdkPixbufAnimation,
        start_time: *const GTimeVal,
    ) -> *mut GdkPixbufAnimationIter;
    pub fn gdk_pixbuf_animation_iter_advance(iter: *mut GdkPixbufAnimationIter, current_time: *const GTimeVal) -> gboolean;
    pub fn gdk_pixbuf_animation_iter_get_pixbuf(iter: *mut GdkPixbufAnimationIter) -> *mut GdkPixbuf;
    pub fn gdk_pixbuf_get_width(pixbuf: *const GdkPixbuf) -> c_int;
    pub fn gdk_pixbuf_get_height(pixbuf: *const GdkPixbuf) -> c_int;
    pub fn gdk_pixbuf_scale_simple(
        src: *const GdkPixbuf,
        dest_width: c_int,
        dest_height: c_int,
        interp_type: c_int,
    ) -> *mut GdkPixbuf;
}

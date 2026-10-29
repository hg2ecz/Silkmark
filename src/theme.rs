//! Light and dark appearance.
//!
//! The theme name stays the built-in Adwaita (some desktop themes are GTK3-only and make GTK4
//! print parser warnings); only its dark variant is switched, through GtkSettings.
use std::ffi::{c_char, c_int, c_void};
use std::ptr;

pub enum GtkSettings {}

#[link(name = "gtk-4")]
unsafe extern "C" {
    fn gtk_settings_get_default() -> *mut GtkSettings;
}

#[link(name = "gobject-2.0")]
unsafe extern "C" {
    fn g_object_set(object: *mut c_void, first_property_name: *const c_char, ...);
    fn g_object_get(object: *mut c_void, first_property_name: *const c_char, ...);
    fn g_object_class_find_property(class: *mut c_void, name: *const c_char) -> *mut c_void;
}

const PREFER_DARK: &std::ffi::CStr = c"gtk-application-prefer-dark-theme";
const COLOR_SCHEME: &std::ffi::CStr = c"gtk-interface-color-scheme";
const THEME_NAME: &std::ffi::CStr = c"gtk-theme-name";
/// `GtkInterfaceColorScheme::GTK_INTERFACE_COLOR_SCHEME_DARK` (GTK 4.20 and later).
const COLOR_SCHEME_DARK: c_int = 2;

/// Whether the desktop asks for a dark appearance. Call after `gtk_init`.
pub unsafe fn system_prefers_dark() -> bool {
    let settings = gtk_settings_get_default() as *mut c_void;
    if settings.is_null() {
        return false;
    }
    let mut prefer: c_int = 0;
    g_object_get(settings, PREFER_DARK.as_ptr(), &mut prefer as *mut c_int, ptr::null::<c_char>());
    // Newer GTK also reports the desktop's color scheme; the property does not exist before 4.20.
    let class = *(settings as *const *mut c_void);
    let mut scheme: c_int = 0;
    if !g_object_class_find_property(class, COLOR_SCHEME.as_ptr()).is_null() {
        g_object_get(settings, COLOR_SCHEME.as_ptr(), &mut scheme as *mut c_int, ptr::null::<c_char>());
    }
    prefer != 0 || scheme == COLOR_SCHEME_DARK
}

/// Applies the built-in theme in its light or dark variant.
pub unsafe fn apply(dark: bool) {
    let settings = gtk_settings_get_default() as *mut c_void;
    if settings.is_null() {
        return;
    }
    if std::env::var_os("GTK_THEME").is_none() {
        g_object_set(settings, THEME_NAME.as_ptr(), c"Adwaita".as_ptr(), ptr::null::<c_char>());
    }
    g_object_set(settings, PREFER_DARK.as_ptr(), dark as c_int, ptr::null::<c_char>());
}

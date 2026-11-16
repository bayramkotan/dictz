//! Hiding the main window to the tray and bringing it back.
//!
//! The window must keep receiving frames while it is "hidden", because the
//! scan popup is drawn from the main window's update (egui viewports):
//! * Windows: hidden windows get no WM_PAINT, so the window is moved off
//!   screen and taken off the taskbar (tool-window style) instead.
//! * Linux X11: an unmapped window keeps getting frames, so it is hidden.
//! * Wayland and macOS: windows cannot hide themselves reliably; they are
//!   minimized.

use eframe::egui::{self, ViewportCommand};

/// Main-window visibility helper.
#[derive(Default)]
pub struct MainWindow {
    hidden: bool,
    #[cfg(windows)]
    hwnd: isize,
    #[cfg(windows)]
    saved: Option<(i32, i32, isize)>,
}

#[cfg(target_os = "linux")]
fn wayland() -> bool {
    // winit picks Wayland whenever WAYLAND_DISPLAY is set (unless told otherwise)
    std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var("WINIT_UNIX_BACKEND")
            .map(|v| v != "x11")
            .unwrap_or(true)
}

impl MainWindow {
    /// Remember the native window (Windows needs its handle).
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let _ = cc;
        #[allow(unused_mut)]
        let mut w = Self::default();
        #[cfg(windows)]
        {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(h) = cc.window_handle() {
                if let RawWindowHandle::Win32(h) = h.as_raw() {
                    w.hwnd = h.hwnd.get();
                }
            }
        }
        w
    }

    /// `true` while hidden / minimized to the tray.
    pub fn is_hidden(&self) -> bool {
        self.hidden
    }

    /// Hide to the tray.
    pub fn hide(&mut self, ctx: &egui::Context) {
        if self.hidden {
            return;
        }
        self.hidden = true;
        #[cfg(windows)]
        {
            if self.hwnd != 0 {
                self.saved = win::hide(self.hwnd);
                return;
            }
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }
        #[cfg(target_os = "linux")]
        {
            if wayland() {
                ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
            } else {
                ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            }
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
    }

    /// Bring back and focus.
    pub fn show(&mut self, ctx: &egui::Context) {
        let was_hidden = self.hidden;
        self.hidden = false;
        #[cfg(windows)]
        {
            if self.hwnd != 0 {
                if was_hidden {
                    if let Some(s) = self.saved.take() {
                        win::show(self.hwnd, s);
                    }
                }
                win::raise(self.hwnd);
            }
        }
        #[cfg(target_os = "linux")]
        {
            if was_hidden && !wayland() {
                ctx.send_viewport_cmd(ViewportCommand::Visible(true));
            }
        }
        let _ = was_hidden;
        ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod win {
    use windows_sys::Win32::Foundation::{HWND, RECT};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, GetWindowRect, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos,
        ShowWindow, GWL_EXSTYLE, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SW_HIDE, SW_RESTORE,
        SW_SHOW, SW_SHOWNOACTIVATE, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    };

    fn h(hwnd: isize) -> HWND {
        hwnd as HWND
    }

    /// Off screen + tool window (no taskbar button, still painted).
    pub fn hide(hwnd: isize) -> Option<(i32, i32, isize)> {
        // SAFETY: plain Win32 calls on our own top-level window handle.
        unsafe {
            let mut r: RECT = std::mem::zeroed();
            GetWindowRect(h(hwnd), &mut r);
            let ex = GetWindowLongPtrW(h(hwnd), GWL_EXSTYLE);
            ShowWindow(h(hwnd), SW_HIDE);
            SetWindowLongPtrW(
                h(hwnd),
                GWL_EXSTYLE,
                (ex | WS_EX_TOOLWINDOW as isize) & !(WS_EX_APPWINDOW as isize),
            );
            SetWindowPos(
                h(hwnd),
                std::ptr::null_mut(),
                -32000,
                -32000,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            ShowWindow(h(hwnd), SW_SHOWNOACTIVATE);
            Some((r.left, r.top, ex))
        }
    }

    pub fn show(hwnd: isize, (x, y, ex): (i32, i32, isize)) {
        // SAFETY: see `hide`.
        unsafe {
            ShowWindow(h(hwnd), SW_HIDE);
            SetWindowLongPtrW(h(hwnd), GWL_EXSTYLE, ex);
            SetWindowPos(
                h(hwnd),
                std::ptr::null_mut(),
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER,
            );
            ShowWindow(h(hwnd), SW_SHOW);
        }
    }

    pub fn raise(hwnd: isize) {
        // SAFETY: see `hide`.
        unsafe {
            ShowWindow(h(hwnd), SW_RESTORE);
            SetForegroundWindow(h(hwnd));
        }
    }
}

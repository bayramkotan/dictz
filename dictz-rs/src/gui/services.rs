//! Background services of the window: the instance server (`dictz show` /
//! `popup` / `watch`, single instance), the clipboard / selection watcher for
//! the scan popup, global hotkeys and the tray icon. Everything reports to
//! the window through one channel and wakes it up with `request_repaint`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use eframe::egui;
use serde_json::{json, Value};

use crate::instance::{Handler, Server};
use crate::VERSION;

/// Tray menu entries (which ones are used depends on the platform).
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    /// Show / hide the main window.
    Toggle,
    /// Scanning on / off.
    Scan(bool),
    /// Flip scanning.
    ToggleScan,
    /// Look the clipboard text up in the popup.
    LookupClipboard,
    /// Open the preferences.
    Preferences,
    /// Quit dictz.
    Quit,
}

/// Something happened outside the window.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Event {
    /// The clipboard changed (raw text, when).
    Clipboard(String, std::time::Instant),
    /// The X11 / Wayland primary selection changed (raw text).
    Selection(String),
    /// A global hotkey: "show_main", "toggle_scan" or "lookup_clipboard".
    Hotkey(&'static str),
    /// Tray icon / menu.
    Tray(TrayAction),
    /// A request from `dictz show` / `popup` / `watch` / a second start.
    Remote(Value),
}

/// The running services.
pub struct Services {
    tx: Sender<Event>,
    rx: Receiver<Event>,
    ctx: egui::Context,
    server: Option<Server>,
    /// Shared with the instance server (answers `dictz watch status`).
    pub scan_on: Arc<AtomicBool>,
    selection_on: Arc<AtomicBool>,
    #[cfg(feature = "scan")]
    clipboard_started: bool,
    #[cfg(feature = "scan")]
    hotkeys: Option<hotkeys::Hotkeys>,
    #[cfg(feature = "scan")]
    tray: Option<tray::Tray>,
    /// Errors to show in the status bar.
    pub messages: Vec<String>,
}

impl Services {
    /// Create; nothing runs until the `start_*` calls.
    pub fn new(ctx: &egui::Context, scan_on: bool) -> Self {
        let (tx, rx) = channel();
        Self {
            tx,
            rx,
            ctx: ctx.clone(),
            server: None,
            scan_on: Arc::new(AtomicBool::new(scan_on)),
            selection_on: Arc::new(AtomicBool::new(false)),
            #[cfg(feature = "scan")]
            clipboard_started: false,
            #[cfg(feature = "scan")]
            hotkeys: None,
            #[cfg(feature = "scan")]
            tray: None,
            messages: Vec::new(),
        }
    }

    fn sender(&self) -> impl Fn(Event) + Send + Sync + Clone + 'static {
        let tx = std::sync::Mutex::new(self.tx.clone());
        let tx = Arc::new(tx);
        let ctx = self.ctx.clone();
        move |e: Event| {
            if let Ok(t) = tx.lock() {
                let _ = t.send(e);
            }
            ctx.request_repaint();
        }
    }

    /// Pending events.
    pub fn poll(&self) -> Vec<Event> {
        self.rx.try_iter().collect()
    }

    /// Start the instance server.
    pub fn start_server(&mut self, data_dir: &std::path::Path) {
        let send = self.sender();
        let scan_on = self.scan_on.clone();
        let handler: Handler = Arc::new(move |req: Value| {
            let cmd = req["cmd"].as_str().unwrap_or("").to_string();
            match cmd.as_str() {
                "ping" => json!({"ok": true, "edition": "rust", "version": VERSION}),
                "scan" => {
                    let state = req["state"].as_str().unwrap_or("status");
                    let cur = scan_on.load(Ordering::SeqCst);
                    let new = match state {
                        "on" => true,
                        "off" => false,
                        "toggle" => !cur,
                        "status" => cur,
                        other => {
                            return json!({"ok": false, "error": format!("unknown state: {}", other)})
                        }
                    };
                    if state != "status" {
                        scan_on.store(new, Ordering::SeqCst);
                        send(Event::Remote(json!({"cmd": "scan", "on": new})));
                    }
                    json!({"ok": true, "scan": new})
                }
                "show" | "hide" | "toggle" | "lookup" | "popup" | "open" | "quit" => {
                    send(Event::Remote(req.clone()));
                    json!({"ok": true})
                }
                other => json!({"ok": false, "error": format!("unknown command: {}", other)}),
            }
        });
        self.server = Server::start(data_dir, handler, "rust", VERSION);
    }

    /// Stop everything that leaves traces (instance file, hotkeys).
    pub fn stop(&mut self) {
        if let Some(s) = self.server.take() {
            s.stop();
        }
        #[cfg(feature = "scan")]
        {
            self.hotkeys = None;
            self.tray = None;
        }
    }

    /// Apply the scan / hotkey / tray settings.
    #[allow(unused_variables)]
    pub fn configure(&mut self, prefs: &crate::settings::Preferences) {
        let scan = prefs.bool("scan.enabled");
        self.scan_on.store(scan, Ordering::SeqCst);
        self.selection_on
            .store(scan && prefs.bool("scan.selection"), Ordering::SeqCst);
        #[cfg(feature = "scan")]
        {
            if scan && !self.clipboard_started {
                self.clipboard_started = true;
                clipboard::start(self.sender(), self.selection_on.clone());
            }
            let wanted = if prefs.bool("hotkeys.enabled") {
                vec![
                    ("show_main", prefs.str("hotkeys.show_main")),
                    ("toggle_scan", prefs.str("hotkeys.toggle_scan")),
                    ("lookup_clipboard", prefs.str("hotkeys.lookup_clipboard")),
                ]
            } else {
                Vec::new()
            };
            let same = self
                .hotkeys
                .as_ref()
                .map(|h| h.bindings == wanted)
                .unwrap_or(wanted.is_empty());
            if !same {
                self.hotkeys = None;
                if !wanted.is_empty() {
                    let (h, errors) = hotkeys::Hotkeys::new(wanted, self.sender());
                    self.hotkeys = h;
                    if !errors.is_empty() {
                        self.messages.push(format!(
                            "{} {}",
                            crate::i18n::tr("Genel kısayollar:"),
                            errors.join("; ")
                        ));
                    }
                }
            }
            // the tray is always there when dictz starts with the system
            if prefs.bool("general.tray_icon") || prefs.bool("general.autostart") {
                if self.tray.is_none() {
                    match tray::Tray::new(self.sender(), scan) {
                        Ok(t) => self.tray = Some(t),
                        Err(e) => self.messages.push(format!(
                            "{} {}",
                            crate::i18n::tr("Tepsi simgesi gösterilemedi:"),
                            e
                        )),
                    }
                }
                if let Some(t) = &self.tray {
                    t.set_scan(scan);
                }
            } else {
                self.tray = None;
            }
        }
    }

    /// `true` when a tray icon is shown (closing / starting hidden needs one).
    pub fn has_tray(&self) -> bool {
        #[cfg(feature = "scan")]
        {
            self.tray.is_some()
        }
        #[cfg(not(feature = "scan"))]
        {
            false
        }
    }

    /// Read the clipboard text now.
    pub fn clipboard_text() -> Option<String> {
        #[cfg(feature = "scan")]
        {
            arboard::Clipboard::new().ok()?.get_text().ok()
        }
        #[cfg(not(feature = "scan"))]
        {
            None
        }
    }

    /// Global mouse position in physical pixels, if the platform tells.
    pub fn mouse_position() -> Option<(i32, i32)> {
        #[cfg(feature = "scan")]
        {
            match mouse_position::mouse_position::Mouse::get_mouse_position() {
                mouse_position::mouse_position::Mouse::Position { x, y } => Some((x, y)),
                mouse_position::mouse_position::Mouse::Error => None,
            }
        }
        #[cfg(not(feature = "scan"))]
        {
            None
        }
    }
}

impl Drop for Services {
    fn drop(&mut self) {
        self.stop();
    }
}

// ---------------------------------------------------------------- clipboard
#[cfg(feature = "scan")]
mod clipboard {
    use super::Event;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    struct Handler<F: Fn(Event)> {
        send: F,
        cb: Option<arboard::Clipboard>,
    }

    impl<F: Fn(Event)> clipboard_master::ClipboardHandler for Handler<F> {
        fn on_clipboard_change(&mut self) -> clipboard_master::CallbackResult {
            if self.cb.is_none() {
                self.cb = arboard::Clipboard::new().ok();
            }
            // the new owner may need a moment to publish the text
            std::thread::sleep(Duration::from_millis(30));
            if let Some(text) = self.cb.as_mut().and_then(|c| c.get_text().ok()) {
                (self.send)(Event::Clipboard(text, std::time::Instant::now()));
            }
            clipboard_master::CallbackResult::Next
        }

        fn on_clipboard_error(
            &mut self,
            _error: std::io::Error,
        ) -> clipboard_master::CallbackResult {
            clipboard_master::CallbackResult::Next
        }
    }

    /// Watch the clipboard (change notifications; polling when the platform
    /// has none) and, on Linux, the primary selection while `selection_on`.
    pub fn start(
        send: impl Fn(Event) + Send + Sync + Clone + 'static,
        selection_on: Arc<AtomicBool>,
    ) {
        let s = send.clone();
        let _ = std::thread::Builder::new()
            .name("dictz-clipboard".into())
            .spawn(move || {
                #[cfg(target_os = "linux")]
                if std::env::var_os("DISPLAY").is_some() && x11_watch(&s).is_ok() {
                    return;
                }
                let handler = Handler {
                    send: s.clone(),
                    cb: None,
                };
                match clipboard_master::Master::new(handler) {
                    Ok(mut m) => {
                        if m.run().is_ok() {
                            return;
                        }
                        poll(s)
                    }
                    Err(_) => poll(s),
                }
            });
        #[cfg(target_os = "linux")]
        {
            let _ = std::thread::Builder::new()
                .name("dictz-selection".into())
                .spawn(move || {
                    use arboard::{GetExtLinux, LinuxClipboardKind};
                    let mut cb: Option<arboard::Clipboard> = None;
                    let mut last = String::new();
                    let mut pending: Option<(String, u8)> = None;
                    loop {
                        std::thread::sleep(Duration::from_millis(250));
                        if !selection_on.load(Ordering::SeqCst) {
                            pending = None;
                            continue;
                        }
                        if cb.is_none() {
                            cb = arboard::Clipboard::new().ok();
                        }
                        let Some(c) = cb.as_mut() else { continue };
                        let text = c
                            .get()
                            .clipboard(LinuxClipboardKind::Primary)
                            .text()
                            .unwrap_or_default();
                        if text.is_empty() || text == last {
                            pending = None;
                            continue;
                        }
                        // wait until the selection stops changing (mouse released)
                        match &mut pending {
                            Some((t, n)) if *t == text => {
                                *n += 1;
                                if *n >= 1 {
                                    last = text.clone();
                                    pending = None;
                                    send(Event::Selection(text));
                                }
                            }
                            _ => pending = Some((text, 0)),
                        }
                    }
                });
        }
        #[cfg(not(target_os = "linux"))]
        let _ = (selection_on, send);
    }

    /// X11: every SetSelectionOwner on CLIPBOARD is one copy (Ctrl+C+C needs
    /// each of them, even with the same text).
    #[cfg(target_os = "linux")]
    fn x11_watch(send: &impl Fn(Event)) -> Result<(), String> {
        use x11rb::connection::Connection;
        use x11rb::protocol::xfixes::{ConnectionExt as _, SelectionEventMask};
        use x11rb::protocol::xproto::ConnectionExt as _;
        use x11rb::protocol::Event as XEvent;
        let e = |x: &dyn std::fmt::Display| x.to_string();
        let (conn, screen) = x11rb::connect(None).map_err(|x| e(&x))?;
        conn.xfixes_query_version(5, 0)
            .map_err(|x| e(&x))?
            .reply()
            .map_err(|x| e(&x))?;
        let root = conn.setup().roots[screen].root;
        let clip = conn
            .intern_atom(false, b"CLIPBOARD")
            .map_err(|x| e(&x))?
            .reply()
            .map_err(|x| e(&x))?
            .atom;
        conn.xfixes_select_selection_input(root, clip, SelectionEventMask::SET_SELECTION_OWNER)
            .map_err(|x| e(&x))?;
        conn.flush().map_err(|x| e(&x))?;
        let mut cb: Option<arboard::Clipboard> = None;
        loop {
            let ev = conn.wait_for_event().map_err(|x| e(&x))?;
            if let XEvent::XfixesSelectionNotify(n) = ev {
                if n.selection != clip {
                    continue;
                }
                if cb.is_none() {
                    cb = arboard::Clipboard::new().ok();
                }
                // give the new owner a moment before asking for the text
                let mut text = None;
                for _ in 0..5 {
                    std::thread::sleep(Duration::from_millis(20));
                    text = cb.as_mut().and_then(|c| c.get_text().ok());
                    if text.is_some() {
                        break;
                    }
                }
                if let Some(t) = text {
                    send(Event::Clipboard(t, std::time::Instant::now()));
                }
            }
        }
    }

    fn poll(send: impl Fn(Event)) {
        let mut cb: Option<arboard::Clipboard> = None;
        let mut last: Option<String> = None;
        loop {
            std::thread::sleep(Duration::from_millis(300));
            if cb.is_none() {
                cb = arboard::Clipboard::new().ok();
            }
            let Some(c) = cb.as_mut() else { continue };
            let text = c.get_text().ok();
            if text.is_some() && text != last {
                if last.is_some() {
                    send(Event::Clipboard(
                        text.clone().unwrap_or_default(),
                        std::time::Instant::now(),
                    ));
                }
                last = text;
            } else if last.is_none() {
                last = Some(String::new());
            }
        }
    }
}

// ---------------------------------------------------------------- hotkeys
#[cfg(feature = "scan")]
mod hotkeys {
    use super::Event;
    use global_hotkey::hotkey::{Code, HotKey, Modifiers};
    use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    pub struct Hotkeys {
        pub bindings: Vec<(&'static str, String)>,
        manager: GlobalHotKeyManager,
        registered: Vec<HotKey>,
    }

    fn code(key: &str) -> Option<Code> {
        use Code::*;
        let c = match key {
            "a" => KeyA,
            "b" => KeyB,
            "c" => KeyC,
            "d" => KeyD,
            "e" => KeyE,
            "f" => KeyF,
            "g" => KeyG,
            "h" => KeyH,
            "i" => KeyI,
            "j" => KeyJ,
            "k" => KeyK,
            "l" => KeyL,
            "m" => KeyM,
            "n" => KeyN,
            "o" => KeyO,
            "p" => KeyP,
            "q" => KeyQ,
            "r" => KeyR,
            "s" => KeyS,
            "t" => KeyT,
            "u" => KeyU,
            "v" => KeyV,
            "w" => KeyW,
            "x" => KeyX,
            "y" => KeyY,
            "z" => KeyZ,
            "0" => Digit0,
            "1" => Digit1,
            "2" => Digit2,
            "3" => Digit3,
            "4" => Digit4,
            "5" => Digit5,
            "6" => Digit6,
            "7" => Digit7,
            "8" => Digit8,
            "9" => Digit9,
            "f1" => F1,
            "f2" => F2,
            "f3" => F3,
            "f4" => F4,
            "f5" => F5,
            "f6" => F6,
            "f7" => F7,
            "f8" => F8,
            "f9" => F9,
            "f10" => F10,
            "f11" => F11,
            "f12" => F12,
            "f13" => F13,
            "f14" => F14,
            "f15" => F15,
            "f16" => F16,
            "f17" => F17,
            "f18" => F18,
            "f19" => F19,
            "f20" => F20,
            "f21" => F21,
            "f22" => F22,
            "f23" => F23,
            "f24" => F24,
            "space" => Space,
            "tab" => Tab,
            "return" => Enter,
            "escape" => Escape,
            "insert" => Insert,
            "delete" => Delete,
            "home" => Home,
            "end" => End,
            "prior" => PageUp,
            "next" => PageDown,
            "up" => ArrowUp,
            "down" => ArrowDown,
            "left" => ArrowLeft,
            "right" => ArrowRight,
            "pause" => Pause,
            "print" => PrintScreen,
            "backspace" => Backspace,
            _ => return None,
        };
        Some(c)
    }

    fn ids() -> &'static Mutex<HashMap<u32, &'static str>> {
        static IDS: std::sync::OnceLock<Mutex<HashMap<u32, &'static str>>> =
            std::sync::OnceLock::new();
        IDS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    impl Hotkeys {
        pub fn new(
            bindings: Vec<(&'static str, String)>,
            send: impl Fn(Event) + Send + Sync + 'static,
        ) -> (Option<Self>, Vec<String>) {
            let mut errors = Vec::new();
            #[cfg(target_os = "linux")]
            if std::env::var_os("DISPLAY").is_none() {
                errors.push(
                    crate::i18n::tr("Wayland'da desteklenmiyor; masaüstü ayarlarında `dictz show` / `dictz popup` komutuna kısayol atayın")
                        .to_string(),
                );
                return (None, errors);
            }
            let manager = match GlobalHotKeyManager::new() {
                Ok(m) => m,
                Err(e) => {
                    errors.push(e.to_string());
                    return (None, errors);
                }
            };
            let send = Arc::new(send);
            GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
                if e.state == HotKeyState::Pressed {
                    if let Some(name) = ids().lock().ok().and_then(|m| m.get(&e.id).copied()) {
                        send(Event::Hotkey(name));
                    }
                }
            }));
            let mut registered = Vec::new();
            for (name, text) in &bindings {
                if text.trim().is_empty() {
                    continue;
                }
                let hk = match crate::hotkeys::parse_hotkey(text) {
                    Ok(h) => h,
                    Err(e) => {
                        errors.push(e);
                        continue;
                    }
                };
                let mut mods = Modifiers::empty();
                for m in &hk.mods {
                    mods |= match *m {
                        "ctrl" => Modifiers::CONTROL,
                        "alt" => Modifiers::ALT,
                        "shift" => Modifiers::SHIFT,
                        _ => Modifiers::SUPER,
                    };
                }
                let Some(c) = code(&hk.key) else {
                    errors.push(format!("{}: {}", hk, crate::i18n::tr("tuş desteklenmiyor")));
                    continue;
                };
                let h = HotKey::new(if mods.is_empty() { None } else { Some(mods) }, c);
                match manager.register(h) {
                    Ok(()) => {
                        if let Ok(mut m) = ids().lock() {
                            m.insert(h.id(), name);
                        }
                        registered.push(h);
                    }
                    Err(e) => errors.push(format!("{}: {}", hk, e)),
                }
            }
            (
                Some(Self {
                    bindings,
                    manager,
                    registered,
                }),
                errors,
            )
        }
    }

    impl Drop for Hotkeys {
        fn drop(&mut self) {
            let _ = self.manager.unregister_all(&self.registered);
            if let Ok(mut m) = ids().lock() {
                m.clear();
            }
        }
    }
}

// ---------------------------------------------------------------- tray
#[cfg(all(feature = "scan", target_os = "linux"))]
mod tray {
    use super::{Event, TrayAction};
    use ksni::blocking::TrayMethods;
    use std::sync::Arc;

    struct DictzTray {
        send: Arc<dyn Fn(Event) + Send + Sync>,
        scan: bool,
    }

    impl ksni::Tray for DictzTray {
        fn id(&self) -> String {
            "dictz".into()
        }
        fn title(&self) -> String {
            "dictz".into()
        }
        fn icon_pixmap(&self) -> Vec<ksni::Icon> {
            let rgba = crate::icon::app_icon_rgba(64);
            let mut argb = Vec::with_capacity(rgba.len());
            for px in rgba.chunks_exact(4) {
                argb.extend_from_slice(&[px[3], px[0], px[1], px[2]]);
            }
            vec![ksni::Icon {
                width: 64,
                height: 64,
                data: argb,
            }]
        }
        fn activate(&mut self, _x: i32, _y: i32) {
            (self.send)(Event::Tray(TrayAction::Toggle));
        }
        fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
            use ksni::menu::{CheckmarkItem, StandardItem};
            vec![
                StandardItem {
                    label: crate::i18n::tr("Göster / gizle"),
                    activate: Box::new(|t: &mut Self| (t.send)(Event::Tray(TrayAction::Toggle))),
                    ..Default::default()
                }
                .into(),
                CheckmarkItem {
                    label: crate::i18n::tr("Tarama"),
                    checked: self.scan,
                    activate: Box::new(|t: &mut Self| {
                        t.scan = !t.scan;
                        (t.send)(Event::Tray(TrayAction::Scan(t.scan)))
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: crate::i18n::tr("Panodaki kelimeyi ara"),
                    activate: Box::new(|t: &mut Self| {
                        (t.send)(Event::Tray(TrayAction::LookupClipboard))
                    }),
                    ..Default::default()
                }
                .into(),
                ksni::MenuItem::Separator,
                StandardItem {
                    label: crate::i18n::tr("Tercihler…"),
                    activate: Box::new(|t: &mut Self| {
                        (t.send)(Event::Tray(TrayAction::Preferences))
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: crate::i18n::tr("Çıkış"),
                    activate: Box::new(|t: &mut Self| (t.send)(Event::Tray(TrayAction::Quit))),
                    ..Default::default()
                }
                .into(),
            ]
        }
    }

    pub struct Tray {
        handle: ksni::blocking::Handle<DictzTray>,
    }

    impl Tray {
        pub fn new(
            send: impl Fn(Event) + Send + Sync + 'static,
            scan: bool,
        ) -> Result<Self, String> {
            let t = DictzTray {
                send: Arc::new(send),
                scan,
            };
            t.spawn()
                .map(|handle| Tray { handle })
                .map_err(|e| e.to_string())
        }

        pub fn set_scan(&self, on: bool) {
            self.handle.update(|t| t.scan = on);
        }
    }

    impl Drop for Tray {
        fn drop(&mut self) {
            self.handle.shutdown().wait();
        }
    }
}

#[cfg(all(feature = "scan", not(target_os = "linux")))]
mod tray {
    use super::{Event, TrayAction};
    use tray_icon::menu::{
        CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem,
    };
    use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

    pub struct Tray {
        _icon: TrayIcon,
        scan_item: CheckMenuItem,
    }

    impl Tray {
        pub fn new(
            send: impl Fn(Event) + Send + Sync + Clone + 'static,
            scan: bool,
        ) -> Result<Self, String> {
            let toggle = MenuItem::new(crate::i18n::tr("Göster / gizle"), true, None);
            let scan_item = CheckMenuItem::new(crate::i18n::tr("Tarama"), true, scan, None);
            let lookup = MenuItem::new(crate::i18n::tr("Panodaki kelimeyi ara"), true, None);
            let prefs = MenuItem::new(crate::i18n::tr("Tercihler…"), true, None);
            let quit = MenuItem::new(crate::i18n::tr("Çıkış"), true, None);
            let sep = PredefinedMenuItem::separator();
            let menu = Menu::new();
            let items: [&dyn IsMenuItem; 6] = [&toggle, &scan_item, &lookup, &sep, &prefs, &quit];
            menu.append_items(&items).map_err(|e| e.to_string())?;
            let ids = (
                toggle.id().clone(),
                scan_item.id().clone(),
                lookup.id().clone(),
                prefs.id().clone(),
                quit.id().clone(),
            );
            let s = send.clone();
            MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
                let a = if e.id == ids.0 {
                    TrayAction::Toggle
                } else if e.id == ids.1 {
                    TrayAction::ToggleScan
                } else if e.id == ids.2 {
                    TrayAction::LookupClipboard
                } else if e.id == ids.3 {
                    TrayAction::Preferences
                } else if e.id == ids.4 {
                    TrayAction::Quit
                } else {
                    return;
                };
                s(Event::Tray(a));
            }));
            TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = e
                {
                    send(Event::Tray(TrayAction::Toggle));
                }
            }));
            let icon = tray_icon::Icon::from_rgba(crate::icon::app_icon_rgba(64), 64, 64)
                .map_err(|e| e.to_string())?;
            let tray = TrayIconBuilder::new()
                .with_icon(icon)
                .with_tooltip(format!("dictz {}", crate::VERSION))
                .with_menu(Box::new(menu))
                .with_menu_on_left_click(false)
                .build()
                .map_err(|e| e.to_string())?;
            Ok(Tray {
                _icon: tray,
                scan_item,
            })
        }

        pub fn set_scan(&self, on: bool) {
            self.scan_item.set_checked(on);
        }
    }
}

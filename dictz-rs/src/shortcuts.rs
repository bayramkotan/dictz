//! Keyboard shortcuts — the same list as the Python edition
//! (`dictz/core/shortcuts.py`, checked by `tests/parity.rs`).

/// (keys, description) shown in Help → Shortcuts and Preferences.
pub const SHORTCUTS: &[(&str, &str)] = &[
    ("Ctrl+F", "Arama kutusuna odaklan"),
    ("Enter", "Ara (sonuç yoksa “Bunu mu demek istediniz?”)"),
    ("Ctrl+Enter", "Yeni sekmede ara"),
    ("Ctrl+T", "Yeni sekme"),
    ("Ctrl+W", "Sekmeyi kapat"),
    ("Ctrl+Tab / Ctrl+Shift+Tab", "Sonraki / önceki sekme"),
    ("Alt+← / Alt+→", "Geri / ileri"),
    ("Ctrl+R", "Rastgele kelime"),
    ("Ctrl+D", "Favorilere ekle / çıkar"),
    ("Ctrl+Shift+C", "Kelimeyi kopyala"),
    ("Ctrl+Shift+A", "Makaleyi metin olarak kopyala"),
    ("Ctrl+S", "Makaleyi HTML olarak kaydet"),
    ("Ctrl+Shift+F", "Sayfada bul"),
    ("F3 / Shift+F3", "Sonraki / önceki eşleşme"),
    ("Ctrl+= / Ctrl+- / Ctrl+0", "Büyüt / küçült / sıfırla"),
    ("Ctrl+B", "Yan paneli göster/gizle"),
    ("F11", "Tam ekran"),
    ("Ctrl+O", "Sözlük ekle"),
    ("Ctrl+Shift+O", "Klasör ekle"),
    ("Ctrl+M", "Sözlük yöneticisi"),
    ("Ctrl+,", "Tercihler"),
    ("Ctrl+Q", "Çıkış"),
];

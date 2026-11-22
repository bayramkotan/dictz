//! User preferences — `settings.json` in the data folder.
//!
//! The same file, keys, defaults and limits as the Python edition
//! (`dictz/storage/settings.py`); `tests/data/parity/settings_schema.json` is
//! checked by both test suites. Unknown keys are preserved on save, so the two
//! editions never wipe each other's values.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

/// File name inside the data folder.
pub const SETTINGS_FILE: &str = "settings.json";

/// Value type of a setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// true / false
    Bool,
    /// integer (clamped to min/max)
    Int,
    /// number (clamped to min/max)
    Float,
    /// free text
    Str,
    /// one of `choices`
    Choice,
    /// `#rrggbb`
    Color,
    /// list of strings
    List,
}

impl Kind {
    /// Name used in the shared schema file.
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Bool => "bool",
            Kind::Int => "int",
            Kind::Float => "float",
            Kind::Str => "str",
            Kind::Choice => "choice",
            Kind::Color => "color",
            Kind::List => "list",
        }
    }
}

/// Default value of a setting.
#[derive(Debug, Clone, Copy)]
pub enum Def {
    /// bool
    Bool(bool),
    /// integer
    Int(i64),
    /// number
    Float(f64),
    /// text / choice / color
    Str(&'static str),
    /// empty list
    List,
}

impl Def {
    /// As a JSON value.
    pub fn value(&self) -> Value {
        match *self {
            Def::Bool(b) => Value::Bool(b),
            Def::Int(i) => Value::from(i),
            Def::Float(f) => Value::from(f),
            Def::Str(s) => Value::from(s),
            Def::List => Value::Array(Vec::new()),
        }
    }
}

/// One user-visible setting.
#[derive(Debug, Clone, Copy)]
pub struct Setting {
    /// `section.name`
    pub key: &'static str,
    /// Value type.
    pub kind: Kind,
    /// Default.
    pub default: Def,
    /// Label shown in Preferences (Turkish, like the Python edition).
    pub label: &'static str,
    /// Longer explanation.
    pub help: &'static str,
    /// Lower limit for numbers.
    pub min: Option<f64>,
    /// Upper limit for numbers.
    pub max: Option<f64>,
    /// (value, label) pairs for choices.
    pub choices: &'static [(&'static str, &'static str)],
}

// BEGIN GENERATED SCHEMA — written by tests/data/parity/make_samples.py from dictz/storage/settings.py
/// Preference sections: (id, title).
pub const SECTIONS: &[(&str, &str)] = &[
    ("general", "Genel"),
    ("appearance", "Görünüm"),
    ("article", "Makale"),
    ("search", "Arama"),
    ("scan", "Tarama penceresi"),
    ("hotkeys", "Genel kısayollar"),
    ("dictionaries", "Sözlükler"),
    ("center", "Sözlük Merkezi"),
    ("audio", "Ses"),
    ("history", "Geçmiş ve Favoriler"),
];

/// Every setting (same order as the Python edition).
pub static SCHEMA: &[Setting] = &[
    Setting {
        key: "general.language",
        kind: Kind::Choice,
        default: Def::Str("auto"),
        label: "Arayüz dili",
        help: "Pencerenin sağ üstündeki bayraktan da değiştirilir. Otomatik = sistemin dili.",
        min: None,
        max: None,
        choices: &[("auto", "Otomatik (sistem)"), ("tr", "Türkçe"), ("en", "English"), ("az", "Azərbaycan türkcəsi"), ("de", "Deutsch"), ("fr", "Français"), ("es", "Español"), ("it", "Italiano"), ("pt", "Português"), ("nl", "Nederlands"), ("pl", "Polski"), ("ru", "Русский"), ("uk", "Українська"), ("ar", "العربية"), ("fa", "فارسی"), ("ur", "اردو"), ("kk", "Қазақша"), ("ky", "Кыргызча"), ("uz", "Oʻzbekcha"), ("tk", "Türkmençe"), ("zh", "中文"), ("ja", "日本語"), ("ko", "한국어"), ("id", "Bahasa Indonesia"), ("ms", "Bahasa Melayu"), ("hi", "हिन्दी")],
    },
    Setting {
        key: "general.restore_tabs",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Açık sekmeleri geri yükle",
        help: "Kapatırken açık olan sekmeler bir sonraki açılışta yeniden açılır.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "general.restore_last_search",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Son aramayı geri yükle",
        help: "Arama kutusu son yazılan kelimeyle açılır.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "general.check_updates_on_start",
        kind: Kind::Bool,
        default: Def::Bool(false),
        label: "Açılışta güncellemeleri denetle",
        help: "Yeni sürüm varsa durum çubuğunda haber verir (internet gerekir).",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "general.confirm_quit",
        kind: Kind::Bool,
        default: Def::Bool(false),
        label: "Çıkarken onay iste",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "general.remember_window",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Pencere boyutunu ve konumunu hatırla",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "general.always_on_top",
        kind: Kind::Bool,
        default: Def::Bool(false),
        label: "Pencere her zaman üstte",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "general.load_in_background",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Sözlükleri arka planda yükle",
        help: "Pencere hemen açılır; sözlükler yüklendikçe kullanılabilir olur.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "general.tray_icon",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Sistem tepsisinde simge göster",
        help: "Tıklayınca pencere açılır/gizlenir; sağ tık menüsünden tarama açılıp kapatılır.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "general.close_to_tray",
        kind: Kind::Bool,
        default: Def::Bool(false),
        label: "Kapatınca tepsiye gizle",
        help: "Pencere kapatılınca dictz tepside çalışmaya devam eder (tarama ve kısayollar çalışır).",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "general.start_in_tray",
        kind: Kind::Bool,
        default: Def::Bool(false),
        label: "Tepside başlat",
        help: "dictz açılışta pencere göstermeden tepside başlar.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "general.autostart",
        kind: Kind::Bool,
        default: Def::Bool(false),
        label: "Sistem açılınca başlat",
        help: "Bilgisayarda oturum açılınca dictz kendiliğinden, sistem tepsisinde başlar (Windows, Linux, macOS).",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "general.single_instance",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Tek kopya çalıştır",
        help: "dictz ikinci kez açılınca çalışan pencere öne gelir; `dictz popup` / `dictz show` bu kopyaya gider.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "appearance.theme",
        kind: Kind::Choice,
        default: Def::Str("auto"),
        label: "Tema",
        help: "Otomatik = sistemin açık / koyu rengi. Vurgu rengi varsayılandaysa temanın kendi vurgusu kullanılır.",
        min: None,
        max: None,
        choices: &[("auto", "Otomatik (sistem)"), ("light", "Açık"), ("sepia", "Sepya"), ("solarized-light", "Solarized Light"), ("gruvbox-light", "Gruvbox Light"), ("github-light", "GitHub Light"), ("catppuccin-latte", "Catppuccin Latte"), ("rose-pine-dawn", "Rosé Pine Dawn"), ("nord-light", "Nord Light"), ("one-light", "One Light"), ("mint", "Nane"), ("lavender", "Lavanta"), ("high-contrast-light", "Yüksek kontrast (açık)"), ("dark", "Koyu"), ("solarized-dark", "Solarized Dark"), ("gruvbox-dark", "Gruvbox Dark"), ("nord", "Nord"), ("dracula", "Dracula"), ("monokai", "Monokai"), ("one-dark", "One Dark"), ("tokyo-night", "Tokyo Night"), ("catppuccin-mocha", "Catppuccin Mocha"), ("rose-pine", "Rosé Pine"), ("github-dark", "GitHub Dark"), ("everforest", "Everforest"), ("midnight", "Gece mavisi"), ("oled", "Gece siyahı"), ("high-contrast-dark", "Yüksek kontrast (koyu)")],
    },
    Setting {
        key: "appearance.accent_color",
        kind: Kind::Color,
        default: Def::Str("#2563eb"),
        label: "Vurgu rengi",
        help: "Bağlantılar, sözlük başlıkları ve seçimler bu renkle gösterilir.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "appearance.ui_font_family",
        kind: Kind::Str,
        default: Def::Str(""),
        label: "Arayüz yazı tipi",
        help: "Boş = sistem yazı tipi. Rust sürümünde yazı tipi adı veya .ttf/.otf dosya yolu.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "appearance.ui_font_size",
        kind: Kind::Int,
        default: Def::Int(13),
        label: "Arayüz yazı boyutu",
        help: "",
        min: Some(8.0),
        max: Some(32.0),
        choices: &[],
    },
    Setting {
        key: "appearance.ui_scale",
        kind: Kind::Int,
        default: Def::Int(100),
        label: "Arayüz ölçeği",
        help: "Yüzde: tüm pencereyi büyütür ya da küçültür (yüksek çözünürlüklü ekranlar). Python sürümünde yeniden başlatınca uygulanır.",
        min: Some(50.0),
        max: Some(250.0),
        choices: &[],
    },
    Setting {
        key: "appearance.show_toolbar",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Araç çubuğunu göster",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "appearance.show_dictionary_bar",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Sözlük çubuğunu göster",
        help: "Her sözlük için tek tıkla aç/kapat düğmesi.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "appearance.show_found_in",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "“Bulunduğu sözlükler” panelini göster",
        help: "Kelimenin geçtiği sözlükler; tıklayınca o makaleye gider.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "appearance.show_sidebar",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Yan paneli göster",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "appearance.sidebar_side",
        kind: Kind::Choice,
        default: Def::Str("left"),
        label: "Yan panel konumu",
        help: "",
        min: None,
        max: None,
        choices: &[("left", "Sol"), ("right", "Sağ")],
    },
    Setting {
        key: "appearance.show_statusbar",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Durum çubuğunu göster",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "appearance.tab_title_length",
        kind: Kind::Int,
        default: Def::Int(28),
        label: "Sekme başlığı en fazla",
        help: "karakter",
        min: Some(8.0),
        max: Some(80.0),
        choices: &[],
    },
    Setting {
        key: "article.font_family",
        kind: Kind::Str,
        default: Def::Str(""),
        label: "Makale yazı tipi",
        help: "Boş = arayüz yazı tipi. Rust sürümünde yazı tipi adı veya dosya yolu.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "article.font_size",
        kind: Kind::Int,
        default: Def::Int(15),
        label: "Makale yazı boyutu",
        help: "",
        min: Some(8.0),
        max: Some(48.0),
        choices: &[],
    },
    Setting {
        key: "article.line_spacing",
        kind: Kind::Float,
        default: Def::Float(1.45),
        label: "Satır aralığı",
        help: "",
        min: Some(1.0),
        max: Some(3.0),
        choices: &[],
    },
    Setting {
        key: "article.headword_scale",
        kind: Kind::Float,
        default: Def::Float(1.2),
        label: "Başlık kelime büyüklüğü",
        help: "Makale yazı boyutunun katı.",
        min: Some(1.0),
        max: Some(3.0),
        choices: &[],
    },
    Setting {
        key: "article.zoom",
        kind: Kind::Float,
        default: Def::Float(1.0),
        label: "Yakınlaştırma",
        help: "",
        min: Some(0.5),
        max: Some(3.0),
        choices: &[],
    },
    Setting {
        key: "article.max_width",
        kind: Kind::Int,
        default: Def::Int(0),
        label: "Makale en fazla genişliği",
        help: "piksel; 0 = tam genişlik",
        min: Some(0.0),
        max: Some(4000.0),
        choices: &[],
    },
    Setting {
        key: "article.show_dict_headers",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Sözlük başlıklarını göster",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "article.collapsed",
        kind: Kind::Bool,
        default: Def::Bool(false),
        label: "Makaleleri kapalı aç",
        help: "Her sözlüğün makalesi başlığına tıklanınca açılır.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "article.show_images",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Resimleri göster",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "article.max_image_height",
        kind: Kind::Int,
        default: Def::Int(240),
        label: "Resim en fazla yüksekliği",
        help: "piksel",
        min: Some(32.0),
        max: Some(2000.0),
        choices: &[],
    },
    Setting {
        key: "article.highlight_color",
        kind: Kind::Color,
        default: Def::Str("#facc15"),
        label: "Sayfada bul vurgu rengi",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "search.live",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Yazarken ara",
        help: "Kapalıysa yalnızca Enter ile aranır (öneriler yine gösterilir).",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "search.debounce_ms",
        kind: Kind::Int,
        default: Def::Int(150),
        label: "Arama gecikmesi",
        help: "milisaniye",
        min: Some(0.0),
        max: Some(2000.0),
        choices: &[],
    },
    Setting {
        key: "search.min_chars",
        kind: Kind::Int,
        default: Def::Int(1),
        label: "Öneri için en az harf",
        help: "",
        min: Some(1.0),
        max: Some(10.0),
        choices: &[],
    },
    Setting {
        key: "search.suggestion_limit",
        kind: Kind::Int,
        default: Def::Int(100),
        label: "En fazla öneri",
        help: "",
        min: Some(5.0),
        max: Some(1000.0),
        choices: &[],
    },
    Setting {
        key: "search.prefix_fallback",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Tam eşleşme yoksa ile başlayanları göster",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "search.fuzzy",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Bulanık arama (“Bunu mu demek istediniz?”)",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "search.fuzzy_cutoff",
        kind: Kind::Float,
        default: Def::Float(60.0),
        label: "Bulanık eşleşme eşiği",
        help: "% benzerlik",
        min: Some(0.0),
        max: Some(100.0),
        choices: &[],
    },
    Setting {
        key: "search.fuzzy_limit",
        kind: Kind::Int,
        default: Def::Int(10),
        label: "En fazla bulanık öneri",
        help: "",
        min: Some(1.0),
        max: Some(100.0),
        choices: &[],
    },
    Setting {
        key: "search.fuzzy_fill_below",
        kind: Kind::Int,
        default: Def::Int(10),
        label: "Öneri sayısı bunun altındaysa bulanık eşleşme ekle",
        help: "",
        min: Some(0.0),
        max: Some(1000.0),
        choices: &[],
    },
    Setting {
        key: "search.max_articles",
        kind: Kind::Int,
        default: Def::Int(100),
        label: "Bir aramada en fazla makale",
        help: "",
        min: Some(1.0),
        max: Some(5000.0),
        choices: &[],
    },
    Setting {
        key: "search.enter_new_tab",
        kind: Kind::Bool,
        default: Def::Bool(false),
        label: "Enter sonucu yeni sekmede açsın",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "search.select_all_on_focus",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Arama kutusuna gelince metni seç",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "search.ignore_case",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Büyük/küçük harf duyarsız eşleşme",
        help: "Tam eşleşme yoksa büyük/küçük harf farkını yok sayarak arar.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "scan.enabled",
        kind: Kind::Bool,
        default: Def::Bool(false),
        label: "Taramayı aç",
        help: "Başka bir uygulamada kopyalanan / seçilen kelime açılır pencerede çevrilir.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "scan.ctrl_c_c",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Ctrl+C+C ile ara",
        help: "Kelimeyi seçip Ctrl+C'ye kısa aralıkla iki kez basın.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "scan.clipboard",
        kind: Kind::Bool,
        default: Def::Bool(false),
        label: "Her kopyalamada ara",
        help: "Tek bir Ctrl+C yeter (kopyaladığınız her metin aranır).",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "scan.selection",
        kind: Kind::Bool,
        default: Def::Bool(false),
        label: "Seçilen metni ara",
        help: "Yalnızca Linux: fareyle seçmek yeter (X11; Wayland'da masaüstüne bağlı).",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "scan.double_copy_ms",
        kind: Kind::Int,
        default: Def::Int(500),
        label: "Ctrl+C+C aralığı",
        help: "milisaniye",
        min: Some(150.0),
        max: Some(2000.0),
        choices: &[],
    },
    Setting {
        key: "scan.max_length",
        kind: Kind::Int,
        default: Def::Int(80),
        label: "En uzun metin",
        help: "karakter; daha uzunu yok sayılır",
        min: Some(1.0),
        max: Some(1000.0),
        choices: &[],
    },
    Setting {
        key: "scan.near_cursor",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Pencereyi imlecin yanında aç",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "scan.always_on_top",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Tarama penceresi her zaman üstte",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "scan.auto_hide",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Fare çıkınca gizle",
        help: "Sabitlenmemiş pencere, fare üzerinden ayrılınca kapanır.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "scan.hide_delay_ms",
        kind: Kind::Int,
        default: Def::Int(800),
        label: "Gizleme gecikmesi",
        help: "milisaniye",
        min: Some(0.0),
        max: Some(10000.0),
        choices: &[],
    },
    Setting {
        key: "scan.width",
        kind: Kind::Int,
        default: Def::Int(520),
        label: "Tarama penceresi genişliği",
        help: "piksel",
        min: Some(280.0),
        max: Some(1600.0),
        choices: &[],
    },
    Setting {
        key: "scan.height",
        kind: Kind::Int,
        default: Def::Int(420),
        label: "Tarama penceresi yüksekliği",
        help: "piksel",
        min: Some(200.0),
        max: Some(1200.0),
        choices: &[],
    },
    Setting {
        key: "scan.record_history",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Taramaları geçmişe kaydet",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "hotkeys.enabled",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Genel kısayolları kullan",
        help: "Pencere odakta değilken de çalışır. Wayland'da çoğu masaüstü izin vermez: masaüstü ayarlarında `dictz show` / `dictz popup` komutuna kısayol atayın.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "hotkeys.show_main",
        kind: Kind::Str,
        default: Def::Str("Ctrl+F11"),
        label: "Ana pencereyi göster/gizle",
        help: "Örnek: Ctrl+F11, Ctrl+Alt+D. Boş = kapalı.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "hotkeys.toggle_scan",
        kind: Kind::Str,
        default: Def::Str("Ctrl+F12"),
        label: "Taramayı aç/kapat",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "hotkeys.lookup_clipboard",
        kind: Kind::Str,
        default: Def::Str("Ctrl+Shift+F11"),
        label: "Panodaki kelimeyi tarama penceresinde ara",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "dictionaries.folders",
        kind: Kind::List,
        default: Def::List,
        label: "Sözlük klasörleri",
        help: "Açılışta bu klasörlerdeki .ifo / .dsl / .dsl.dz / .bgl dosyaları otomatik eklenir.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "dictionaries.recursive",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Alt klasörleri de tara",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "dictionaries.flat_folders",
        kind: Kind::List,
        default: Def::List,
        label: "Alt klasörleri taranmayan klasörler",
        help: "Sözlük klasörlerinden alt klasörleri taranmayanlar (klasör listesindeki kutu).",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "dictionaries.scan_on_startup",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Açılışta klasörleri tara",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "dictionaries.reindex_on_change",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Değişen sözlüğün indeksini yenile",
        help: "Dosya değiştiyse açılışta indeksi yeniden oluşturur.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "dictionaries.build_workers",
        kind: Kind::Int,
        default: Def::Int(0),
        label: "Aynı anda hazırlanan sözlük",
        help: "Yeni ya da değişmiş sözlükler ayrı işlemlerde, bu kadarı aynı anda okunur. 0 = otomatik (işlemci sayısı − 1, en çok 4). Her biri ayrıca bellek kullanır.",
        min: Some(0.0),
        max: Some(16.0),
        choices: &[],
    },
    Setting {
        key: "center.online",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "İnternet kaynaklarını kullan",
        help: "Kapalıyken Sözlük Merkezi hiçbir sunucuya bağlanmaz; yalnızca önceki listeler görünür.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "center.refresh_days",
        kind: Kind::Int,
        default: Def::Int(7),
        label: "Katalogları yenileme sıklığı (gün)",
        help: "Sözlük Merkezi açılınca bu kadar günden eski listeler arka planda yenilenir. 0 = yalnızca elle.",
        min: Some(0.0),
        max: Some(365.0),
        choices: &[],
    },
    Setting {
        key: "center.parallel_downloads",
        kind: Kind::Int,
        default: Def::Int(2),
        label: "Aynı anda indirme",
        help: "",
        min: Some(1.0),
        max: Some(8.0),
        choices: &[],
    },
    Setting {
        key: "center.install_folder",
        kind: Kind::Str,
        default: Def::Str(""),
        label: "İndirilen sözlüklerin klasörü",
        help: "Boş = veri klasöründe dictionaries/<kaynak>/<sözlük>.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "center.verify_checksums",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "İndirilenlerin sağlamasını denetle",
        help: "Katalog SHA-256 / SHA-512 / MD5 veriyorsa dosya denetlenir; tutmazsa kurulmaz.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "center.keep_archives",
        kind: Kind::Bool,
        default: Def::Bool(false),
        label: "İndirilen arşivi de sakla",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "center.add_after_install",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Kurulunca sözlük listesine ekle",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "center.show_unknown_license",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Lisansı bilinmeyenleri de göster",
        help: "Kapalıyken katalog yalnızca açık ya da serbest lisanslı sözlükleri gösterir.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "center.proxy",
        kind: Kind::Str,
        default: Def::Str(""),
        label: "Vekil sunucu (proxy)",
        help: "Boş = sistemin ayarı; none = doğrudan; ya da http://sunucu:port",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "center.timeout",
        kind: Kind::Int,
        default: Def::Int(30),
        label: "Bağlantı zaman aşımı (saniye)",
        help: "",
        min: Some(5.0),
        max: Some(300.0),
        choices: &[],
    },
    Setting {
        key: "audio.auto_play",
        kind: Kind::Bool,
        default: Def::Bool(false),
        label: "Aramada ilk sesi otomatik çal",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "audio.volume",
        kind: Kind::Int,
        default: Def::Int(80),
        label: "Ses düzeyi",
        help: "%",
        min: Some(0.0),
        max: Some(100.0),
        choices: &[],
    },
    Setting {
        key: "audio.system_player",
        kind: Kind::Bool,
        default: Def::Bool(false),
        label: "Sistem oynatıcısını kullan",
        help: "Sesler uygulama içinde değil, işletim sisteminin oynatıcısıyla açılır.",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "history.enabled",
        kind: Kind::Bool,
        default: Def::Bool(true),
        label: "Arama geçmişini kaydet",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "history.limit",
        kind: Kind::Int,
        default: Def::Int(200),
        label: "Yan panelde gösterilen geçmiş",
        help: "",
        min: Some(10.0),
        max: Some(5000.0),
        choices: &[],
    },
    Setting {
        key: "history.max_entries",
        kind: Kind::Int,
        default: Def::Int(10000),
        label: "Veritabanında tutulan en fazla kayıt",
        help: "Eskiler silinir.",
        min: Some(100.0),
        max: Some(1000000.0),
        choices: &[],
    },
    Setting {
        key: "history.clear_on_exit",
        kind: Kind::Bool,
        default: Def::Bool(false),
        label: "Çıkarken geçmişi temizle",
        help: "",
        min: None,
        max: None,
        choices: &[],
    },
    Setting {
        key: "history.favorites_limit",
        kind: Kind::Int,
        default: Def::Int(500),
        label: "Yan panelde gösterilen favori",
        help: "",
        min: Some(10.0),
        max: Some(10000.0),
        choices: &[],
    },
];
// END GENERATED SCHEMA

/// Window / session state (not shown in Preferences).
pub fn state_defaults() -> Vec<(&'static str, Value)> {
    vec![
        ("state.last_search", Value::from("")),
        ("state.sidebar_width", Value::from(230)),
        ("state.open_tabs", Value::Array(Vec::new())),
        ("state.current_tab", Value::from(0)),
        ("state.layout_version", Value::from(0)),
    ]
}

/// v0.1.4: the compact layout. Settings still at the old (larger) defaults
/// move to the new ones once; values the user chose are kept (same table as
/// the Python edition's `OLD_DEFAULTS`).
pub const LAYOUT_VERSION: i64 = 2;
const OLD_DEFAULTS: &[(&str, f64, f64)] = &[
    ("appearance.ui_font_size", 14.0, 13.0),
    ("article.font_size", 16.0, 15.0),
    ("article.line_spacing", 1.6, 1.45),
    ("article.headword_scale", 1.4, 1.2),
    ("state.sidebar_width", 280.0, 230.0),
];

/// Look a setting up by key.
pub fn setting(key: &str) -> Option<&'static Setting> {
    SCHEMA.iter().find(|s| s.key == key)
}

fn truthy(s: &str) -> Option<bool> {
    match s.trim().to_lowercase().as_str() {
        "1" | "true" | "yes" | "on" | "evet" => Some(true),
        "0" | "false" | "no" | "off" | "hayır" | "hayir" => Some(false),
        _ => None,
    }
}

/// Validate / convert `value` for `s` (clamps numbers).
pub fn coerce(s: &Setting, value: &Value) -> Result<Value, String> {
    match s.kind {
        Kind::Bool => match value {
            Value::Bool(b) => Ok(Value::Bool(*b)),
            Value::String(t) => truthy(t).map(Value::Bool).ok_or_else(|| {
                format!(
                    "{}: expected true/false, got {}",
                    s.key,
                    pyrepr(&Value::from(t.as_str()))
                )
            }),
            Value::Number(n) => Ok(Value::Bool(n.as_f64().unwrap_or(0.0) != 0.0)),
            other => Err(format!(
                "{}: expected true/false, got {}",
                s.key,
                pyrepr(other)
            )),
        },
        Kind::Int | Kind::Float => {
            let v = match value {
                Value::Number(n) => n.as_f64(),
                Value::String(t) => t.trim().parse::<f64>().ok(),
                Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
                _ => None,
            }
            .filter(|v| v.is_finite())
            .ok_or_else(|| format!("{}: expected a number, got {}", s.key, pyrepr(value)))?;
            let mut v = v;
            if let Some(m) = s.min {
                v = v.max(m);
            }
            if let Some(m) = s.max {
                v = v.min(m);
            }
            Ok(if s.kind == Kind::Int {
                Value::from(v.round_ties_even() as i64)
            } else {
                Value::from(v)
            })
        }
        Kind::Choice => {
            let t = match value {
                Value::String(t) => t.clone(),
                other => other.to_string(),
            };
            if s.choices.iter().any(|(c, _)| *c == t) {
                Ok(Value::from(t))
            } else {
                let allowed: Vec<&str> = s.choices.iter().map(|c| c.0).collect();
                Err(format!(
                    "{}: one of {}, got {}",
                    s.key,
                    allowed.join(", "),
                    pyrepr(value)
                ))
            }
        }
        Kind::Color => {
            let t = value.as_str().unwrap_or("").trim().to_string();
            if t.len() == 7 && t.starts_with('#') && t[1..].bytes().all(|b| b.is_ascii_hexdigit()) {
                Ok(Value::from(t.to_lowercase()))
            } else {
                Err(format!(
                    "{}: expected #rrggbb, got {}",
                    s.key,
                    pyrepr(value)
                ))
            }
        }
        Kind::List => match value {
            Value::Array(a) => Ok(Value::Array(
                a.iter()
                    .map(|v| {
                        Value::from(
                            v.as_str()
                                .map(str::to_string)
                                .unwrap_or_else(|| v.to_string()),
                        )
                    })
                    .collect(),
            )),
            Value::String(t) => {
                let sep = if cfg!(windows) { ';' } else { ':' };
                Ok(Value::Array(
                    t.split(sep)
                        .map(str::trim)
                        .filter(|p| !p.is_empty())
                        .map(Value::from)
                        .collect(),
                ))
            }
            _ => Err(format!("{}: expected a list", s.key)),
        },
        Kind::Str => Ok(Value::from(match value {
            Value::String(t) => t.clone(),
            Value::Null => String::new(),
            other => other.to_string(),
        })),
    }
}

/// Loaded preferences.
#[derive(Debug, Clone)]
pub struct Preferences {
    path: Option<PathBuf>,
    values: Map<String, Value>,
    raw: Map<String, Value>,
}

impl Default for Preferences {
    fn default() -> Self {
        let mut values = Map::new();
        for s in SCHEMA {
            values.insert(s.key.to_string(), s.default.value());
        }
        for (k, v) in state_defaults() {
            values.insert(k.to_string(), v);
        }
        Self {
            path: None,
            values,
            raw: Map::new(),
        }
    }
}

/// Python's `repr()` of a JSON value (error messages match the Python edition).
pub fn pyrepr(v: &Value) -> String {
    match v {
        Value::String(t) => {
            if t.contains('\'') && !t.contains('"') {
                format!("\"{}\"", t)
            } else {
                format!("'{}'", t.replace('\\', "\\\\").replace('\'', "\\'"))
            }
        }
        Value::Bool(b) => if *b { "True" } else { "False" }.to_string(),
        Value::Null => "None".to_string(),
        Value::Array(a) => format!("[{}]", a.iter().map(pyrepr).collect::<Vec<_>>().join(", ")),
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter()
                .map(|(k, v)| format!("{}: {}", pyrepr(&Value::from(k.as_str())), pyrepr(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Number(n) => n.to_string(),
    }
}

/// Copy of `v` with object keys sorted recursively (`json.dump(sort_keys=True)`).
fn sort_keys(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            Value::Object(
                keys.into_iter()
                    .map(|k| (k.clone(), sort_keys(&m[k])))
                    .collect(),
            )
        }
        Value::Array(a) => Value::Array(a.iter().map(sort_keys).collect()),
        other => other.clone(),
    }
}

fn read_json(path: &Path) -> Option<Map<String, Value>> {
    let text = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str::<Value>(&text).ok()? {
        Value::Object(m) => Some(m),
        _ => None,
    }
}

impl Preferences {
    /// Load `path`; missing file or keys fall back to the defaults.
    pub fn load(path: &Path) -> Self {
        let mut p = Self {
            path: Some(path.to_path_buf()),
            ..Self::default()
        };
        let raw = read_json(path).unwrap_or_default();
        let keys: Vec<String> = p.values.keys().cloned().collect();
        for key in keys {
            let (section, name) = key.split_once('.').unwrap();
            if let Some(v) = raw.get(section).and_then(|s| s.get(name)) {
                match setting(&key) {
                    Some(s) => {
                        if let Ok(c) = coerce(s, v) {
                            p.values.insert(key, c);
                        }
                    }
                    None => {
                        p.values.insert(key, v.clone());
                    }
                }
            }
        }
        let has_file = !raw.is_empty();
        let version = p
            .values
            .get("state.layout_version")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        if has_file && version < LAYOUT_VERSION {
            for (key, old, new) in OLD_DEFAULTS {
                let cur = p.values.get(*key).and_then(Value::as_f64);
                if cur.map(|c| (c - old).abs() < 1e-9).unwrap_or(false) {
                    let v = if key.ends_with("font_size") || key.ends_with("_width") {
                        Value::from(*new as i64)
                    } else {
                        Value::from(*new)
                    };
                    p.values.insert(key.to_string(), v);
                }
            }
        }
        p.values
            .insert("state.layout_version".into(), Value::from(LAYOUT_VERSION));
        p.raw = raw;
        p
    }

    /// File this was loaded from.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// `true` when the settings file exists.
    pub fn exists(&self) -> bool {
        self.path.as_ref().map(|p| p.exists()).unwrap_or(false)
    }

    /// Write back, merging with what is on disk (keeps the other edition's keys).
    pub fn save(&mut self) -> std::io::Result<()> {
        let Some(path) = self.path.clone() else {
            return Ok(());
        };
        let mut raw = read_json(&path).unwrap_or_else(|| self.raw.clone());
        for (key, value) in &self.values {
            let (section, name) = key.split_once('.').unwrap();
            let sec = raw
                .entry(section.to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            if !sec.is_object() {
                *sec = Value::Object(Map::new());
            }
            sec.as_object_mut()
                .unwrap()
                .insert(name.to_string(), value.clone());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        let sorted = sort_keys(&Value::Object(raw.clone()));
        std::fs::write(
            &tmp,
            format!("{}\n", serde_json::to_string_pretty(&sorted)?),
        )?;
        std::fs::rename(&tmp, &path)?;
        self.raw = raw;
        Ok(())
    }

    /// Raw value.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }

    /// Bool value (false when missing).
    pub fn bool(&self, key: &str) -> bool {
        self.get(key).and_then(Value::as_bool).unwrap_or(false)
    }

    /// Integer value.
    pub fn int(&self, key: &str) -> i64 {
        self.get(key)
            .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
            .unwrap_or(0)
    }

    /// Number value.
    pub fn float(&self, key: &str) -> f64 {
        self.get(key).and_then(Value::as_f64).unwrap_or(0.0)
    }

    /// Text value.
    pub fn str(&self, key: &str) -> String {
        self.get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    }

    /// List value.
    pub fn list(&self, key: &str) -> Vec<String> {
        self.get(key)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Set a value (validated). Returns the stored value.
    pub fn set(&mut self, key: &str, value: Value) -> Result<Value, String> {
        let v = match setting(key) {
            Some(s) => coerce(s, &value)?,
            None if key.starts_with("state.") => value,
            None => return Err(format!("unknown setting: {}", key)),
        };
        self.values.insert(key.to_string(), v.clone());
        Ok(v)
    }

    /// Reset one key, or everything with `None`.
    pub fn reset(&mut self, key: Option<&str>) -> Result<(), String> {
        match key {
            None => {
                let fresh = Self::default();
                self.values = fresh.values;
                Ok(())
            }
            Some(k) => {
                let s = setting(k).ok_or_else(|| format!("unknown setting: {}", k))?;
                self.values.insert(k.to_string(), s.default.value());
                Ok(())
            }
        }
    }

    /// Nested `{section: {name: value}}` object.
    pub fn export(&self) -> Value {
        let mut out = Map::new();
        for (key, value) in &self.values {
            let (section, name) = key.split_once('.').unwrap();
            out.entry(section.to_string())
                .or_insert_with(|| Value::Object(Map::new()))
                .as_object_mut()
                .unwrap()
                .insert(name.to_string(), value.clone());
        }
        Value::Object(out)
    }

    /// Apply an exported object. Returns error messages.
    pub fn import(&mut self, data: &Value) -> Vec<String> {
        let mut errors = Vec::new();
        if let Some(obj) = data.as_object() {
            for (section, sec) in obj {
                if let Some(sec) = sec.as_object() {
                    for (name, value) in sec {
                        if let Err(e) = self.set(&format!("{}.{}", section, name), value.clone()) {
                            errors.push(e);
                        }
                    }
                }
            }
        }
        errors
    }

    /// One-time import of the v0.0.6 `dictz-rs.ini`. Returns true if done.
    pub fn migrate_from_ini(&mut self, ini: &Path) -> bool {
        if self.exists() {
            return false;
        }
        let Ok(text) = std::fs::read_to_string(ini) else {
            return false;
        };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let key = match k.trim() {
                "theme" => "appearance.theme",
                "font_family" => "appearance.ui_font_family",
                "font_size" => "appearance.ui_font_size",
                "zoom_factor" => "article.zoom",
                "search_debounce_ms" => "search.debounce_ms",
                "show_sidebar" => "appearance.show_sidebar",
                "sidebar_width" => "state.sidebar_width",
                "last_search" => "state.last_search",
                _ => continue,
            };
            let v = v.trim();
            let value = if key.starts_with("state.sidebar") {
                v.parse::<f64>()
                    .map(Value::from)
                    .unwrap_or(Value::from(280))
            } else {
                Value::from(v)
            };
            let _ = self.set(key, value);
        }
        self.save().is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_default_sizes_move_to_the_compact_ones_once() {
        let f = std::env::temp_dir().join(format!("dictz-layout-{}.json", std::process::id()));
        std::fs::write(
            &f,
            "{\"appearance\": {\"ui_font_size\": 14}, \"article\": {\"font_size\": 18, \"line_spacing\": 1.6}}",
        )
        .unwrap();
        let mut p = Preferences::load(&f);
        assert_eq!(
            p.int("appearance.ui_font_size"),
            13,
            "old default → new default"
        );
        assert_eq!(p.int("article.font_size"), 18, "the user's own value stays");
        assert!((p.float("article.line_spacing") - 1.45).abs() < 1e-9);
        p.save().unwrap();
        p.set("appearance.ui_font_size", Value::from(14)).unwrap();
        p.save().unwrap();
        // chosen again after the move: kept
        assert_eq!(Preferences::load(&f).int("appearance.ui_font_size"), 14);
        let _ = std::fs::remove_file(&f);
    }

    #[test]
    fn defaults_and_roundtrip() {
        let f = std::env::temp_dir().join(format!("dictz-settings-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&f);
        std::fs::write(
            &f,
            "{\"other\": {\"keep\": 1}, \"appearance\": {\"theme\": \"bogus\"}}",
        )
        .unwrap();
        let mut p = Preferences::load(&f);
        assert_eq!(
            p.str("appearance.theme"),
            "auto",
            "invalid value falls back to default"
        );
        assert_eq!(p.int("article.font_size"), 15);
        p.set("appearance.theme", Value::from("dark")).unwrap();
        assert_eq!(
            p.set("article.font_size", Value::from(500)).unwrap(),
            Value::from(48)
        );
        assert_eq!(
            p.set("search.live", Value::from("off")).unwrap(),
            Value::Bool(false)
        );
        assert!(p
            .set("appearance.accent_color", Value::from("blue"))
            .is_err());
        assert!(p.set("nope.x", Value::from(1)).is_err());
        p.save().unwrap();
        let q = Preferences::load(&f);
        assert_eq!(q.str("appearance.theme"), "dark");
        let raw: Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(raw["other"]["keep"], Value::from(1), "unknown keys survive");
    }

    #[test]
    fn int_rounding_is_bankers() {
        let mut p = Preferences::default();
        assert_eq!(
            p.set("article.font_size", Value::from(12.5)).unwrap(),
            Value::from(12)
        );
        assert_eq!(
            p.set("article.font_size", Value::from(13.5)).unwrap(),
            Value::from(14)
        );
    }
}

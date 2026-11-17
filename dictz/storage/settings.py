"""User preferences — ``settings.json`` in the data folder.

The same file, keys, defaults and limits are used by the Rust edition
(``dictz-rs/src/settings.rs``); ``tests/data/parity/settings_schema.json`` is
checked by both test suites. Unknown keys are preserved on save, so the two
editions never wipe each other's values.

Keys are ``section.name``. Values outside the schema's limits are clamped.
"""
from __future__ import annotations

import contextlib
import json
import os
import tempfile
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from ..core.i18n import LANGUAGES

SETTINGS_FILE = "settings.json"


@dataclass(frozen=True)
class Setting:
    key: str
    kind: str  # bool | int | float | str | choice | color | list
    default: Any
    label: str
    help: str = ""
    min: float | None = None
    max: float | None = None
    choices: tuple[tuple[str, str], ...] = ()


SECTIONS: list[tuple[str, str]] = [
    ("general", "Genel"),
    ("appearance", "Görünüm"),
    ("article", "Makale"),
    ("search", "Arama"),
    ("scan", "Tarama penceresi"),
    ("hotkeys", "Genel kısayollar"),
    ("dictionaries", "Sözlükler"),
    ("audio", "Ses"),
    ("history", "Geçmiş ve Favoriler"),
]

SCHEMA: list[Setting] = [
    # ------------------------------------------------------------- general
    Setting("general.language", "choice", "auto", "Arayüz dili",
            "Pencerenin sağ üstündeki bayraktan da değiştirilir. Otomatik = sistemin dili.",
            choices=(("auto", "Otomatik (sistem)"),) + tuple((c, n) for c, n, _e, _r in LANGUAGES)),
    Setting("general.restore_tabs", "bool", True, "Açık sekmeleri geri yükle",
            "Kapatırken açık olan sekmeler bir sonraki açılışta yeniden açılır."),
    Setting("general.restore_last_search", "bool", True, "Son aramayı geri yükle",
            "Arama kutusu son yazılan kelimeyle açılır."),
    Setting("general.check_updates_on_start", "bool", False, "Açılışta güncellemeleri denetle",
            "Yeni sürüm varsa durum çubuğunda haber verir (internet gerekir)."),
    Setting("general.confirm_quit", "bool", False, "Çıkarken onay iste"),
    Setting("general.remember_window", "bool", True, "Pencere boyutunu ve konumunu hatırla"),
    Setting("general.always_on_top", "bool", False, "Pencere her zaman üstte"),
    Setting("general.load_in_background", "bool", True, "Sözlükleri arka planda yükle",
            "Pencere hemen açılır; sözlükler yüklendikçe kullanılabilir olur."),
    Setting("general.tray_icon", "bool", True, "Sistem tepsisinde simge göster",
            "Tıklayınca pencere açılır/gizlenir; sağ tık menüsünden tarama açılıp kapatılır."),
    Setting("general.close_to_tray", "bool", False, "Kapatınca tepsiye gizle",
            "Pencere kapatılınca dictz tepside çalışmaya devam eder (tarama ve kısayollar çalışır)."),
    Setting("general.start_in_tray", "bool", False, "Tepside başlat",
            "dictz açılışta pencere göstermeden tepside başlar."),
    Setting("general.single_instance", "bool", True, "Tek kopya çalıştır",
            "dictz ikinci kez açılınca çalışan pencere öne gelir; `dictz popup` / `dictz show` bu kopyaya gider."),
    # ---------------------------------------------------------- appearance
    Setting("appearance.theme", "choice", "auto", "Tema",
            choices=(("auto", "Otomatik (sistem)"), ("light", "Açık"), ("dark", "Koyu"))),
    Setting("appearance.accent_color", "color", "#2563eb", "Vurgu rengi",
            "Bağlantılar, sözlük başlıkları ve seçimler bu renkle gösterilir."),
    Setting("appearance.ui_font_family", "str", "", "Arayüz yazı tipi",
            "Boş = sistem yazı tipi. Rust sürümünde yazı tipi adı veya .ttf/.otf dosya yolu."),
    Setting("appearance.ui_font_size", "int", 14, "Arayüz yazı boyutu", min=8, max=32),
    Setting("appearance.show_toolbar", "bool", True, "Araç çubuğunu göster"),
    Setting("appearance.show_dictionary_bar", "bool", True, "Sözlük çubuğunu göster",
            "Her sözlük için tek tıkla aç/kapat düğmesi."),
    Setting("appearance.show_found_in", "bool", True, "“Bulunduğu sözlükler” panelini göster",
            "Kelimenin geçtiği sözlükler; tıklayınca o makaleye gider."),
    Setting("appearance.show_sidebar", "bool", True, "Yan paneli göster"),
    Setting("appearance.sidebar_side", "choice", "left", "Yan panel konumu",
            choices=(("left", "Sol"), ("right", "Sağ"))),
    Setting("appearance.show_statusbar", "bool", True, "Durum çubuğunu göster"),
    Setting("appearance.tab_title_length", "int", 28, "Sekme başlığı en fazla", "karakter", min=8, max=80),
    # ------------------------------------------------------------- article
    Setting("article.font_family", "str", "", "Makale yazı tipi",
            "Boş = arayüz yazı tipi. Rust sürümünde yazı tipi adı veya dosya yolu."),
    Setting("article.font_size", "int", 16, "Makale yazı boyutu", min=8, max=48),
    Setting("article.line_spacing", "float", 1.6, "Satır aralığı", min=1.0, max=3.0),
    Setting("article.headword_scale", "float", 1.4, "Başlık kelime büyüklüğü",
            "Makale yazı boyutunun katı.", min=1.0, max=3.0),
    Setting("article.zoom", "float", 1.0, "Yakınlaştırma", min=0.5, max=3.0),
    Setting("article.max_width", "int", 0, "Makale en fazla genişliği", "piksel; 0 = tam genişlik",
            min=0, max=4000),
    Setting("article.show_dict_headers", "bool", True, "Sözlük başlıklarını göster"),
    Setting("article.collapsed", "bool", False, "Makaleleri kapalı aç",
            "Her sözlüğün makalesi başlığına tıklanınca açılır."),
    Setting("article.show_images", "bool", True, "Resimleri göster"),
    Setting("article.max_image_height", "int", 240, "Resim en fazla yüksekliği", "piksel", min=32, max=2000),
    Setting("article.highlight_color", "color", "#facc15", "Sayfada bul vurgu rengi"),
    # -------------------------------------------------------------- search
    Setting("search.live", "bool", True, "Yazarken ara",
            "Kapalıysa yalnızca Enter ile aranır (öneriler yine gösterilir)."),
    Setting("search.debounce_ms", "int", 150, "Arama gecikmesi", "milisaniye", min=0, max=2000),
    Setting("search.min_chars", "int", 1, "Öneri için en az harf", min=1, max=10),
    Setting("search.suggestion_limit", "int", 100, "En fazla öneri", min=5, max=1000),
    Setting("search.prefix_fallback", "bool", True, "Tam eşleşme yoksa ile başlayanları göster"),
    Setting("search.fuzzy", "bool", True, "Bulanık arama (“Bunu mu demek istediniz?”)"),
    Setting("search.fuzzy_cutoff", "float", 60.0, "Bulanık eşleşme eşiği", "% benzerlik", min=0, max=100),
    Setting("search.fuzzy_limit", "int", 10, "En fazla bulanık öneri", min=1, max=100),
    Setting("search.fuzzy_fill_below", "int", 10, "Öneri sayısı bunun altındaysa bulanık eşleşme ekle",
            min=0, max=1000),
    Setting("search.max_articles", "int", 100, "Bir aramada en fazla makale", min=1, max=5000),
    Setting("search.enter_new_tab", "bool", False, "Enter sonucu yeni sekmede açsın"),
    Setting("search.select_all_on_focus", "bool", True, "Arama kutusuna gelince metni seç"),
    Setting("search.ignore_case", "bool", True, "Büyük/küçük harf duyarsız eşleşme",
            "Tam eşleşme yoksa büyük/küçük harf farkını yok sayarak arar."),
    # ---------------------------------------------------------------- scan
    Setting("scan.enabled", "bool", False, "Taramayı aç",
            "Başka bir uygulamada kopyalanan / seçilen kelime açılır pencerede çevrilir."),
    Setting("scan.ctrl_c_c", "bool", True, "Ctrl+C+C ile ara",
            "Kelimeyi seçip Ctrl+C'ye kısa aralıkla iki kez basın."),
    Setting("scan.clipboard", "bool", False, "Her kopyalamada ara",
            "Tek bir Ctrl+C yeter (kopyaladığınız her metin aranır)."),
    Setting("scan.selection", "bool", False, "Seçilen metni ara",
            "Yalnızca Linux: fareyle seçmek yeter (X11; Wayland'da masaüstüne bağlı)."),
    Setting("scan.double_copy_ms", "int", 500, "Ctrl+C+C aralığı", "milisaniye", min=150, max=2000),
    Setting("scan.max_length", "int", 80, "En uzun metin", "karakter; daha uzunu yok sayılır", min=1, max=1000),
    Setting("scan.near_cursor", "bool", True, "Pencereyi imlecin yanında aç"),
    Setting("scan.always_on_top", "bool", True, "Tarama penceresi her zaman üstte"),
    Setting("scan.auto_hide", "bool", True, "Fare çıkınca gizle",
            "Sabitlenmemiş pencere, fare üzerinden ayrılınca kapanır."),
    Setting("scan.hide_delay_ms", "int", 800, "Gizleme gecikmesi", "milisaniye", min=0, max=10000),
    Setting("scan.width", "int", 520, "Tarama penceresi genişliği", "piksel", min=280, max=1600),
    Setting("scan.height", "int", 420, "Tarama penceresi yüksekliği", "piksel", min=200, max=1200),
    Setting("scan.record_history", "bool", True, "Taramaları geçmişe kaydet"),
    # ------------------------------------------------------------- hotkeys
    Setting("hotkeys.enabled", "bool", True, "Genel kısayolları kullan",
            "Pencere odakta değilken de çalışır. Wayland'da çoğu masaüstü izin vermez: "
            "masaüstü ayarlarında `dictz show` / `dictz popup` komutuna kısayol atayın."),
    Setting("hotkeys.show_main", "str", "Ctrl+F11", "Ana pencereyi göster/gizle",
            "Örnek: Ctrl+F11, Ctrl+Alt+D. Boş = kapalı."),
    Setting("hotkeys.toggle_scan", "str", "Ctrl+F12", "Taramayı aç/kapat"),
    Setting("hotkeys.lookup_clipboard", "str", "Ctrl+Shift+F11", "Panodaki kelimeyi tarama penceresinde ara"),
    # -------------------------------------------------------- dictionaries
    Setting("dictionaries.folders", "list", [], "Sözlük klasörleri",
            "Açılışta bu klasörlerdeki .ifo / .dsl / .dsl.dz / .bgl dosyaları otomatik eklenir."),
    Setting("dictionaries.recursive", "bool", True, "Alt klasörleri de tara"),
    Setting("dictionaries.flat_folders", "list", [], "Alt klasörleri taranmayan klasörler",
            "Sözlük klasörlerinden alt klasörleri taranmayanlar (klasör listesindeki kutu)."),
    Setting("dictionaries.scan_on_startup", "bool", True, "Açılışta klasörleri tara"),
    Setting("dictionaries.reindex_on_change", "bool", True, "Değişen sözlüğün indeksini yenile",
            "Dosya değiştiyse açılışta indeksi yeniden oluşturur."),
    # --------------------------------------------------------------- audio
    Setting("audio.auto_play", "bool", False, "Aramada ilk sesi otomatik çal"),
    Setting("audio.volume", "int", 80, "Ses düzeyi", "%", min=0, max=100),
    Setting("audio.system_player", "bool", False, "Sistem oynatıcısını kullan",
            "Sesler uygulama içinde değil, işletim sisteminin oynatıcısıyla açılır."),
    # ------------------------------------------------------------- history
    Setting("history.enabled", "bool", True, "Arama geçmişini kaydet"),
    Setting("history.limit", "int", 200, "Yan panelde gösterilen geçmiş", min=10, max=5000),
    Setting("history.max_entries", "int", 10000, "Veritabanında tutulan en fazla kayıt",
            "Eskiler silinir.", min=100, max=1000000),
    Setting("history.clear_on_exit", "bool", False, "Çıkarken geçmişi temizle"),
    Setting("history.favorites_limit", "int", 500, "Yan panelde gösterilen favori", min=10, max=10000),
]

SCHEMA_BY_KEY: dict[str, Setting] = {s.key: s for s in SCHEMA}

# Window / session state (not shown in Preferences).
STATE_DEFAULTS: dict[str, Any] = {
    "state.last_search": "",
    "state.sidebar_width": 280,
    "state.open_tabs": [],
    "state.current_tab": 0,
}


def schema_as_json() -> list[dict]:
    """Serialisable schema (shared with the Rust edition's parity test)."""
    out = []
    for s in SCHEMA:
        d: dict[str, Any] = {"key": s.key, "kind": s.kind, "default": s.default, "label": s.label,
                             "help": s.help}
        if s.min is not None:
            d["min"] = s.min
        if s.max is not None:
            d["max"] = s.max
        if s.choices:
            d["choices"] = [list(c) for c in s.choices]
        out.append(d)
    return out


def coerce(setting: Setting, value: Any) -> Any:
    """Validate/convert ``value``; raises ValueError when impossible."""
    k = setting.kind
    if k == "bool":
        if isinstance(value, bool):
            return value
        if isinstance(value, str) and value.strip().lower() in ("1", "true", "yes", "on", "evet"):
            return True
        if isinstance(value, str) and value.strip().lower() in ("0", "false", "no", "off", "hayır", "hayir"):
            return False
        if isinstance(value, (int, float)):
            return bool(value)
        raise ValueError(f"{setting.key}: expected true/false, got {value!r}")
    if k in ("int", "float"):
        try:
            v = float(value)
        except (TypeError, ValueError):
            raise ValueError(f"{setting.key}: expected a number, got {value!r}") from None
        if setting.min is not None:
            v = max(setting.min, v)
        if setting.max is not None:
            v = min(setting.max, v)
        return int(round(v)) if k == "int" else float(v)
    if k == "choice":
        allowed = [c[0] for c in setting.choices]
        if str(value) not in allowed:
            raise ValueError(f"{setting.key}: one of {', '.join(allowed)}, got {value!r}")
        return str(value)
    if k == "color":
        v = str(value).strip()
        if len(v) == 7 and v.startswith("#") and all(c in "0123456789abcdefABCDEF" for c in v[1:]):
            return v.lower()
        raise ValueError(f"{setting.key}: expected #rrggbb, got {value!r}")
    if k == "list":
        if isinstance(value, str):
            return [p.strip() for p in value.split(os.pathsep) if p.strip()] if value.strip() else []
        if isinstance(value, (list, tuple)):
            return [str(v) for v in value]
        raise ValueError(f"{setting.key}: expected a list")
    return "" if value is None else str(value)


@dataclass
class Preferences:
    path: Path | None = None
    values: dict[str, Any] = field(default_factory=dict)
    _raw: dict = field(default_factory=dict, repr=False)

    # ------------------------------------------------------------ io
    @classmethod
    def defaults(cls) -> Preferences:
        p = cls()
        p.values = {s.key: _copy(s.default) for s in SCHEMA}
        p.values.update({k: _copy(v) for k, v in STATE_DEFAULTS.items()})
        return p

    @classmethod
    def load(cls, path: Path | str | None = None) -> Preferences:
        if path is None:
            from ..app import user_data_dir

            path = user_data_dir() / SETTINGS_FILE
        p = cls.defaults()
        p.path = Path(path)
        try:
            raw = json.loads(p.path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            raw = {}
        if not isinstance(raw, dict):
            raw = {}
        p._raw = raw
        for key in list(p.values):
            section, name = key.split(".", 1)
            sec = raw.get(section)
            if isinstance(sec, dict) and name in sec:
                setting = SCHEMA_BY_KEY.get(key)
                with contextlib.suppress(ValueError):
                    p.values[key] = coerce(setting, sec[name]) if setting else sec[name]
        return p

    def exists(self) -> bool:
        return self.path is not None and self.path.exists()

    def save(self) -> None:
        if self.path is None:
            return
        raw = dict(self._raw)
        # re-read so keys written meanwhile by the other edition survive
        try:
            disk = json.loads(self.path.read_text(encoding="utf-8"))
            if isinstance(disk, dict):
                raw = disk
        except (OSError, ValueError):
            pass
        for key, value in self.values.items():
            section, name = key.split(".", 1)
            sec = raw.get(section)
            if not isinstance(sec, dict):
                sec = {}
                raw[section] = sec
            sec[name] = value
        self._raw = raw
        self.path.parent.mkdir(parents=True, exist_ok=True)
        fd, tmp = tempfile.mkstemp(prefix=".settings-", dir=str(self.path.parent))
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            json.dump(raw, f, ensure_ascii=False, indent=2, sort_keys=True)
            f.write("\n")
        os.replace(tmp, self.path)

    # ------------------------------------------------------------ access
    def __getitem__(self, key: str) -> Any:
        return self.values[key]

    def get(self, key: str, default: Any = None) -> Any:
        return self.values.get(key, default)

    def set(self, key: str, value: Any, save: bool = False) -> Any:
        setting = SCHEMA_BY_KEY.get(key)
        if setting is None and key not in STATE_DEFAULTS:
            raise KeyError(f"unknown setting: {key}")
        v = coerce(setting, value) if setting else value
        self.values[key] = v
        if save:
            self.save()
        return v

    def reset(self, key: str | None = None) -> None:
        if key is None:
            fresh = Preferences.defaults().values
            self.values.update(fresh)
            return
        setting = SCHEMA_BY_KEY.get(key)
        if setting is None:
            raise KeyError(f"unknown setting: {key}")
        self.values[key] = _copy(setting.default)

    def copy(self) -> Preferences:
        return Preferences(self.path, {k: _copy(v) for k, v in self.values.items()}, dict(self._raw))

    def export(self) -> dict:
        out: dict = {}
        for key, value in self.values.items():
            section, name = key.split(".", 1)
            out.setdefault(section, {})[name] = value
        return out

    def import_dict(self, data: dict) -> list[str]:
        """Apply values from an exported dict. Returns error messages."""
        errors = []
        for section, sec in data.items():
            if not isinstance(sec, dict):
                continue
            for name, value in sec.items():
                key = f"{section}.{name}"
                try:
                    self.set(key, value)
                except (KeyError, ValueError) as exc:
                    errors.append(str(exc))
        return errors


def _copy(v: Any) -> Any:
    return list(v) if isinstance(v, list) else v


def migrate_from_qsettings(prefs: Preferences) -> bool:
    """One-time import of the pre-0.0.7 Qt settings. Returns True if done."""
    if prefs.exists():
        return False
    try:
        from PySide6.QtCore import QSettings
    except ImportError:
        return False
    qs = QSettings()
    mapping = {
        "ui/theme": "appearance.theme",
        "ui/font_family": "appearance.ui_font_family",
        "ui/font_size": "appearance.ui_font_size",
        "ui/zoom_factor": "article.zoom",
        "search/debounce_ms": "search.debounce_ms",
        "ui/show_sidebar": "appearance.show_sidebar",
        "ui/sidebar_width": "state.sidebar_width",
        "search/last": "state.last_search",
    }
    found = False
    for old, new in mapping.items():
        if qs.contains(old):
            found = True
            with contextlib.suppress(KeyError, ValueError):
                prefs.set(new, qs.value(old))
    if found:
        prefs.save()
    return found

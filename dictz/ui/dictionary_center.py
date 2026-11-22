"""The Dictionaries window — everything about dictionaries in one place:
your dictionaries (order by drag and drop, on / off, remove, files, updates),
dictionary folders, the catalog of downloadable dictionaries, sources
(servers), downloads, import / export and the dictionary settings.

Everything heavy (reading sources, downloading, unpacking, building a
catalog) runs in threads; results come back through Qt signals. The logic is
in ``dictz.center`` (also used by the command line and, ported, by the Rust
edition's window).
"""
from __future__ import annotations

import html
import threading
from pathlib import Path

from PySide6.QtCore import (
    QAbstractTableModel,
    QModelIndex,
    QObject,
    Qt,
    QTimer,
    QUrl,
    Signal,
)
from PySide6.QtGui import QColor, QDesktopServices, QGuiApplication
from PySide6.QtWidgets import (
    QAbstractItemView,
    QButtonGroup,
    QCheckBox,
    QComboBox,
    QDialog,
    QDialogButtonBox,
    QFileDialog,
    QFormLayout,
    QGroupBox,
    QHBoxLayout,
    QHeaderView,
    QInputDialog,
    QLabel,
    QLineEdit,
    QMessageBox,
    QPlainTextEdit,
    QProgressBar,
    QPushButton,
    QRadioButton,
    QSpinBox,
    QSplitter,
    QTableView,
    QTableWidget,
    QTableWidgetItem,
    QTabWidget,
    QTextBrowser,
    QToolButton,
    QTreeWidget,
    QTreeWidgetItem,
    QVBoxLayout,
    QWidget,
)

from ..center import adapters, langs, licenses
from ..center.catalog import Catalog
from ..center.install import DownloadQueue, Installer
from ..center.model import human_size, matches, pair_text
from ..center.net import Http
from ..center.sources import AUTH, TYPES, Sources, guess_type
from ..core.i18n import t
from .icons import icon

# the tabs, in order (the main window opens one by number)
TAB_MINE, TAB_FOLDERS, TAB_CATALOG, TAB_SOURCES, TAB_DOWNLOADS, TAB_TRANSFER, TAB_SETTINGS = range(7)

STATUS_COLORS = {"open": "#2e9d5b", "free": "#3b82c4", "unknown": "#c28a1e", "nonfree": "#c2453a"}


def status_text(status: str) -> str:
    return {"open": t("açık lisans"), "free": t("serbest"), "unknown": t("lisans bilinmiyor"),
            "nonfree": t("kişisel kullanım")}.get(status, status)


def license_name(lic) -> str:
    """The licence's name; the few that are not names are translated."""
    return {"unknown": t("Bilinmiyor — kaynak belirtmiyor"), "free": t("Serbest (lisans adı verilmemiş)"),
            "personal": t("Yalnızca kişisel kullanım için serbest"), "public-domain": t("Kamu malı"),
            "freedict": t("FreeDict (GPL, CC BY-SA … — her sözlüğün kendi lisansı)")}.get(lic.id, lic.name)


def kind_text(kind: str) -> str:
    return {"bilingual": t("iki dilli"), "monolingual": t("tek dilli"), "multilingual": t("çok dilli"),
            "unknown": "–"}.get(kind, kind)


def type_text(tp: str) -> str:
    return {"dictz-catalog": t("dictz kataloğu (sunucu)"), "freedict": "FreeDict",
            "github-releases": t("GitHub sürümleri"), "github-folder": t("GitHub klasörü"),
            "http-index": t("Web klasör listesi"), "webdav": "WebDAV", "nextcloud-share": t("Nextcloud paylaşımı"),
            "archive-org": "Internet Archive", "local": t("Yerel klasör"), "telegram": t("Telegram kanalı")}.get(tp, tp)


class _Bridge(QObject):
    """Thread → window."""

    refreshed = Signal(str, object)  # source id, result
    refresh_done = Signal()
    task = Signal(object)  # download task
    installed = Signal(object, object)  # record, entry
    work_done = Signal(str, object)  # what, result or Exception


# ---------------------------------------------------------------- catalog table

COLUMNS = ("", "Ad", "Diller", "Tür", "Biçim", "Kelime", "Boyut", "Lisans", "Kaynak", "Tarih")


class CatalogModel(QAbstractTableModel):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.rows: list[dict] = []
        self.installed: set[str] = set()
        self.updates: set[str] = set()

    def set_rows(self, rows: list[dict]) -> None:
        self.beginResetModel()
        self.rows = rows
        self.endResetModel()

    def rowCount(self, parent=QModelIndex()):  # noqa: N802
        return 0 if parent.isValid() else len(self.rows)

    def columnCount(self, parent=QModelIndex()):  # noqa: N802
        return len(COLUMNS)

    def headerData(self, section, orientation, role=Qt.ItemDataRole.DisplayRole):  # noqa: N802
        if orientation == Qt.Orientation.Horizontal and role == Qt.ItemDataRole.DisplayRole:
            return t(COLUMNS[section]) if COLUMNS[section] else ""
        return None

    def data(self, index, role=Qt.ItemDataRole.DisplayRole):
        e = self.rows[index.row()]
        c = index.column()
        if role == Qt.ItemDataRole.DisplayRole:
            if c == 0:
                return "↑" if e["id"] in self.updates else ("✓" if e["id"] in self.installed else "")
            return [None, e["name"], pair_text(e) or "–", kind_text(e["kind"]),
                    e["format"] + ("" if e["supported"] is not False else " *"),
                    f"{e['headwords']:,}" if e["headwords"] else "–", human_size(e["size"]),
                    licenses.get(e["license"]).id, e["source"], e["date"] or "–"][c]
        if role == Qt.ItemDataRole.ForegroundRole and c == 7:
            return QColor(STATUS_COLORS.get(licenses.get(e["license"]).status, "#888"))
        if role == Qt.ItemDataRole.ToolTipRole:
            if c == 7:
                lic = licenses.get(e["license"])
                return f"{license_name(lic)} — {status_text(lic.status)}"
            if c == 4 and e["supported"] is False:
                return t("dictz bu biçimi henüz açamıyor; indirilebilir, sonraki sürümler açacak")
            if c == 0:
                return t("güncelleme var") if e["id"] in self.updates else (
                    t("kurulu") if e["id"] in self.installed else "")
            return e["name"] if c == 1 else None
        if role == Qt.ItemDataRole.TextAlignmentRole and c in (5, 6):
            return int(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
        return None

    def sort(self, column, order=Qt.SortOrder.AscendingOrder):
        keys = [lambda e: (e["id"] not in self.installed), lambda e: e["name"].lower(), pair_text,
                lambda e: e["kind"], lambda e: e["format"], lambda e: e["headwords"], lambda e: e["size"],
                lambda e: e["license"], lambda e: e["source"], lambda e: e["date"]]
        self.beginResetModel()
        self.rows.sort(key=keys[column], reverse=order == Qt.SortOrder.DescendingOrder)
        self.endResetModel()


# ---------------------------------------------------------------- source editor

class SourceDialog(QDialog):
    """Add / edit a source (server): address, type, sign-in, languages, licence."""

    def __init__(self, sources: Sources, source: dict | None, http: Http, parent=None):
        super().__init__(parent)
        self.sources, self.http = sources, http
        self.source = dict(source or {})
        self.setWindowTitle(t("Kaynağı düzenle") if source else t("Kaynak ekle"))
        self.resize(620, 560)
        lay = QVBoxLayout(self)
        form = QFormLayout()
        s = self.source
        self.name = QLineEdit(s.get("name", ""))
        self.url = QLineEdit(s.get("url", ""))
        self.url.setPlaceholderText("https://…/catalog.json · https://github.com/owner/repo · /path/to/folder")
        self.type = QComboBox()
        for tp in TYPES:
            self.type.addItem(type_text(tp), tp)
        self.type.setCurrentIndex(max(0, TYPES.index(s["type"]) if s.get("type") in TYPES else 0))
        guess = QToolButton()
        guess.setText(t("Tahmin et"))
        guess.clicked.connect(lambda: self.type.setCurrentIndex(TYPES.index(guess_type(self.url.text()))))
        trow = QHBoxLayout()
        trow.addWidget(self.type, 1)
        trow.addWidget(guess)
        self.filter = QLineEdit(s.get("filter", ""))
        self.filter.setPlaceholderText(r"\.(tar\.gz|zip)$")
        self.pattern = QLineEdit(s.get("name_pattern", ""))
        self.pattern.setPlaceholderText(r"^(?P<src>.+?)-(?P<dst>.+?) ")
        self.src = QLineEdit(s.get("source_lang", ""))
        self.dst = QLineEdit(s.get("target_lang", ""))
        self.license = QComboBox()
        self.license.setEditable(True)
        self.license.addItem("", "")
        for lic in licenses.LICENSES:
            self.license.addItem(f"{lic.id} — {lic.name}", lic.id)
        cur = s.get("license", "")
        i = self.license.findData(cur)
        if i >= 0:
            self.license.setCurrentIndex(i)
        else:
            self.license.setEditText(cur)
        self.depth = QSpinBox()
        self.depth.setRange(0, 8)
        self.depth.setValue(int(s.get("depth", 2) or 2))
        self.auth = QComboBox()
        for a in AUTH:
            self.auth.addItem({"none": t("yok"), "basic": t("kullanıcı + parola"), "bearer": t("belirteç (token)"),
                               "header": t("özel başlık")}[a], a)
        self.auth.setCurrentIndex(AUTH.index(s.get("auth", "none")) if s.get("auth") in AUTH else 0)
        self.user = QLineEdit(s.get("user", ""))
        pw, tok = sources.secret(s["id"]) if s.get("id") else ("", "")
        self.password = QLineEdit(pw)
        self.password.setEchoMode(QLineEdit.EchoMode.Password)
        self.token = QLineEdit(tok)
        self.token.setEchoMode(QLineEdit.EchoMode.Password)
        self.description = QLineEdit(s.get("description", ""))
        self.notes = QPlainTextEdit(s.get("notes", ""))
        self.notes.setMaximumHeight(70)
        self.enabled = QCheckBox(t("Açık"))
        self.enabled.setChecked(bool(s.get("enabled", True)))
        form.addRow(t("Ad"), self.name)
        form.addRow(t("Adres"), self.url)
        form.addRow(t("Tür"), trow)
        form.addRow(t("Dosya süzgeci"), self.filter)
        form.addRow(t("Dil kalıbı"), self.pattern)
        lr = QHBoxLayout()
        lr.addWidget(self.src)
        lr.addWidget(QLabel("→"))
        lr.addWidget(self.dst)
        form.addRow(t("Diller (kaynak söylemezse)"), lr)
        form.addRow(t("Lisans (kaynak söylemezse)"), self.license)
        form.addRow(t("Derinlik"), self.depth)
        form.addRow(t("Oturum açma"), self.auth)
        form.addRow(t("Kullanıcı / başlık adı"), self.user)
        form.addRow(t("Parola"), self.password)
        form.addRow(t("Belirteç (token)"), self.token)
        form.addRow(t("Açıklama"), self.description)
        form.addRow(t("Notlar"), self.notes)
        form.addRow("", self.enabled)
        lay.addLayout(form)
        hint = QLabel(t("Parola ve belirteç yalnızca bu bilgisayarda credentials.json içinde tutulur; "
                        "dışa aktarılan listelere asla girmez."))
        hint.setWordWrap(True)
        hint.setObjectName("muted")
        lay.addWidget(hint)
        self.test_label = QLabel("")
        self.test_label.setWordWrap(True)
        test = QPushButton(icon("refresh"), t("Dene"))
        test.clicked.connect(self._test)
        row = QHBoxLayout()
        row.addWidget(test)
        row.addWidget(self.test_label, 1)
        lay.addLayout(row)
        bb = QDialogButtonBox(QDialogButtonBox.StandardButton.Ok | QDialogButtonBox.StandardButton.Cancel)
        bb.accepted.connect(self._ok)
        bb.rejected.connect(self.reject)
        lay.addWidget(bb)

    def values(self) -> dict:
        lic = self.license.currentData() if self.license.currentText() == self.license.itemText(
            self.license.currentIndex()) else self.license.currentText().split(" — ", 1)[0]
        return {"name": self.name.text().strip(), "url": self.url.text().strip(), "type": self.type.currentData(),
                "filter": self.filter.text().strip(), "name_pattern": self.pattern.text().strip(),
                "source_lang": self.src.text().strip(), "target_lang": self.dst.text().strip(),
                "license": lic or "", "depth": self.depth.value(), "auth": self.auth.currentData(),
                "user": self.user.text().strip(), "description": self.description.text().strip(),
                "notes": self.notes.toPlainText().strip(), "enabled": self.enabled.isChecked()}

    def _test(self) -> None:
        from ..center.sources import clean

        src = clean({**self.source, **self.values(), "id": self.source.get("id") or "test"})
        self.test_label.setText(t("Okunuyor…"))
        QGuiApplication.processEvents()
        try:
            n = len(adapters.read(src, self.http, self.password.text(), self.token.text()))
            self.test_label.setText(t("Tamam: {n} sözlük bulundu", n=n))
        except Exception as exc:  # noqa: BLE001
            self.test_label.setText(t("Olmadı:") + f" {exc}")

    def _ok(self) -> None:
        v = self.values()
        if not v["url"]:
            QMessageBox.warning(self, self.windowTitle(), t("Adres gerekli."))
            return
        if self.source.get("id"):
            self.sources.update(self.source["id"], **v)
            sid = self.source["id"]
        else:
            sid = self.sources.add(v)["id"]
        self.sources.set_secret(sid, self.password.text(), self.token.text())
        self.saved_id = sid
        self.accept()


# ---------------------------------------------------------------- the window

class OrderTree(QTreeWidget):
    """Rows reordered by drag and drop; ``reordered(names)`` after a drop
    (the names are in column 0's ``UserRole``)."""

    reordered = Signal(list)

    def __init__(self, parent=None):
        super().__init__(parent)
        self.setRootIsDecorated(False)
        self.setUniformRowHeights(True)
        self.setAlternatingRowColors(True)
        self.setSelectionMode(QAbstractItemView.SelectionMode.ExtendedSelection)
        self.setDragEnabled(True)
        self.setAcceptDrops(True)
        self.setDropIndicatorShown(True)
        self.setDragDropMode(QAbstractItemView.DragDropMode.InternalMove)
        self.setDefaultDropAction(Qt.DropAction.MoveAction)

    def names(self) -> list[str]:
        return [self.topLevelItem(i).data(0, Qt.ItemDataRole.UserRole) for i in range(self.topLevelItemCount())]

    def dropEvent(self, event):  # noqa: N802
        super().dropEvent(event)
        self.reordered.emit(self.names())


class DictionaryCenter(QDialog):
    """Non-modal; one per main window. ``install_files(files)`` is called with
    the dictionary files of each finished install (the main window adds them
    in the background)."""

    install_files = Signal(list)
    removed_files = Signal(list)
    add_paths_requested = Signal(list)  # files to add (in the background)
    add_folder_requested = Signal(str)  # a folder kept in the folder list and scanned now
    rescan_requested = Signal()
    rebuild_requested = Signal(list)  # names read again
    dictionaries_changed = Signal()  # order / on-off / removed: the main window refreshes
    prefs_changed = Signal()

    def __init__(self, data_dir: Path, prefs, parent=None, query_manager=None):
        super().__init__(parent)
        self.data_dir = Path(data_dir)
        self.prefs = prefs
        self.qm = query_manager
        self.setWindowTitle(t("Sözlükler"))
        self.setWindowFlag(Qt.WindowType.WindowMaximizeButtonHint, True)
        self.resize(1180, 760)
        self.sources = Sources(self.data_dir)
        self.http = Http(timeout=float(prefs["center.timeout"]), proxy=str(prefs["center.proxy"]))
        self.catalog = Catalog(self.data_dir, self.sources, self.http)
        root = str(prefs["center.install_folder"]).strip() or None
        self.installer = Installer(self.data_dir, self.sources, self.http, root,
                                   verify=bool(prefs["center.verify_checksums"]),
                                   keep_archives=bool(prefs["center.keep_archives"]))
        self.bridge = _Bridge()
        self.queue = DownloadQueue(self.installer, int(prefs["center.parallel_downloads"]),
                                   on_change=self.bridge.task.emit,
                                   on_installed=self.bridge.installed.emit)
        self.bridge.refreshed.connect(self._on_refreshed)
        self.bridge.refresh_done.connect(self._on_refresh_done)
        self.bridge.task.connect(self._on_task)
        self.bridge.installed.connect(self._on_installed)
        self.bridge.work_done.connect(self._on_work_done)
        self._refreshing = False

        lay = QVBoxLayout(self)
        lay.setContentsMargins(6, 6, 6, 6)
        top = QHBoxLayout()
        self.status = QLabel("")
        self.status.setObjectName("muted")
        link_btn = QPushButton(icon("link"), t("Bağlantıdan ekle…"))
        link_btn.clicked.connect(self.add_from_link)
        self.refresh_btn = QPushButton(icon("refresh"), t("Katalogları yenile"))
        self.refresh_btn.clicked.connect(lambda: self.refresh(None, force=True))
        top.addWidget(self.status, 1)
        top.addWidget(link_btn)
        top.addWidget(self.refresh_btn)
        lay.addLayout(top)
        self.tabs = QTabWidget()
        self.tabs.setDocumentMode(True)
        self._updates: dict[str, dict] = {}  # installed id → newer catalog entry
        self.tabs.addTab(self._build_mine(), icon("dict"), t("Sözlüklerim"))
        self.tabs.addTab(self._build_folders(), icon("folder"), t("Klasörler"))
        self.tabs.addTab(self._build_catalog(), icon("center"), t("Katalog"))
        self.tabs.addTab(self._build_sources(), icon("server"), t("Kaynaklar (sunucular)"))
        self.tabs.addTab(self._build_downloads(), icon("download"), t("İndirmeler"))
        self.tabs.addTab(self._build_transfer(), icon("upload"), t("İçe / Dışa aktar"))
        self.tabs.addTab(self._build_settings(), icon("settings"), t("Ayarlar"))
        lay.addWidget(self.tabs, 1)
        self.reload_all()
        QTimer.singleShot(200, self._auto_refresh)

    # ------------------------------------------------------------ catalog tab
    def _build_catalog(self) -> QWidget:
        w = QWidget()
        lay = QVBoxLayout(w)
        lay.setContentsMargins(0, 4, 0, 0)
        bar = QHBoxLayout()
        self.search = QLineEdit()
        self.search.setPlaceholderText(t("Ara: ad, açıklama, yazar, etiket…"))
        self.search.setClearButtonEnabled(True)
        self.from_combo, self.to_combo = QComboBox(), QComboBox()
        swap = QToolButton()
        swap.setText("⇄")
        swap.setToolTip(t("Dilleri değiştir"))
        swap.clicked.connect(self._swap_langs)
        self.source_combo, self.license_combo, self.format_combo = QComboBox(), QComboBox(), QComboBox()
        for key, label in (("all", t("Tüm lisanslar")), ("open", t("Açık lisans")), ("free", t("Açık + serbest")),
                           ("unknown", t("Lisansı bilinmeyen")), ("nonfree", t("Kişisel kullanım"))):
            self.license_combo.addItem(label, key)
        if not self.prefs["center.show_unknown_license"]:
            self.license_combo.setCurrentIndex(2)
        self.supported_only = QCheckBox(t("Yalnızca açılabilenler"))
        self.hide_installed = QCheckBox(t("Kurulu olanları gizle"))
        for wdg in (self.search,):
            wdg.textChanged.connect(lambda _=None: self._filter_soon())
        for c in (self.from_combo, self.to_combo, self.source_combo, self.license_combo, self.format_combo):
            c.currentIndexChanged.connect(lambda _=None: self._apply_filter())
            c.setMinimumContentsLength(10)
        for c in (self.supported_only, self.hide_installed):
            c.toggled.connect(lambda _=None: self._apply_filter())
        bar.addWidget(self.search, 2)
        bar.addWidget(QLabel(t("Kaynak dil")))
        bar.addWidget(self.from_combo, 1)
        bar.addWidget(swap)
        bar.addWidget(QLabel(t("Hedef dil")))
        bar.addWidget(self.to_combo, 1)
        lay.addLayout(bar)
        bar2 = QHBoxLayout()
        bar2.addWidget(self.source_combo, 1)
        bar2.addWidget(self.license_combo, 1)
        bar2.addWidget(self.format_combo, 1)
        bar2.addWidget(self.supported_only)
        bar2.addWidget(self.hide_installed)
        bar2.addStretch(1)
        lay.addLayout(bar2)
        split = QSplitter(Qt.Orientation.Horizontal)
        self.model = CatalogModel(self)
        self.table = QTableView()
        self.table.setModel(self.model)
        self.table.setSelectionBehavior(QAbstractItemView.SelectionBehavior.SelectRows)
        self.table.setSelectionMode(QAbstractItemView.SelectionMode.ExtendedSelection)
        self.table.setSortingEnabled(True)
        self.table.verticalHeader().setVisible(False)
        self.table.verticalHeader().setDefaultSectionSize(22)
        self.table.setAlternatingRowColors(True)
        hh = self.table.horizontalHeader()
        hh.setSectionResizeMode(QHeaderView.ResizeMode.Interactive)
        hh.setSectionResizeMode(1, QHeaderView.ResizeMode.Stretch)
        for i, wd in ((0, 22), (2, 76), (3, 66), (4, 70), (5, 62), (6, 64), (7, 112), (8, 100), (9, 78)):
            self.table.setColumnWidth(i, wd)
        self.table.selectionModel().selectionChanged.connect(lambda *_: self._show_detail())
        self.table.doubleClicked.connect(lambda _i: self.install_selected())
        split.addWidget(self.table)
        right = QWidget()
        rl = QVBoxLayout(right)
        rl.setContentsMargins(0, 0, 0, 0)
        self.detail = QTextBrowser()
        self.detail.setOpenExternalLinks(True)
        rl.addWidget(self.detail, 1)
        btns = QHBoxLayout()
        self.install_btn = QPushButton(icon("download"), t("Kur"))
        self.install_btn.clicked.connect(self.install_selected)
        self.remove_btn = QPushButton(icon("remove"), t("Kaldır"))
        self.remove_btn.clicked.connect(self._uninstall_selected_catalog)
        page_btn = QPushButton(icon("open"), t("Sayfası"))
        page_btn.clicked.connect(self._open_page)
        copy_btn = QPushButton(icon("copy"), t("Bağlantıyı kopyala"))
        copy_btn.clicked.connect(self._copy_link)
        for b in (self.install_btn, self.remove_btn, page_btn, copy_btn):
            btns.addWidget(b)
        rl.addLayout(btns)
        split.addWidget(right)
        split.setSizes([860, 330])
        lay.addWidget(split, 1)
        self.count_label = QLabel("")
        self.count_label.setObjectName("muted")
        lay.addWidget(self.count_label)
        self._filter_timer = QTimer(self)
        self._filter_timer.setSingleShot(True)
        self._filter_timer.timeout.connect(self._apply_filter)
        return w

    def _filter_soon(self) -> None:
        self._filter_timer.start(150)

    def _swap_langs(self) -> None:
        a, b = self.from_combo.currentData(), self.to_combo.currentData()
        self._set_combo(self.from_combo, b)
        self._set_combo(self.to_combo, a)

    @staticmethod
    def _set_combo(c: QComboBox, data) -> None:
        i = c.findData(data)
        c.setCurrentIndex(max(0, i))

    def _fill_filters(self) -> None:
        entries = self.catalog.entries()
        src, dst = self.catalog.languages()
        fmts = sorted({e["format"] for e in entries})
        for combo, codes, any_label in ((self.from_combo, src, t("Herhangi")), (self.to_combo, dst, t("Herhangi"))):
            cur = combo.currentData()
            combo.blockSignals(True)
            combo.clear()
            combo.addItem(any_label, "")
            counts: dict[str, int] = {}
            for e in entries:
                keys = [e["source_lang"]] if combo is self.from_combo else e["target_lang"]
                for k in keys:
                    counts[k] = counts.get(k, 0) + 1
            for c in sorted(codes, key=lambda x: langs.name(x) or x):
                combo.addItem(f"{langs.label(c)} · {counts.get(c, 0)}", c)
            self._set_combo(combo, cur)
            combo.blockSignals(False)
        cur = self.source_combo.currentData()
        self.source_combo.blockSignals(True)
        self.source_combo.clear()
        self.source_combo.addItem(t("Tüm kaynaklar"), "")
        for s in self.sources.enabled():
            self.source_combo.addItem(s["name"], s["id"])
        self._set_combo(self.source_combo, cur)
        self.source_combo.blockSignals(False)
        cur = self.format_combo.currentData()
        self.format_combo.blockSignals(True)
        self.format_combo.clear()
        self.format_combo.addItem(t("Tüm biçimler"), "")
        for f in fmts:
            self.format_combo.addItem(f, f)
        self._set_combo(self.format_combo, cur)
        self.format_combo.blockSignals(False)

    def _apply_filter(self) -> None:
        inst = self.installer.installed()
        entries = self.catalog.entries()
        f = {"text": self.search.text().strip(), "src": self.from_combo.currentData() or "",
             "dst": self.to_combo.currentData() or "", "source": self.source_combo.currentData() or "",
             "license": self.license_combo.currentData() or "all", "fmt": self.format_combo.currentData() or "",
             "supported_only": self.supported_only.isChecked()}
        rows = [e for e in entries if matches(e, **f) and not (self.hide_installed.isChecked() and e["id"] in inst)]
        self.model.installed = set(inst)
        self.model.updates = {e["id"] for _r, e in self.installer.updates(entries)}
        self.model.set_rows(rows)
        hdr = self.table.horizontalHeader()
        if hdr.sortIndicatorSection() >= 0:
            self.model.sort(hdr.sortIndicatorSection(), hdr.sortIndicatorOrder())
        self.count_label.setText(t("{n} / {m} sözlük gösteriliyor · {k} kurulu", n=len(rows), m=len(entries),
                                   k=len(inst)))
        self._show_detail()

    def _selected(self) -> list[dict]:
        return [self.model.rows[i.row()] for i in self.table.selectionModel().selectedRows()]

    def _show_detail(self) -> None:
        sel = self._selected()
        if not sel:
            self.detail.setHtml(f"<p class='muted'>{html.escape(t('Bir sözlük seçin.'))}</p>")
            return
        if len(sel) > 1:
            total = sum(e["size"] for e in sel)
            self.detail.setHtml(f"<h3>{html.escape(t('{n} sözlük seçili', n=len(sel)))}</h3>"
                                f"<p>{html.escape(t('Toplam boyut'))}: {human_size(total)}</p>")
            return
        self.detail.setHtml(entry_html(sel[0], self.installer.installed().get(sel[0]["id"])))

    def _open_page(self) -> None:
        for e in self._selected()[:5]:
            url = e["page"] or e["homepage"] or (e["files"][0]["url"] if e["files"] else "")
            if url:
                QDesktopServices.openUrl(QUrl(url))

    def _copy_link(self) -> None:
        links = [f["url"] for e in self._selected() for f in e["files"]] or [e["page"] for e in self._selected()]
        QGuiApplication.clipboard().setText("\n".join(x for x in links if x))

    def install_selected(self) -> None:
        sel = self._selected()
        if not sel:
            return
        not_inst = [e for e in sel if not (e["files"] or e["resolve"])]
        todo = [e for e in sel if e["files"] or e["resolve"]]
        if not_inst:
            QMessageBox.information(self, t("Kur"), t("Bunların indirilecek dosyası yok, sayfaları açılıyor:") + "\n"
                                    + "\n".join(e["name"] for e in not_inst[:10]))
            for e in not_inst[:5]:
                if e["page"]:
                    QDesktopServices.openUrl(QUrl(e["page"]))
        unknown = [e for e in todo if licenses.get(e["license"]).status in ("unknown", "nonfree")]
        if unknown and QMessageBox.question(
                self, t("Lisans"),
                t("{n} sözlüğün lisansı bilinmiyor ya da yalnızca kişisel kullanım için. Kullanım hakkı sizde mi? "
                  "Yine de indirilsin mi?", n=len(unknown))) != QMessageBox.StandardButton.Yes:
            todo = [e for e in todo if e not in unknown]
        unsupported = [e for e in todo if e["supported"] is False]
        if unsupported and QMessageBox.question(
                self, t("Biçim"),
                t("{n} sözlüğün biçimini dictz henüz açamıyor ({f}). Dosyalar indirilip saklansın mı?",
                  n=len(unsupported), f=", ".join(sorted({e['format'] for e in unsupported})))
        ) != QMessageBox.StandardButton.Yes:
            todo = [e for e in todo if e not in unsupported]
        if todo and not self.prefs["center.online"] and any(
                not f["url"].startswith("file:") for e in todo for f in e["files"]):
            QMessageBox.warning(self, t("Kur"), t("İnternet kaynakları kapalı (Sözlükler → Ayarlar)."))
            return
        for e in todo:
            self.queue.add(e)
        if todo:
            self.tabs.setCurrentIndex(TAB_DOWNLOADS)

    def _uninstall_selected_catalog(self) -> None:
        ids = [e["id"] for e in self._selected() if e["id"] in self.installer.installed()]
        self.uninstall(ids)

    # ------------------------------------------------------------ sources tab
    def _build_sources(self) -> QWidget:
        w = QWidget()
        lay = QVBoxLayout(w)
        lay.setContentsMargins(0, 4, 0, 0)
        info = QLabel(t("Sözlükler bu kaynaklardan okunur; sıra katalogdaki sırayı belirler. Varsayılan sunucu (★) "
                        "dictz'in kendi kataloğudur. Her adres değiştirilebilir; kendi sunucunuzu, bir klasörü ya da "
                        "bir paylaşım bağlantısını ekleyebilirsiniz."))
        info.setWordWrap(True)
        info.setObjectName("muted")
        lay.addWidget(info)
        self.src_table = QTableWidget(0, 7)
        self.src_table.setHorizontalHeaderLabels([t("Açık"), t("Ad"), t("Tür"), t("Sözlük"), t("Son okuma"),
                                                  t("Durum"), t("Adres")])
        self.src_table.verticalHeader().setVisible(False)
        self.src_table.verticalHeader().setDefaultSectionSize(22)
        self.src_table.setSelectionBehavior(QAbstractItemView.SelectionBehavior.SelectRows)
        self.src_table.setSelectionMode(QAbstractItemView.SelectionMode.SingleSelection)
        self.src_table.setEditTriggers(QAbstractItemView.EditTrigger.NoEditTriggers)
        hh = self.src_table.horizontalHeader()
        hh.setSectionResizeMode(6, QHeaderView.ResizeMode.Stretch)
        for i, wd in ((0, 44), (1, 230), (2, 150), (3, 64), (4, 90), (5, 220)):
            self.src_table.setColumnWidth(i, wd)
        self.src_table.itemChanged.connect(self._src_item_changed)
        self.src_table.doubleClicked.connect(lambda _i: self._edit_source())
        lay.addWidget(self.src_table, 1)
        row = QHBoxLayout()
        for text, ic, fn in ((t("Ekle…"), "new", self._add_source), (t("Düzenle…"), "settings", self._edit_source),
                             (t("Sil"), "remove", self._remove_source), (t("Yukarı"), "up", lambda: self._move_source(-1)),
                             (t("Aşağı"), "down", lambda: self._move_source(1)),
                             (t("Varsayılan sunucu yap"), "star", self._default_source),
                             (t("Yenile"), "refresh", self._refresh_selected_source)):
            b = QPushButton(icon(ic), text)
            b.clicked.connect(fn)
            row.addWidget(b)
        row.addStretch(1)
        for text, ic, fn in ((t("Dışa aktar…"), "upload", self._export_sources),
                             (t("İçe aktar…"), "download", self._import_sources),
                             (t("Varsayılanlara dön"), "refresh", self._reset_sources)):
            b = QPushButton(icon(ic), text)
            b.clicked.connect(fn)
            row.addWidget(b)
        lay.addLayout(row)
        return w

    def _fill_sources(self) -> None:
        self.src_table.blockSignals(True)
        rows = self.sources.all()
        default = self.sources.default_id()
        self.src_table.setRowCount(len(rows))
        for i, s in enumerate(rows):
            c = self.catalog.cached(s["id"])
            on = QTableWidgetItem()
            on.setFlags(Qt.ItemFlag.ItemIsUserCheckable | Qt.ItemFlag.ItemIsEnabled | Qt.ItemFlag.ItemIsSelectable)
            on.setCheckState(Qt.CheckState.Checked if s["enabled"] else Qt.CheckState.Unchecked)
            on.setData(Qt.ItemDataRole.UserRole, s["id"])
            state = t("tamam") if c["ok"] else (t("hata") + ": " + c["error"] if c["error"] else t("hiç okunmadı"))
            lic = licenses.get(s["license"]).id if s["license"] else ""
            vals = [("★ " if s["id"] == default else "") + s["name"], type_text(s["type"]), str(len(c["entries"])),
                    c["fetched"][:10] or "–", state, s["url"]]
            self.src_table.setItem(i, 0, on)
            for j, v in enumerate(vals, start=1):
                it = QTableWidgetItem(v)
                if j == 5 and c["error"]:
                    it.setForeground(QColor("#c2453a"))
                tip = s["description"] + (f"\n{t('Lisans')}: {lic}" if lic else "") + \
                    (f"\n{s['homepage']}" if s["homepage"] else "")
                it.setToolTip(tip.strip() if j != 5 else v)
                self.src_table.setItem(i, j, it)
        self.src_table.blockSignals(False)

    def _src_item_changed(self, item: QTableWidgetItem) -> None:
        if item.column() == 0:
            self.sources.set_enabled(item.data(Qt.ItemDataRole.UserRole),
                                     item.checkState() == Qt.CheckState.Checked)
            self._fill_filters()
            self._apply_filter()

    def _current_source(self) -> dict | None:
        r = self.src_table.currentRow()
        if r < 0:
            return None
        return self.sources.get(self.src_table.item(r, 0).data(Qt.ItemDataRole.UserRole))

    def _add_source(self, url: str = "") -> None:
        d = SourceDialog(self.sources, {"url": url, "type": guess_type(url)} if url else None, self.http, self)
        if d.exec() == QDialog.DialogCode.Accepted:
            self._fill_sources()
            self.refresh([d.saved_id], force=True)

    def _edit_source(self) -> None:
        s = self._current_source()
        if s is None:
            return
        d = SourceDialog(self.sources, s, self.http, self)
        if d.exec() == QDialog.DialogCode.Accepted:
            self._fill_sources()
            self.refresh([s["id"]], force=True)

    def _remove_source(self) -> None:
        s = self._current_source()
        if s and QMessageBox.question(self, t("Sil"), t("“{name}” kaynağı silinsin mi? (Kurulu sözlükler kalır.)",
                                                        name=s["name"])) == QMessageBox.StandardButton.Yes:
            self.sources.remove(s["id"])
            self.reload_all()

    def _move_source(self, delta: int) -> None:
        s = self._current_source()
        if s:
            r = self.src_table.currentRow()
            self.sources.move(s["id"], delta)
            self._fill_sources()
            self.src_table.selectRow(max(0, min(self.src_table.rowCount() - 1, r + delta)))
            self._apply_filter()

    def _default_source(self) -> None:
        s = self._current_source()
        if s:
            self.sources.set_default(s["id"])
            self._fill_sources()

    def _refresh_selected_source(self) -> None:
        s = self._current_source()
        if s:
            self.refresh([s["id"]], force=True)

    def _export_sources(self) -> None:
        path, _ = QFileDialog.getSaveFileName(self, t("Kaynak listesini dışa aktar"), "dictz-sources.json",
                                              "JSON (*.json)")
        if path:
            n = self.sources.export(path)
            self.status.setText(t("{n} kaynak dışa aktarıldı", n=n))

    def _import_sources(self) -> None:
        path, _ = QFileDialog.getOpenFileName(self, t("Kaynak listesini içe aktar"), "", "JSON (*.json)")
        if not path:
            return
        try:
            n = self.sources.import_(path)
        except ValueError as e:
            QMessageBox.warning(self, t("İçe aktar"), str(e))
            return
        self.status.setText(t("{n} kaynak içe aktarıldı", n=n))
        self.reload_all()

    def _reset_sources(self) -> None:
        if QMessageBox.question(self, t("Varsayılanlara dön"),
                                t("Yerleşik kaynaklar ilk hallerine dönsün mü? (Sizin eklediğiniz kaynaklar kalır.)")
                                ) == QMessageBox.StandardButton.Yes:
            self.sources.reset()
            self.reload_all()

    # ------------------------------------------------------------ downloads tab
    def _build_downloads(self) -> QWidget:
        w = QWidget()
        lay = QVBoxLayout(w)
        lay.setContentsMargins(0, 4, 0, 0)
        self.dl_table = QTableWidget(0, 6)
        self.dl_table.setHorizontalHeaderLabels([t("Ad"), t("Durum"), t("İlerleme"), t("Boyut"), t("Hız"), t("Hata")])
        self.dl_table.verticalHeader().setVisible(False)
        self.dl_table.setSelectionBehavior(QAbstractItemView.SelectionBehavior.SelectRows)
        self.dl_table.setEditTriggers(QAbstractItemView.EditTrigger.NoEditTriggers)
        self.dl_table.horizontalHeader().setSectionResizeMode(5, QHeaderView.ResizeMode.Stretch)
        for i, wd in ((0, 300), (1, 100), (2, 180), (3, 90), (4, 90)):
            self.dl_table.setColumnWidth(i, wd)
        lay.addWidget(self.dl_table, 2)
        row = QHBoxLayout()
        for text, ic, fn in ((t("Duraklat"), "close", self.queue.pause), (t("Sürdür"), "download", self.queue.resume),
                             (t("İptal"), "remove", self.queue.cancel),
                             (t("Yeniden dene"), "refresh", self.queue.resume)):
            b = QPushButton(icon(ic), text)
            b.clicked.connect(lambda _=False, f=fn: [f(tid) for tid in self._selected_tasks()])
            row.addWidget(b)
        clear = QPushButton(t("Bitenleri temizle"))
        clear.clicked.connect(lambda: (self.queue.clear_finished(), self._fill_downloads()))
        row.addWidget(clear)
        row.addStretch(1)
        lay.addLayout(row)
        hist = QGroupBox(t("Geçmiş"))
        hl = QVBoxLayout(hist)
        self.hist_table = QTableWidget(0, 5)
        self.hist_table.setHorizontalHeaderLabels([t("Zaman"), t("Durum"), t("Ad"), t("Lisans"), t("Hata")])
        self.hist_table.verticalHeader().setVisible(False)
        self.hist_table.setEditTriggers(QAbstractItemView.EditTrigger.NoEditTriggers)
        self.hist_table.horizontalHeader().setSectionResizeMode(4, QHeaderView.ResizeMode.Stretch)
        for i, wd in ((0, 140), (1, 80), (2, 300), (3, 130)):
            self.hist_table.setColumnWidth(i, wd)
        hl.addWidget(self.hist_table)
        hc = QPushButton(t("Geçmişi temizle"))
        hc.clicked.connect(lambda: (self.installer.clear_history(), self._fill_history()))
        hl.addWidget(hc, 0, Qt.AlignmentFlag.AlignRight)
        lay.addWidget(hist, 1)
        return w

    def _selected_tasks(self) -> list[str]:
        out = []
        for idx in self.dl_table.selectionModel().selectedRows():
            it = self.dl_table.item(idx.row(), 0)
            if it is not None:
                out.append(it.data(Qt.ItemDataRole.UserRole))
        return out

    def _state_text(self, task: dict) -> str:
        if task["state"] == "running":
            return {"download": t("iniyor"), "check": t("denetleniyor"), "unpack": t("açılıyor")}.get(
                task["stage"], t("iniyor"))
        return {"queued": t("sırada"), "paused": t("duraklatıldı"), "done": t("kuruldu"), "failed": t("olmadı"),
                "cancelled": t("iptal edildi")}.get(task["state"], task["state"])

    def _fill_downloads(self) -> None:
        tasks = self.queue.tasks
        self.dl_table.setRowCount(len(tasks))
        for i, task in enumerate(tasks):
            name = QTableWidgetItem(task["entry"]["name"])
            name.setData(Qt.ItemDataRole.UserRole, task["id"])
            self.dl_table.setItem(i, 0, name)
            self.dl_table.setItem(i, 1, QTableWidgetItem(self._state_text(task)))
            bar = self.dl_table.cellWidget(i, 2)
            if not isinstance(bar, QProgressBar):
                bar = QProgressBar()
                bar.setTextVisible(True)
                bar.setMaximumHeight(16)
                self.dl_table.setCellWidget(i, 2, bar)
            if task["total"]:
                bar.setRange(0, 1000)
                bar.setValue(int(task["done"] * 1000 / max(1, task["total"])) if task["state"] != "done" else 1000)
            else:
                bar.setRange(0, 0 if task["state"] == "running" else 1000)
                bar.setValue(1000 if task["state"] == "done" else 0)
            self.dl_table.setItem(i, 3, QTableWidgetItem(human_size(task["total"] or task["done"])))
            speed = human_size(int(task["speed"])) + "/s" if task["state"] == "running" and task["speed"] else ""
            self.dl_table.setItem(i, 4, QTableWidgetItem(speed))
            err = QTableWidgetItem(task["error"])
            err.setToolTip(task["error"])
            err.setForeground(QColor("#c2453a"))
            self.dl_table.setItem(i, 5, err)
        active = self.queue.active()
        self.tabs.setTabText(TAB_DOWNLOADS, t("İndirmeler") + (f" ({active})" if active else ""))

    def _fill_history(self) -> None:
        rows = self.installer.history()[:300]
        self.hist_table.setRowCount(len(rows))
        for i, h in enumerate(rows):
            vals = [h.get("finished", "")[:16].replace("T", " "), {"done": t("kuruldu"), "failed": t("olmadı")}.get(
                h.get("status", ""), h.get("status", "")), h.get("name", ""), h.get("license", ""), h.get("error", "")]
            for j, v in enumerate(vals):
                it = QTableWidgetItem(v)
                it.setToolTip(v)
                self.hist_table.setItem(i, j, it)

    def _on_task(self, task: dict) -> None:
        self._fill_downloads()
        if task["state"] in ("done", "failed"):
            self._fill_history()

    def _on_installed(self, rec: dict, entry: dict) -> None:
        if rec["files"] and self.prefs["center.add_after_install"]:
            self.install_files.emit(list(rec["files"]))
        self.status.setText(t("Kuruldu: {name}", name=entry["name"]) + (
            "" if rec["files"] else " — " + t("dictz bu biçimi henüz açamıyor; dosyalar klasörde saklandı")))
        self._compute_updates()
        self._apply_filter()

    # ------------------------------------------------------------ my dictionaries tab
    MINE_COLS = ("", "Ad", "Biçim", "Kelime", "Diller", "Kaynak", "Lisans", "Güncelleme", "Dosya")

    def _build_mine(self) -> QWidget:
        w = QWidget()
        lay = QVBoxLayout(w)
        lay.setContentsMargins(0, 4, 0, 0)
        hint = QLabel(t("Arama sonuçlarındaki sıra budur: satırları sürükleyip bırakarak sıralayın. "
                        "Kutu = etkin. Çift tık: bilgiler."))
        hint.setObjectName("muted")
        hint.setWordWrap(True)
        lay.addWidget(hint)
        self.mine = OrderTree()
        self.mine.setColumnCount(len(self.MINE_COLS))
        self.mine.setHeaderLabels([t(c) for c in self.MINE_COLS])
        hh = self.mine.header()
        hh.setStretchLastSection(True)
        for i, wd in ((0, 34), (1, 280), (2, 70), (3, 90), (4, 90), (5, 110), (6, 120), (7, 90)):
            self.mine.setColumnWidth(i, wd)
        self.mine.itemChanged.connect(self._mine_item_changed)
        self.mine.itemDoubleClicked.connect(lambda _it, _c: self._mine_info())
        self.mine.reordered.connect(self._mine_reordered)
        self.mine.setContextMenuPolicy(Qt.ContextMenuPolicy.CustomContextMenu)
        self.mine.customContextMenuRequested.connect(self._mine_menu)
        lay.addWidget(self.mine, 1)
        row = QHBoxLayout()
        for text, ic, fn in ((t("Dosya ekle…"), "open", self._mine_add_files),
                             (t("Klasör ekle…"), "folder", self._mine_add_folder),
                             (t("Bağlantıdan ekle…"), "link", self.add_from_link),
                             (t("Kaldır…"), "remove", self._mine_remove)):
            b = QPushButton(icon(ic), text)
            b.clicked.connect(fn)
            row.addWidget(b)
        row.addSpacing(10)
        for text, ic, where in ((t("En üste"), "up", "top"), (t("Yukarı"), "up", -1), (t("Aşağı"), "down", 1),
                                (t("En alta"), "down", "bottom")):
            b = QPushButton(icon(ic), text)
            b.clicked.connect(lambda _=False, d=where: self._mine_move(d))
            row.addWidget(b)
        row.addSpacing(10)
        for text, ic, fn in ((t("Bilgi"), "help", self._mine_info),
                             (t("Yeniden indeksle"), "refresh", self._mine_reindex),
                             (t("Klasörü aç"), "folder", self._mine_open_folder),
                             (t("Güncelle"), "download", self._update_selected),
                             (t("Hepsini güncelle"), "refresh", self._update_all)):
            b = QPushButton(icon(ic), text)
            b.clicked.connect(fn)
            row.addWidget(b)
        row.addStretch(1)
        lay.addLayout(row)
        self.mine_summary = QLabel("")
        self.mine_summary.setObjectName("hint")
        lay.addWidget(self.mine_summary)
        return w

    def refresh_dictionaries(self) -> None:
        """Fill "Sözlüklerim" again (the main window calls it after a change)."""
        if self.qm is None:
            return
        from ..storage.registry import norm_path

        qm = self.qm
        selected = set(self._mine_selected())
        recs = {}
        for rid, rec in self.installer.installed().items():
            for f in rec.get("files") or []:
                recs[norm_path(f)] = (rid, rec)
        src_names = {s["id"]: s["name"] for s in self.sources.all()}
        self.mine.blockSignals(True)
        self.mine.clear()
        rows = qm.list_rows()
        for row in rows:
            name = row["name"]
            rid, rec = recs.get(norm_path(row["path"]), (None, None))
            up = self._updates.get(rid) if rid else None
            if row["loaded"]:
                m = qm.dictionaries[name].metadata
                langs_text = f"{m.source_language} → {m.target_language}" if m.source_language else ""
                label = name
            else:
                langs_text, label = "", f"⚠ {name} — {t('açılamadı')}"
            vals = ["", label, row["format"], f"{row['word_count']:,}", langs_text,
                    src_names.get(rec.get("source", ""), rec.get("source", "")) if rec else t("yerel"),
                    rec.get("license", "") if rec else "", (up["version"] or up["date"]) if up else "", row["path"]]
            it = QTreeWidgetItem(vals)
            it.setData(0, Qt.ItemDataRole.UserRole, name)
            it.setData(1, Qt.ItemDataRole.UserRole, rid or "")
            it.setFlags(Qt.ItemFlag.ItemIsSelectable | Qt.ItemFlag.ItemIsEnabled | Qt.ItemFlag.ItemIsUserCheckable
                        | (Qt.ItemFlag.ItemIsDragEnabled if row["loaded"] else Qt.ItemFlag.NoItemFlags))
            it.setCheckState(0, Qt.CheckState.Checked if row["enabled"] else Qt.CheckState.Unchecked)
            if row["loaded"]:
                from . import dicticons

                it.setIcon(1, dicticons.qicon(name))
                it.setToolTip(1, dicticons.look(name).tooltip())
            else:
                for c in range(len(vals)):
                    it.setForeground(c, QColor("#dc2626"))
                    it.setToolTip(c, row["error"])
            it.setTextAlignment(3, Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
            if up:
                it.setForeground(7, QColor("#2e9d5b"))
            it.setToolTip(8, row["path"])
            self.mine.addTopLevelItem(it)
            it.setSelected(name in selected)
        self.mine.blockSignals(False)
        enabled = [r for r in rows if r["enabled"] and r["loaded"]]
        words = sum(r["word_count"] for r in enabled)
        self.mine_summary.setText(t("{n} / {m} sözlük etkin · {w} kelime", n=len(enabled), m=len(rows), w=f"{words:,}"))
        self.tabs.setTabText(TAB_MINE, t("Sözlüklerim") + f" ({len(rows)})")

    def select_dictionary(self, name: str) -> None:
        """Select ``name`` in "Sözlüklerim" (and show it)."""
        for i in range(self.mine.topLevelItemCount()):
            it = self.mine.topLevelItem(i)
            it.setSelected(it.data(0, Qt.ItemDataRole.UserRole) == name)
            if it.isSelected():
                self.mine.scrollToItem(it)

    def _mine_selected(self) -> list[str]:
        return [it.data(0, Qt.ItemDataRole.UserRole) for it in self.mine.selectedItems()]

    def _mine_item_changed(self, item: QTreeWidgetItem, column: int) -> None:
        if column != 0 or self.qm is None:
            return
        self.qm.set_enabled(item.data(0, Qt.ItemDataRole.UserRole), item.checkState(0) == Qt.CheckState.Checked)
        self.dictionaries_changed.emit()

    def _mine_reordered(self, names: list[str]) -> None:
        if self.qm is None:
            return
        self.qm.set_order([n for n in names if n in self.qm.dictionaries])
        self.dictionaries_changed.emit()

    def _mine_move(self, where) -> None:
        names = [n for n in self._mine_selected() if self.qm is not None and n in self.qm.dictionaries]
        if not names:
            return
        order = self.qm.loaded_names()
        if where == "top":
            order = names + [n for n in order if n not in names]
        elif where == "bottom":
            order = [n for n in order if n not in names] + names
        else:
            for n in (names if where < 0 else reversed(names)):
                i = order.index(n)
                j = max(0, min(len(order) - 1, i + where))
                order.insert(j, order.pop(i))
        self.qm.set_order(order)
        self.dictionaries_changed.emit()

    def _mine_menu(self, pos) -> None:
        from PySide6.QtWidgets import QMenu

        if not self.mine.itemAt(pos):
            return
        menu = QMenu(self)
        menu.addAction(icon("help"), t("Bilgi"), self._mine_info)
        menu.addAction(t("Yalnızca bunu etkinleştir"), self._mine_solo)
        menu.addAction(icon("refresh"), t("Yeniden indeksle"), self._mine_reindex)
        menu.addAction(icon("folder"), t("Klasörü aç"), self._mine_open_folder)
        menu.addSeparator()
        menu.addAction(icon("up"), t("En üste"), lambda: self._mine_move("top"))
        menu.addAction(icon("down"), t("En alta"), lambda: self._mine_move("bottom"))
        menu.addSeparator()
        menu.addAction(icon("remove"), t("Kaldır…"), self._mine_remove)
        menu.exec(self.mine.viewport().mapToGlobal(pos))

    def _mine_solo(self) -> None:
        names = set(self._mine_selected())
        if self.qm is None or not names:
            return
        for n in self.qm.loaded_names():
            self.qm.set_enabled(n, n in names)
        self.dictionaries_changed.emit()

    def _mine_add_files(self) -> None:
        from .dictionary_manager import file_filter

        paths, _ = QFileDialog.getOpenFileNames(self, t("Sözlük dosyası seçin"), "", file_filter())
        if paths:
            self.add_paths_requested.emit([Path(p) for p in paths])

    def _mine_add_folder(self) -> None:
        d = QFileDialog.getExistingDirectory(self, t("Sözlük klasörü seçin"))
        if d:
            self.add_folder_requested.emit(d)

    def _mine_info(self) -> None:
        names = self._mine_selected()
        if not names or self.qm is None:
            return
        name = names[0]
        if name not in self.qm.dictionaries:
            QMessageBox.warning(self, name, self.qm.failed.get(name, ""))
            return
        from .dictionary_manager import info_html

        QMessageBox.information(self, name, info_html(self.qm, name))

    def _mine_reindex(self) -> None:
        names = [n for n in self._mine_selected() if self.qm is not None and n in self.qm.dictionaries]
        if names:
            self.rebuild_requested.emit(names)

    def _mine_open_folder(self) -> None:
        if self.qm is None:
            return
        paths = {r["name"]: r["path"] for r in self.qm.list_rows()}
        for n in self._mine_selected()[:3]:
            if paths.get(n):
                QDesktopServices.openUrl(QUrl.fromLocalFile(str(Path(paths[n]).parent)))

    def _mine_remove(self) -> None:
        """Remove from the list, or delete with the files: a catalog install
        loses its folder, an own dictionary only the files it is made of
        (listed in the question)."""
        from ..center.transfer import own_files
        from ..storage.registry import norm_path

        names = self._mine_selected()
        if not names or self.qm is None:
            return
        rows = {r["name"]: r for r in self.qm.list_rows()}
        recs = {}
        for rid, rec in self.installer.installed().items():
            for f in rec.get("files") or []:
                recs[norm_path(f)] = rid
        files: list[str] = []
        for n in names:
            p = rows.get(n, {}).get("path", "")
            if p and norm_path(p) not in recs:
                files += [str(f) for f in own_files(p)]
        box = QMessageBox(self)
        box.setIcon(QMessageBox.Icon.Question)
        box.setWindowTitle(t("Kaldır"))
        text = t("Bu sözlükler dictz'ten kaldırılsın mı?") + "\n\n" + "\n".join(names[:20])
        if files:
            text += "\n\n" + t("“Dosyalarıyla sil” şunları da siler:") + "\n" + "\n".join(files[:20])
        box.setText(text)
        only = box.addButton(t("Listeden kaldır"), QMessageBox.ButtonRole.AcceptRole)
        delete = box.addButton(t("Dosyalarıyla sil"), QMessageBox.ButtonRole.DestructiveRole)
        box.addButton(t("İptal"), QMessageBox.ButtonRole.RejectRole)
        box.exec()
        if box.clickedButton() not in (only, delete):
            return
        with_files = box.clickedButton() is delete
        problems = []
        for n in names:
            p = rows.get(n, {}).get("path", "")
            rid = recs.get(norm_path(p)) if p else None
            own = own_files(p) if p and with_files and not rid else []
            self.qm.remove_dictionary(n)  # closed first: Windows cannot delete an open file
            if rid:
                self.installer.uninstall(rid, delete_files=with_files)
            for f in own:
                try:
                    f.unlink()
                except OSError as e:
                    problems.append(f"{f}: {e}")
        self.dictionaries_changed.emit()
        self._apply_filter()
        if problems:
            QMessageBox.warning(self, t("Kaldır"), "\n".join(problems[:20]))

    def _compute_updates(self) -> None:
        self._updates = {r["id"]: e for r, e in self.installer.updates(self.catalog.entries(include_disabled=True))}

    def _update_selected(self) -> None:
        ids = {self.mine.topLevelItem(i).data(1, Qt.ItemDataRole.UserRole)
               for i in range(self.mine.topLevelItemCount()) if self.mine.topLevelItem(i).isSelected()}
        todo = [e for rid, e in self._updates.items() if rid in ids]
        if not todo:
            QMessageBox.information(self, t("Güncelle"), t("Kurulu sözlüklerin hepsi güncel."))
            return
        for e in todo:
            self.queue.add(e)
        self.tabs.setCurrentIndex(TAB_DOWNLOADS)

    def _update_all(self) -> None:
        self._compute_updates()
        if not self._updates:
            QMessageBox.information(self, t("Güncelle"), t("Kurulu sözlüklerin hepsi güncel."))
            return
        for e in self._updates.values():
            self.queue.add(e)
        self.tabs.setCurrentIndex(TAB_DOWNLOADS)

    def uninstall(self, ids: list[str]) -> None:
        """Remove catalog installs (from the catalog tab's "Kaldır")."""
        if not ids:
            return
        recs = self.installer.installed()
        names = "\n".join(recs[i]["name"] for i in ids if i in recs)
        msg = t("Bu sözlükler dictz'ten kaldırılsın mı?") + "\n\n" + names + "\n\n" + t("İndirilen dosyalar da silinecek.")
        if QMessageBox.question(self, t("Kaldır"), msg) != QMessageBox.StandardButton.Yes:
            return
        files = []
        for i in ids:
            rec = recs.get(i)
            if rec:
                files += rec.get("files") or []
        self.removed_files.emit(files)  # the window drops them from the list first (Windows: closes them)
        for i in ids:
            self.installer.uninstall(i, delete_files=True)
        self._apply_filter()
        self.refresh_dictionaries()

    # ------------------------------------------------------------ folders tab
    def _build_folders(self) -> QWidget:
        from ..core.query_manager import folder_sources
        from .folder_sources import FolderSources

        w = QWidget()
        lay = QVBoxLayout(w)
        lay.setContentsMargins(0, 4, 0, 0)
        hint = QLabel(t("Açılışta bu klasörlerdeki .ifo / .dsl / .dsl.dz / .bgl dosyaları otomatik eklenir.") + " "
                      + t("Kutu = alt klasörleri de tara. Bir klasörü kaldırmak sözlüklerini listeden çıkarmaz."))
        hint.setObjectName("muted")
        hint.setWordWrap(True)
        lay.addWidget(hint)
        self.folders = FolderSources(folder_sources(self.prefs), bool(self.prefs["dictionaries.recursive"]))
        self.folders.changed.connect(self._folders_changed)
        self.folders.rescan_requested.connect(self.rescan_requested.emit)
        lay.addWidget(self.folders, 1)
        return w

    def _folders_changed(self) -> None:
        from ..core.query_manager import folder_sources, set_folder_sources

        old = {f for f, _ in folder_sources(self.prefs)}
        set_folder_sources(self.prefs, self.folders.value())
        self.prefs.save()
        for f, _rec in self.folders.value():
            if f not in old:
                self.add_folder_requested.emit(f)

    def refresh_folders(self) -> None:
        from ..core.query_manager import folder_sources

        self.folders.set_value(folder_sources(self.prefs))

    # ------------------------------------------------------------ settings tab
    def _build_settings(self) -> QWidget:
        from .preferences_dialog import PAGES_IN_DICTIONARIES_WINDOW, SettingsForm

        self.settings_form = SettingsForm(self.prefs, PAGES_IN_DICTIONARIES_WINDOW, skip=("dictionaries.folders",))
        self.settings_form.changed.connect(self.prefs_changed.emit)
        return self.settings_form

    # ------------------------------------------------------------ import / export tab
    def _build_transfer(self) -> QWidget:
        w = QWidget()
        lay = QVBoxLayout(w)
        lay.setContentsMargins(4, 6, 4, 4)
        # export
        ex = QGroupBox(t("Kurulumu dışa aktar (başka bilgisayara taşımak için)"))
        el = QVBoxLayout(ex)
        self.ex_parts: dict[str, QCheckBox] = {}
        row = QHBoxLayout()
        for key, label, on in (("dictionaries", t("Sözlük listesi"), True), ("settings", t("Ayarlar"), True),
                               ("sources", t("Kaynaklar (parolasız)"), True), ("installed", t("Kurulum kayıtları"), True),
                               ("history", t("Geçmiş ve favoriler"), True), ("files", t("Sözlük dosyaları"), False)):
            cb = QCheckBox(label)
            cb.setChecked(on)
            self.ex_parts[key] = cb
            row.addWidget(cb)
        row.addStretch(1)
        el.addLayout(row)
        b = QPushButton(icon("upload"), t("Dışa aktar…"))
        b.clicked.connect(self._export_setup)
        el.addWidget(b, 0, Qt.AlignmentFlag.AlignLeft)
        lay.addWidget(ex)
        # import
        im = QGroupBox(t("Kurulumu içe aktar"))
        il = QHBoxLayout(im)
        self.im_merge = QRadioButton(t("Birleştir (var olanlar kalır)"))
        self.im_replace = QRadioButton(t("Değiştir (liste, kaynaklar ve geçmiş arşivdeki olur)"))
        self.im_merge.setChecked(True)
        grp = QButtonGroup(im)
        grp.addButton(self.im_merge)
        grp.addButton(self.im_replace)
        il.addWidget(self.im_merge)
        il.addWidget(self.im_replace)
        b = QPushButton(icon("download"), t("İçe aktar…"))
        b.clicked.connect(self._import_setup)
        il.addStretch(1)
        il.addWidget(b)
        lay.addWidget(im)
        # list
        li = QGroupBox(t("Sözlük listesini dışa aktar (lisanslarıyla)"))
        ll = QHBoxLayout(li)
        for fmt in ("csv", "json", "txt"):
            b = QPushButton(fmt.upper() + "…")
            b.clicked.connect(lambda _=False, f=fmt: self._export_list(f))
            ll.addWidget(b)
        ll.addStretch(1)
        lay.addWidget(li)
        # catalog build
        cb = QGroupBox(t("Kendi sunucun için katalog oluştur"))
        cl = QFormLayout(cb)
        self.cb_folder = QLineEdit()
        pick = QToolButton()
        pick.setIcon(icon("folder"))
        pick.clicked.connect(lambda: self.cb_folder.setText(
            QFileDialog.getExistingDirectory(self, t("Sözlüklerin klasörü")) or self.cb_folder.text()))
        fr = QHBoxLayout()
        fr.addWidget(self.cb_folder, 1)
        fr.addWidget(pick)
        self.cb_url = QLineEdit("https://raw.githubusercontent.com/bayramkotan/dictz-data/main/")
        self.cb_name = QLineEdit("dictz-data")
        self.cb_license = QComboBox()
        self.cb_license.addItem(t("(her sözlüğün kendi lisansı)"), "")
        for lic in licenses.LICENSES:
            self.cb_license.addItem(f"{lic.id} — {lic.name}", lic.id)
        cl.addRow(t("Klasör"), fr)
        cl.addRow(t("Dosyaların adresi"), self.cb_url)
        cl.addRow(t("Katalog adı"), self.cb_name)
        cl.addRow(t("Varsayılan lisans"), self.cb_license)
        brow = QHBoxLayout()
        b = QPushButton(icon("save"), t("catalog.json oluştur"))
        b.clicked.connect(self._build_catalog_file)
        c = QPushButton(icon("license"), t("Kataloğu denetle…"))
        c.clicked.connect(self._check_catalog_file)
        brow.addWidget(b)
        brow.addWidget(c)
        brow.addStretch(1)
        cl.addRow("", brow)
        hint = QLabel(t("Klasördeki her sözlük için boyut, SHA-256, diller, kelime sayısı ve lisans yazılır. "
                        "Bir sözlüğün yanına <dosya>.json koyarak lisans, açıklama, ana sayfa gibi alanları "
                        "değiştirebilirsiniz. Ayrıntılar: docs/CATALOG.md"))
        hint.setWordWrap(True)
        hint.setObjectName("muted")
        cl.addRow("", hint)
        lay.addWidget(cb)
        lic = QPushButton(icon("license"), t("Lisanslar ve anlamları…"))
        lic.clicked.connect(self._show_licenses)
        lay.addWidget(lic, 0, Qt.AlignmentFlag.AlignLeft)
        lay.addStretch(1)
        return w

    def _run(self, what: str, fn) -> None:
        """``fn()`` in a thread; ``_on_work_done(what, result)`` after."""
        self.status.setText(t("Çalışıyor…"))

        def go():
            try:
                res = fn()
            except Exception as exc:  # noqa: BLE001
                res = exc
            self.bridge.work_done.emit(what, res)

        threading.Thread(target=go, daemon=True, name=f"dictz-{what}").start()

    def _on_work_done(self, what: str, res) -> None:
        if isinstance(res, Exception):
            self.status.setText("")
            QMessageBox.warning(self, t("Olmadı"), str(res))
            return
        if what == "export":
            self.status.setText(t("{n} sözlük dışa aktarıldı", n=len(res["dictionaries"])))
        elif what == "import":
            self.status.setText(t("İçe aktarıldı: {n} sözlük, {f} dosya, {s} kaynak", n=len(res["added"]),
                                  f=res["files"], s=res["sources"]))
            QMessageBox.information(self, t("İçe aktar"), self.status.text() + (
                "\n\n" + t("Bulunamayan dosyalar:") + "\n" + "\n".join(res["missing"][:20]) if res["missing"] else ""))
            self.imported.emit(res)
            self.reload_all()
        elif what == "catalog":
            path, n = res
            self.status.setText(t("{n} sözlük → {path}", n=n, path=path))
        else:
            self.status.setText("")

    imported = Signal(object)

    def _export_setup(self) -> None:
        path, _ = QFileDialog.getSaveFileName(self, t("Kurulumu dışa aktar"), "dictz-setup.zip", "ZIP (*.zip)")
        if not path:
            return
        from ..center.transfer import export_setup

        parts = tuple(k for k, cb in self.ex_parts.items() if cb.isChecked())
        self._run("export", lambda: export_setup(path, self.data_dir, parts))

    def _import_setup(self) -> None:
        path, _ = QFileDialog.getOpenFileName(self, t("Kurulumu içe aktar"), "", "ZIP (*.zip)")
        if not path:
            return
        from ..center.transfer import import_setup, read_manifest

        try:
            m = read_manifest(path)
        except ValueError as e:
            QMessageBox.warning(self, t("İçe aktar"), str(e))
            return
        replace = self.im_replace.isChecked()
        msg = t("{app} ile {date} tarihinde oluşturulmuş: {n} sözlük; içinde: {parts}.",
                app=m.get("app", ""), date=str(m.get("created", ""))[:10], n=len(m.get("dictionaries") or []),
                parts=", ".join(m.get("parts") or []))
        msg += "\n\n" + (t("Var olan liste, kaynaklar ve geçmiş DEĞİŞTİRİLECEK.") if replace
                         else t("Var olanlara eklenecek."))
        if QMessageBox.question(self, t("İçe aktar"), msg) != QMessageBox.StandardButton.Yes:
            return
        if replace:
            self.before_replace.emit()
        self._run("import", lambda: import_setup(path, self.data_dir, replace=replace))

    before_replace = Signal()

    def _export_list(self, fmt: str) -> None:
        path, _ = QFileDialog.getSaveFileName(self, t("Sözlük listesini dışa aktar"), f"dictz-list.{fmt}",
                                              f"{fmt.upper()} (*.{fmt})")
        if path:
            from ..center.transfer import export_list

            n = export_list(path, self.data_dir, fmt)
            self.status.setText(t("{n} sözlük dışa aktarıldı", n=n))

    def _build_catalog_file(self) -> None:
        folder = self.cb_folder.text().strip()
        if not folder or not Path(folder).is_dir():
            QMessageBox.warning(self, t("Katalog"), t("Bir klasör seçin."))
            return
        from ..center.store import dumps
        from ..center.transfer import build_catalog

        url, name, lic = self.cb_url.text().strip(), self.cb_name.text().strip(), self.cb_license.currentData()

        def go():
            cat = build_catalog(folder, url, name, lic or "")
            out = Path(folder) / "catalog.json"
            out.write_text(dumps(cat), encoding="utf-8")
            return str(out), len(cat["dictionaries"])

        self._run("catalog", go)

    def _check_catalog_file(self) -> None:
        path, _ = QFileDialog.getOpenFileName(self, t("Kataloğu denetle"), "", "JSON (*.json)")
        if not path:
            return
        import json

        from ..center.transfer import check_catalog

        try:
            n, problems = check_catalog(json.loads(Path(path).read_text(encoding="utf-8-sig")))
        except ValueError as e:
            QMessageBox.warning(self, t("Kataloğu denetle"), str(e))
            return
        errors = sum(1 for p in problems if p.startswith("error"))
        QMessageBox.information(self, t("Kataloğu denetle"),
                                t("{n} sözlük · {e} hata · {w} uyarı", n=n, e=errors, w=len(problems) - errors)
                                + ("\n\n" + "\n".join(problems[:40]) if problems else ""))

    def _show_licenses(self) -> None:
        d = QDialog(self)
        d.setWindowTitle(t("Lisanslar"))
        d.resize(820, 520)
        lay = QVBoxLayout(d)
        tb = QTableWidget(len(licenses.LICENSES), 6)
        tb.setHorizontalHeaderLabels([t("Kimlik"), t("Ad"), t("Durum"), t("Atıf"), t("Aynı lisans"), t("Ticari")])
        tb.verticalHeader().setVisible(False)
        tb.setEditTriggers(QAbstractItemView.EditTrigger.NoEditTriggers)
        yn = {True: t("evet"), False: t("hayır")}
        for i, lic in enumerate(licenses.LICENSES):
            for j, v in enumerate([lic.id, license_name(lic), status_text(lic.status), yn[lic.attribution],
                                   yn[lic.share_alike], yn[lic.commercial]]):
                it = QTableWidgetItem(v)
                if j == 2:
                    it.setForeground(QColor(STATUS_COLORS[lic.status]))
                it.setToolTip(lic.url)
                tb.setItem(i, j, it)
        tb.horizontalHeader().setSectionResizeMode(1, QHeaderView.ResizeMode.Stretch)
        tb.doubleClicked.connect(lambda idx: QDesktopServices.openUrl(QUrl(licenses.LICENSES[idx.row()].url))
                                 if licenses.LICENSES[idx.row()].url else None)
        lay.addWidget(tb)
        note = QLabel(t("Bu tablo bir özet, hukuki tavsiye değildir; bağlayıcı olan lisansın kendi metnidir "
                        "(çift tıklayınca açılır)."))
        note.setWordWrap(True)
        note.setObjectName("muted")
        lay.addWidget(note)
        d.exec()

    # ------------------------------------------------------------ refresh
    def _auto_refresh(self) -> None:
        days = int(self.prefs["center.refresh_days"])
        if days > 0 and self.prefs["center.online"]:
            stale = [s["id"] for s in self.sources.enabled() if self.catalog.stale(s["id"], days)]
            if stale:
                self.refresh(stale)

    def refresh(self, ids: list[str] | None, force: bool = False) -> None:
        if self._refreshing:
            return
        if not self.prefs["center.online"]:
            self.status.setText(t("İnternet kaynakları kapalı (Sözlükler → Ayarlar)."))
            return
        todo = ids if ids is not None else [s["id"] for s in self.sources.enabled()]
        if not todo:
            return
        self._refreshing = True
        self.refresh_btn.setEnabled(False)
        self._pending = len(todo)
        self.status.setText(t("Kataloglar okunuyor ({n})…", n=len(todo)))

        def go():
            try:
                self.catalog.refresh(todo, progress=lambda sid, r: self.bridge.refreshed.emit(sid, r))
            finally:
                self.bridge.refresh_done.emit()

        threading.Thread(target=go, daemon=True, name="dictz-catalog-refresh").start()

    def _on_refreshed(self, sid: str, res: dict) -> None:
        self._pending -= 1
        s = self.sources.get(sid)
        name = s["name"] if s else sid
        state = t("{n} sözlük", n=len(res["entries"])) if res["ok"] else t("hata")
        self.status.setText(t("Kataloglar okunuyor ({n})…", n=max(0, self._pending)) + f"  {name}: {state}")
        self._fill_sources()

    def _on_refresh_done(self) -> None:
        self._refreshing = False
        self.refresh_btn.setEnabled(True)
        errors = [s["name"] for s in self.sources.enabled() if self.catalog.cached(s["id"])["error"]]
        self.status.setText(t("Katalog") + ": " + t("{n} sözlük, {k} kaynak", n=len(self.catalog.entries()), k=len(self.sources.enabled()))
                            + (" · " + t("okunamayan: {names}", names=", ".join(errors)) if errors else ""))
        self.reload_all()

    def reload_all(self) -> None:
        self._compute_updates()
        self._fill_sources()
        self._fill_filters()
        self._apply_filter()
        self._fill_downloads()
        self._fill_history()
        self.refresh_dictionaries()
        if not self._refreshing:
            n = len(self.catalog.entries())
            self.status.setText(t("Katalog") + ": " + t("{n} sözlük, {k} kaynak", n=n, k=len(self.sources.enabled())))

    # ------------------------------------------------------------ link
    def add_from_link(self) -> None:
        url, ok = QInputDialog.getText(self, t("Bağlantıdan ekle"),
                                       t("Bir sözlük dosyasının, arşivin ya da kataloğun adresi:"))
        url = url.strip()
        if not ok or not url:
            return
        tp = guess_type(url)
        if (tp != "http-index" or url.endswith("/")) and tp not in ("http-index", "local") and QMessageBox.question(
                    self, t("Bağlantıdan ekle"),
                    t("Bu adres bir kaynak gibi görünüyor ({type}). Kaynak olarak eklensin mi?", type=type_text(tp))
        ) == QMessageBox.StandardButton.Yes:
            self.tabs.setCurrentIndex(TAB_SOURCES)
            self._add_source(url)
            return
        entry = adapters.entry_from_url(url)
        if not self.prefs["center.online"] and not url.startswith("file:"):
            QMessageBox.warning(self, t("Bağlantıdan ekle"), t("İnternet kaynakları kapalı (Sözlükler → Ayarlar)."))
            return
        self.queue.add(entry)
        self.tabs.setCurrentIndex(TAB_DOWNLOADS)

    def closeEvent(self, event):  # noqa: N802
        if self.queue.active() and QMessageBox.question(
                self, t("Sözlükler"),
                t("İndirmeler sürüyor. Pencere kapanınca arka planda devam etsinler mi?")
        ) != QMessageBox.StandardButton.Yes:
            event.ignore()
            return
        super().closeEvent(event)


def entry_html(e: dict, installed: dict | None) -> str:
    """The detail pane of one catalog entry."""
    lic = licenses.get(e["license"])
    esc = html.escape
    color = STATUS_COLORS.get(lic.status, "#888")
    yn = {True: t("evet"), False: t("hayır")}
    parts = [f"<h3 style='margin:0'>{esc(e['name'])}</h3>"]
    pair = pair_text(e)
    if pair:
        langs_txt = langs.label(e["source_lang"]) + (
            " → " + ", ".join(langs.label(x) for x in e["target_lang"]) if e["kind"] != "monolingual" else "")
        parts.append(f"<p style='margin:4px 0'><b>{esc(langs_txt)}</b> · {esc(kind_text(e['kind']))}</p>")
    if e["description"]:
        parts.append(f"<p>{esc(e['description'])}</p>")
    rows = [(t("Biçim"), e["format"] + ("" if e["supported"] is not False
                                        else " — " + t("dictz henüz açamıyor"))),
            (t("Kelime"), f"{e['headwords']:,}" if e["headwords"] else "–"), (t("Boyut"), human_size(e["size"])),
            (t("Sürüm"), e["version"] or "–"), (t("Tarih"), e["date"] or "–"), (t("Yazar"), e["author"] or "–"),
            (t("Kaynak"), e["source"])]
    if installed:
        rows.append((t("Kurulu"), installed["installed"][:10] + " · " + installed.get("folder", "")))
    parts.append("<table cellspacing=0 cellpadding=2>" + "".join(
        f"<tr><td style='color:#888'>{esc(k)}</td><td>{esc(v)}</td></tr>" for k, v in rows) + "</table>")
    name = license_name(lic)
    lic_link = f"<a href='{esc(e['license_url'])}'>{esc(name)}</a>" if e["license_url"] else esc(name)
    parts.append(f"<p style='margin-top:8px'><b>{esc(t('Lisans'))}:</b> {lic_link} "
                 f"<span style='color:{color}'>({esc(status_text(lic.status))})</span><br>"
                 f"<small>{esc(t('Atıf gerekli'))}: {yn[lic.attribution]} · {esc(t('Aynı lisansla paylaş'))}: "
                 f"{yn[lic.share_alike]} · {esc(t('Ticari kullanım'))}: {yn[lic.commercial]}</small></p>")
    if lic.status in ("unknown", "nonfree"):
        warn = t("Kaynak lisansı belirtmiyor ya da kullanımı kısıtlı. "
                 "İndirmeden önce kullanım hakkınız olduğundan emin olun.")
        parts.append(f"<p style='color:{color}'><small>{esc(warn)}</small></p>")
    if e["sample"]:
        parts.append(f"<p><small style='color:#888'>{esc(t('Örnek kelimeler'))}: {esc(e['sample'])}</small></p>")
    links = [(t("Sayfası"), e["page"]), (t("Ana sayfa"), e["homepage"])]
    links = [(k, v) for k, v in links if v]
    if links:
        parts.append("<p>" + " · ".join(f"<a href='{esc(v)}'>{esc(k)}</a>" for k, v in links) + "</p>")
    if e["files"]:
        parts.append(f"<p style='margin-bottom:0'><b>{esc(t('Dosyalar'))}</b></p><ul style='margin-top:2px'>")
        for f in e["files"][:20]:
            sums = ", ".join(k.upper() for k in ("sha256", "sha512", "md5") if f[k])
            parts.append(f"<li><a href='{esc(f['url'])}'>{esc(f['name'])}</a> "
                         f"<small>{esc(human_size(f['size']))}{(' · ' + sums) if sums else ''}</small></li>")
        parts.append("</ul>")
    elif e["resolve"]:
        parts.append(f"<p><small>{esc(t('Dosyalar kurulurken kaynaktan okunur.'))}</small></p>")
    else:
        parts.append(f"<p><small>{esc(t('İndirilecek dosya yok; sözlük sayfasından alınır.'))}</small></p>")
    return "".join(parts)

"""Preferences — every setting of ``dictz.storage.settings.SCHEMA``, grouped in
pages, searchable, with per-setting and per-page reset, plus Dictionary
folders, Shortcuts and Advanced pages."""
from __future__ import annotations

import contextlib
import json
from pathlib import Path

from PySide6.QtCore import Qt, Signal
from PySide6.QtGui import QColor, QFont
from PySide6.QtWidgets import (
    QCheckBox,
    QColorDialog,
    QComboBox,
    QDialog,
    QDialogButtonBox,
    QDoubleSpinBox,
    QFileDialog,
    QFontComboBox,
    QFormLayout,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QListWidget,
    QListWidgetItem,
    QMessageBox,
    QPushButton,
    QScrollArea,
    QSpinBox,
    QSplitter,
    QStackedWidget,
    QTableWidget,
    QTableWidgetItem,
    QToolButton,
    QVBoxLayout,
    QWidget,
)

from ..core.i18n import t
from ..core.shortcuts import SHORTCUTS
from ..storage.settings import SCHEMA, SECTIONS, Preferences, Setting

FONT_KEYS = {"appearance.ui_font_family", "article.font_family"}


class _ColorButton(QPushButton):
    changed = Signal()

    def __init__(self, value: str):
        super().__init__()
        self.value = value
        self.setFixedWidth(110)
        self.clicked.connect(self._pick)
        self._paint()

    def _paint(self) -> None:
        c = QColor(self.value)
        fg = "#000" if c.lightness() > 140 else "#fff"
        self.setText(self.value)
        self.setStyleSheet(f"background:{self.value}; color:{fg}; border-radius:6px; padding:4px;")

    def _pick(self) -> None:
        c = QColorDialog.getColor(QColor(self.value), self, t("Renk seçin"))
        if c.isValid():
            self.value = c.name()
            self._paint()
            self.changed.emit()

    def set_value(self, v: str) -> None:
        self.value = v
        self._paint()


class _FolderList(QWidget):
    changed = Signal()

    def __init__(self, folders: list[str]):
        super().__init__()
        lay = QVBoxLayout(self)
        lay.setContentsMargins(0, 0, 0, 0)
        self.list = QListWidget()
        self.list.setMinimumHeight(110)
        for f in folders:
            self.list.addItem(f)
        lay.addWidget(self.list)
        row = QHBoxLayout()
        add = QPushButton(t("Klasör ekle…"))
        rm = QPushButton(t("Kaldır"))
        add.clicked.connect(self._add)
        rm.clicked.connect(self._remove)
        row.addWidget(add)
        row.addWidget(rm)
        row.addStretch(1)
        lay.addLayout(row)

    def _add(self) -> None:
        d = QFileDialog.getExistingDirectory(self, t("Sözlük klasörü seçin"))
        if d and not self.list.findItems(d, Qt.MatchFlag.MatchExactly):
            self.list.addItem(d)
            self.changed.emit()

    def _remove(self) -> None:
        for it in self.list.selectedItems():
            self.list.takeItem(self.list.row(it))
        self.changed.emit()

    def value(self) -> list[str]:
        return [self.list.item(i).text() for i in range(self.list.count())]

    def set_value(self, folders: list[str]) -> None:
        self.list.clear()
        for f in folders:
            self.list.addItem(f)


class PreferencesDialog(QDialog):
    preferences_changed = Signal(object)
    action_requested = Signal(str)  # reindex | clear_history | clear_favorites | check_updates | open_data

    def __init__(self, prefs: Preferences, parent: QWidget | None = None):
        super().__init__(parent)
        self.setWindowTitle(t("Tercihler"))
        self.resize(900, 640)
        self.prefs = prefs
        self.draft = prefs.copy()
        self.editors: dict[str, tuple[QWidget, QWidget]] = {}  # key -> (editor, row widget)
        self.page_keys: list[list[str]] = []

        root = QVBoxLayout(self)
        top = QHBoxLayout()
        self.search = QLineEdit()
        self.search.setPlaceholderText(t("Ayar ara…  (ör. yazı tipi, bulanık, ses)"))
        self.search.setClearButtonEnabled(True)
        self.search.textChanged.connect(self._filter)
        top.addWidget(self.search)
        root.addLayout(top)

        split = QSplitter()
        self.nav = QListWidget()
        self.nav.setFixedWidth(200)
        self.stack = QStackedWidget()
        split.addWidget(self.nav)
        split.addWidget(self.stack)
        split.setStretchFactor(1, 1)
        root.addWidget(split, 1)

        for sec_id, title in SECTIONS:
            settings = [s for s in SCHEMA if s.key.split(".")[0] == sec_id]
            self._add_page(title, settings)
        self._add_shortcuts_page()
        self._add_advanced_page()
        self.nav.currentRowChanged.connect(self.stack.setCurrentIndex)
        self.nav.setCurrentRow(0)

        bottom = QHBoxLayout()
        reset_page = QPushButton(t("Bu sayfayı varsayılana döndür"))
        reset_page.clicked.connect(self._reset_page)
        reset_all = QPushButton(t("Tümünü varsayılana döndür"))
        reset_all.clicked.connect(self._reset_all)
        bottom.addWidget(reset_page)
        bottom.addWidget(reset_all)
        bottom.addStretch(1)
        self.buttons = QDialogButtonBox(
            QDialogButtonBox.StandardButton.Ok | QDialogButtonBox.StandardButton.Apply
            | QDialogButtonBox.StandardButton.Cancel)
        self.buttons.button(QDialogButtonBox.StandardButton.Ok).setText(t("Tamam"))
        self.buttons.button(QDialogButtonBox.StandardButton.Apply).setText(t("Uygula"))
        self.buttons.button(QDialogButtonBox.StandardButton.Cancel).setText(t("İptal"))
        self.buttons.accepted.connect(self._ok)
        self.buttons.rejected.connect(self.reject)
        self.buttons.button(QDialogButtonBox.StandardButton.Apply).clicked.connect(self._apply)
        bottom.addWidget(self.buttons)
        root.addLayout(bottom)

    # ------------------------------------------------------------ pages
    def _page_container(self, title: str) -> tuple[QWidget, QVBoxLayout]:
        page = QWidget()
        lay = QVBoxLayout(page)
        lab = QLabel(t(title))
        lab.setObjectName("section_title")
        lay.addWidget(lab)
        scroll = QScrollArea()
        scroll.setWidgetResizable(True)
        inner = QWidget()
        inner_lay = QVBoxLayout(inner)
        inner_lay.setContentsMargins(0, 6, 8, 6)
        scroll.setWidget(inner)
        lay.addWidget(scroll, 1)
        return page, inner_lay

    def _add_page(self, title: str, settings: list[Setting]) -> None:
        page, lay = self._page_container(title)
        form = QFormLayout()
        form.setLabelAlignment(Qt.AlignmentFlag.AlignLeft)
        form.setFieldGrowthPolicy(QFormLayout.FieldGrowthPolicy.ExpandingFieldsGrow)
        keys = []
        for s in settings:
            editor = self._editor_for(s)
            row = QWidget()
            rl = QHBoxLayout(row)
            rl.setContentsMargins(0, 0, 0, 0)
            rl.addWidget(editor, 1)
            reset = QToolButton()
            reset.setText("↺")
            reset.setToolTip(t("Varsayılan:") + f" {self._fmt_default(s)}")
            reset.clicked.connect(lambda _=False, key=s.key: self._reset_key(key))
            rl.addWidget(reset)
            label = QLabel(t(s.label))
            tip = t(s.help) + ("\n" if s.help else "") + t("Anahtar:") + f" {s.key}   (dictz config set {s.key} …)"
            label.setToolTip(tip)
            editor.setToolTip(tip)
            if s.kind == "bool":
                label = QLabel("")
            form.addRow(label, row)
            if s.help and s.kind != "list":
                hint = QLabel(t(s.help))
                hint.setObjectName("hint")
                hint.setWordWrap(True)
                form.addRow("", hint)
            self.editors[s.key] = (editor, row)
            keys.append(s.key)
        lay.addLayout(form)
        lay.addStretch(1)
        self.page_keys.append(keys)
        self.stack.addWidget(page)
        QListWidgetItem(t(title), self.nav)

    def _fmt_default(self, s: Setting) -> str:
        if s.kind == "bool":
            return t("açık") if s.default else t("kapalı")
        if s.kind == "choice":
            return t(dict(s.choices).get(s.default, s.default))
        if s.kind == "list":
            return t("boş")
        return str(s.default) if s.default != "" else t("boş")

    def _editor_for(self, s: Setting) -> QWidget:
        v = self.draft[s.key]
        if s.kind == "bool":
            w = QCheckBox(t(s.label))
            w.setChecked(bool(v))
            w.toggled.connect(lambda val, k=s.key: self._set(k, val))
        elif s.kind == "int":
            w = QSpinBox()
            w.setRange(int(s.min if s.min is not None else -10**9), int(s.max if s.max is not None else 10**9))
            w.setValue(int(v))
            if s.help and len(s.help) < 30:
                w.setSuffix(f"  {t(s.help)}")
            w.valueChanged.connect(lambda val, k=s.key: self._set(k, val))
        elif s.kind == "float":
            w = QDoubleSpinBox()
            w.setDecimals(2)
            w.setSingleStep(0.1 if (s.max or 0) <= 10 else 1.0)
            w.setRange(float(s.min if s.min is not None else -1e9), float(s.max if s.max is not None else 1e9))
            w.setValue(float(v))
            if s.help and len(s.help) < 30:
                w.setSuffix(f"  {t(s.help)}")
            w.valueChanged.connect(lambda val, k=s.key: self._set(k, val))
        elif s.kind == "choice":
            w = QComboBox()
            for val, label in s.choices:
                w.addItem(t(label), val)
            w.setCurrentIndex(max(0, w.findData(v)))
            w.currentIndexChanged.connect(lambda _i, k=s.key, c=w: self._set(k, c.currentData()))
        elif s.kind == "color":
            w = _ColorButton(v)
            w.changed.connect(lambda k=s.key, b=w: self._set(k, b.value))
        elif s.kind == "list":
            w = _FolderList(list(v))
            w.changed.connect(lambda k=s.key, f=w: self._set(k, f.value()))
        elif s.key in FONT_KEYS:
            w = QWidget()
            hl = QHBoxLayout(w)
            hl.setContentsMargins(0, 0, 0, 0)
            box = QFontComboBox()
            box.setEditable(True)
            box.setCurrentFont(QFont(v) if v else QFont())
            if not v:
                box.setEditText("")
            box.lineEdit().setPlaceholderText(t("Sistem yazı tipi"))
            box.currentTextChanged.connect(lambda t, k=s.key: self._set(k, t.strip()))
            hl.addWidget(box, 1)
            w.box = box  # type: ignore[attr-defined]
        else:
            w = QLineEdit(str(v))
            w.textChanged.connect(lambda t, k=s.key: self._set(k, t))
        return w

    def _set(self, key: str, value) -> None:
        with contextlib.suppress(KeyError, ValueError):
            self.draft.set(key, value)

    def _load_editor(self, key: str) -> None:
        editor, _ = self.editors[key]
        v = self.draft[key]
        editor.blockSignals(True)
        if isinstance(editor, QCheckBox):
            editor.setChecked(bool(v))
        elif isinstance(editor, (QSpinBox, QDoubleSpinBox)):
            editor.setValue(v)
        elif isinstance(editor, QComboBox):
            editor.setCurrentIndex(max(0, editor.findData(v)))
        elif isinstance(editor, _ColorButton):
            editor.set_value(v)
        elif isinstance(editor, _FolderList):
            editor.set_value(list(v))
        elif hasattr(editor, "box"):
            editor.box.blockSignals(True)
            editor.box.setEditText(v)
            editor.box.blockSignals(False)
        elif isinstance(editor, QLineEdit):
            editor.setText(str(v))
        editor.blockSignals(False)

    def _reset_key(self, key: str) -> None:
        self.draft.reset(key)
        self._load_editor(key)

    def _reset_page(self) -> None:
        i = self.stack.currentIndex()
        if i < len(self.page_keys):
            for k in self.page_keys[i]:
                self._reset_key(k)

    def _reset_all(self) -> None:
        if QMessageBox.question(self, t("Tercihler"), t("Tüm ayarlar varsayılana dönsün mü?")) \
                == QMessageBox.StandardButton.Yes:
            for keys in self.page_keys:
                for k in keys:
                    self._reset_key(k)

    def _filter(self, text: str) -> None:
        q = text.strip().lower()
        first_visible = None
        for page_index, keys in enumerate(self.page_keys):
            any_visible = False
            for k in keys:
                editor, row = self.editors[k]
                setting = next(s for s in SCHEMA if s.key == k)
                hay = f"{setting.label} {setting.help} {t(setting.label)} {t(setting.help)} {k}".lower()
                visible = not q or q in hay
                row.setVisible(visible)
                form = row.parentWidget().layout() if row.parentWidget() else None
                if isinstance(form, QFormLayout):
                    form.setRowVisible(row, visible)
                any_visible |= visible
            item = self.nav.item(page_index)
            item.setHidden(bool(q) and not any_visible)
            if any_visible and first_visible is None:
                first_visible = page_index
        if q and first_visible is not None:
            self.nav.setCurrentRow(first_visible)

    def _add_shortcuts_page(self) -> None:
        page, lay = self._page_container(t("Kısayollar"))
        table = QTableWidget(len(SHORTCUTS), 2)
        table.setHorizontalHeaderLabels([t("Kısayol"), t("İşlem")])
        table.verticalHeader().setVisible(False)
        table.setEditTriggers(QTableWidget.EditTrigger.NoEditTriggers)
        for i, (keys, desc) in enumerate(SHORTCUTS):
            table.setItem(i, 0, QTableWidgetItem(keys))
            table.setItem(i, 1, QTableWidgetItem(t(desc)))
        table.resizeColumnsToContents()
        table.horizontalHeader().setStretchLastSection(True)
        lay.addWidget(table)
        self.page_keys.append([])
        self.stack.addWidget(page)
        QListWidgetItem(t("Kısayollar"), self.nav)

    def _add_advanced_page(self) -> None:
        from ..app import user_data_dir

        page, lay = self._page_container(t("Gelişmiş"))
        info = QLabel(t("Veri klasörü:") + f" <b>{user_data_dir()}</b><br>"
                      + t("Sözlük listesi, indeks, geçmiş, favoriler ve bu ayarlar burada tutulur; "
                          "Python ve Rust sürümleri aynı klasörü kullanır."))
        info.setWordWrap(True)
        info.setTextInteractionFlags(Qt.TextInteractionFlag.TextSelectableByMouse)
        lay.addWidget(info)
        grid = QVBoxLayout()
        for label, action in [
            (t("Veri klasörünü aç"), "open_data"),
            (t("Tüm sözlüklerin indeksini yeniden oluştur"), "reindex"),
            (t("Güncellemeleri şimdi denetle"), "check_updates"),
            (t("Geçmişi temizle"), "clear_history"),
            (t("Favorileri temizle"), "clear_favorites"),
        ]:
            b = QPushButton(label)
            b.clicked.connect(lambda _=False, a=action: self.action_requested.emit(a))
            grid.addWidget(b)
        row = QHBoxLayout()
        exp = QPushButton(t("Ayarları dışa aktar…"))
        imp = QPushButton(t("Ayarları içe aktar…"))
        exp.clicked.connect(self._export)
        imp.clicked.connect(self._import)
        row.addWidget(exp)
        row.addWidget(imp)
        row.addStretch(1)
        grid.addLayout(row)
        lay.addLayout(grid)
        cli = QLabel(t("Komut satırından:") + " <code>dictz config list</code> · <code>dictz config set KEY VALUE</code> "
                     "· <code>dictz help</code>")
        cli.setObjectName("hint")
        lay.addWidget(cli)
        lay.addStretch(1)
        self.page_keys.append([])
        self.stack.addWidget(page)
        QListWidgetItem(t("Gelişmiş"), self.nav)

    def _export(self) -> None:
        path, _ = QFileDialog.getSaveFileName(self, t("Ayarları dışa aktar"), "dictz-settings.json", t("JSON (*.json)"))
        if path:
            Path(path).write_text(json.dumps(self.draft.export(), ensure_ascii=False, indent=2) + "\n",
                                  encoding="utf-8")

    def _import(self) -> None:
        path, _ = QFileDialog.getOpenFileName(self, t("Ayarları içe aktar"), "", t("JSON (*.json)"))
        if not path:
            return
        try:
            errors = self.draft.import_dict(json.loads(Path(path).read_text(encoding="utf-8")))
        except (OSError, ValueError) as exc:
            QMessageBox.warning(self, t("İçe aktarma"), str(exc))
            return
        for keys in self.page_keys:
            for k in keys:
                self._load_editor(k)
        if errors:
            QMessageBox.information(self, t("İçe aktarma"), "\n".join(errors[:20]))

    # ------------------------------------------------------------ apply
    def _apply(self) -> None:
        self.prefs.values.update({k: v for k, v in self.draft.values.items() if not k.startswith("state.")})
        self.prefs.save()
        self.preferences_changed.emit(self.prefs)

    def _ok(self) -> None:
        self._apply()
        self.accept()

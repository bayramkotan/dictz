"""Dictionary manager: table of dictionaries with order, enable, add files /
folders, remove, move, reindex and details."""
from __future__ import annotations

from pathlib import Path

from PySide6.QtCore import Qt, Signal
from PySide6.QtWidgets import (
    QDialog,
    QDialogButtonBox,
    QFileDialog,
    QHBoxLayout,
    QHeaderView,
    QLabel,
    QMessageBox,
    QPushButton,
    QTableWidget,
    QTableWidgetItem,
    QVBoxLayout,
    QWidget,
)

from ..core.i18n import t
from ..core.query_manager import QueryManager, scan_folder
from .icons import icon


def file_filter() -> str:
    return (t("Tüm sözlükler") + " (*.ifo *.dsl *.dz *.bgl);;StarDict (*.ifo);;DSL / Lingvo (*.dsl *.dz);;"
            "Babylon (*.bgl);;" + t("Tüm dosyalar") + " (*)")


def info_html(qm: QueryManager, name: str) -> str:
    import html
    import time

    d = qm.dictionaries[name]
    m = d.metadata
    p = Path(m.file_path)
    size = p.stat().st_size if p.exists() else 0
    mtime = time.strftime("%Y-%m-%d %H:%M", time.localtime(p.stat().st_mtime)) if p.exists() else "—"
    media = qm.media(name)
    rows = [
        (t("Ad"), m.name), (t("Biçim"), m.format), (t("Kelime"), f"{m.word_count:,}"),
        (t("Kaynak dil"), m.source_language or "—"), (t("Hedef dil"), m.target_language or "—"),
        (t("Yazar"), m.author or "—"), (t("Dosya"), str(p)), (t("Boyut"), f"{size / 1024:,.0f} KB"),
        (t("Değiştirilme"), mtime), (t("Etkin"), t("evet") if qm.is_enabled(name) else t("hayır")),
        (t("Medya klasörleri"), "<br>".join(html.escape(str(x)) for x in (media.search_locations() if media else []))),
    ]
    body = "".join(f"<tr><td><b>{k}</b></td><td>{v if k == t('Medya klasörleri') else html.escape(v)}</td></tr>"
                   for k, v in rows)
    desc = f"<p>{html.escape(m.description)}</p>" if m.description else ""
    return f"<table cellpadding=4>{body}</table>{desc}"


class DictionaryManagerDialog(QDialog):
    dictionaries_changed = Signal()
    add_paths_requested = Signal(list)

    COLS = ["", "Ad", "Biçim", "Kelime", "Diller", "Dosya"]

    def __init__(self, query_manager: QueryManager, parent: QWidget | None = None):
        super().__init__(parent)
        self.query_manager = query_manager
        self.setWindowTitle(t("Sözlük yöneticisi"))
        self.resize(940, 520)
        layout = QVBoxLayout(self)
        layout.addWidget(QLabel(t("Sözlükler arama sırasına göre listelenir. Kutu = etkin. Çift tık: ayrıntılar.")))
        self.table = QTableWidget(0, len(self.COLS))
        self.table.setHorizontalHeaderLabels([t(c) for c in self.COLS])
        self.table.verticalHeader().setVisible(True)
        self.table.setSelectionBehavior(QTableWidget.SelectionBehavior.SelectRows)
        self.table.setSelectionMode(QTableWidget.SelectionMode.SingleSelection)
        self.table.setEditTriggers(QTableWidget.EditTrigger.NoEditTriggers)
        hh = self.table.horizontalHeader()
        hh.setSectionResizeMode(0, QHeaderView.ResizeMode.ResizeToContents)
        hh.setSectionResizeMode(1, QHeaderView.ResizeMode.Stretch)
        hh.setSectionResizeMode(5, QHeaderView.ResizeMode.Stretch)
        self.table.itemChanged.connect(self._on_item_changed)
        self.table.cellDoubleClicked.connect(lambda r, c: self._info())
        layout.addWidget(self.table, 1)

        row = QHBoxLayout()
        buttons = [
            (t("Dosya ekle…"), "open", self._add_files), (t("Klasör ekle…"), "folder", self._add_folder),
            (t("Kaldır"), "remove", self._remove), (t("En üste"), "up", lambda: self._move("top")),
            (t("Yukarı"), "up", lambda: self._move(-1)), (t("Aşağı"), "down", lambda: self._move(1)),
            (t("En alta"), "down", lambda: self._move("bottom")), (t("Yeniden indeksle"), "refresh", self._reindex),
            (t("Bilgi"), "help", self._info),
        ]
        for label, ic, fn in buttons:
            b = QPushButton(icon(ic), label)
            b.clicked.connect(fn)
            row.addWidget(b)
        row.addStretch(1)
        layout.addLayout(row)
        bb = QDialogButtonBox(QDialogButtonBox.StandardButton.Close)
        bb.rejected.connect(self.reject)
        bb.accepted.connect(self.accept)
        bb.button(QDialogButtonBox.StandardButton.Close).setText(t("Kapat"))
        layout.addWidget(bb)
        self.summary = QLabel("")
        self.summary.setObjectName("hint")
        layout.addWidget(self.summary)
        self.refresh()

    def refresh(self) -> None:
        qm = self.query_manager
        self.table.blockSignals(True)
        names = qm.loaded_names()
        self.table.setRowCount(len(names))
        for r, name in enumerate(names):
            m = qm.dictionaries[name].metadata
            chk = QTableWidgetItem()
            chk.setFlags(Qt.ItemFlag.ItemIsUserCheckable | Qt.ItemFlag.ItemIsEnabled | Qt.ItemFlag.ItemIsSelectable)
            chk.setCheckState(Qt.CheckState.Checked if qm.is_enabled(name) else Qt.CheckState.Unchecked)
            chk.setData(Qt.ItemDataRole.UserRole, name)
            self.table.setItem(r, 0, chk)
            langs = f"{m.source_language} → {m.target_language}" if m.source_language else ""
            for c, text in enumerate([name, m.format, f"{m.word_count:,}", langs, str(m.file_path)], start=1):
                it = QTableWidgetItem(text)
                it.setData(Qt.ItemDataRole.UserRole, name)
                if c == 3:
                    it.setTextAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
                self.table.setItem(r, c, it)
        self.table.blockSignals(False)
        enabled = [n for n in names if qm.is_enabled(n)]
        words = sum(qm.dictionaries[n].metadata.word_count for n in enabled)
        self.summary.setText(t("{n} / {m} sözlük etkin · {w} kelime", n=len(enabled), m=len(names), w=f"{words:,}"))

    def _selected_name(self) -> str | None:
        r = self.table.currentRow()
        it = self.table.item(r, 0) if r >= 0 else None
        return it.data(Qt.ItemDataRole.UserRole) if it else None

    def _select(self, name: str) -> None:
        for r in range(self.table.rowCount()):
            if self.table.item(r, 0).data(Qt.ItemDataRole.UserRole) == name:
                self.table.selectRow(r)
                return

    def _add_files(self) -> None:
        paths, _ = QFileDialog.getOpenFileNames(self, t("Sözlük dosyası seçin"), "", file_filter())
        if paths:
            self.add_paths_requested.emit([Path(p) for p in paths])

    def _add_folder(self) -> None:
        d = QFileDialog.getExistingDirectory(self, t("Sözlük klasörü seçin"))
        if not d:
            return
        files = scan_folder(d, recursive=True)
        if not files:
            QMessageBox.information(self, t("Klasör ekle"), t("Bu klasörde sözlük dosyası bulunamadı."))
            return
        self.add_paths_requested.emit(files)

    def _remove(self) -> None:
        name = self._selected_name()
        if not name:
            return
        if QMessageBox.question(self, t("Sözlüğü kaldır"),
                                t("“{name}” dictz'den kaldırılsın mı?\n(Dosyası silinmez.)", name=name)) \
                != QMessageBox.StandardButton.Yes:
            return
        self.query_manager.remove_dictionary(name)
        self.refresh()
        self.dictionaries_changed.emit()

    def _move(self, delta) -> None:
        name = self._selected_name()
        if not name:
            return
        names = self.query_manager.loaded_names()
        i = names.index(name)
        if delta == "top":
            delta = -i
        elif delta == "bottom":
            delta = len(names) - 1 - i
        self.query_manager.move_dictionary(name, delta)
        self.refresh()
        self._select(name)
        self.dictionaries_changed.emit()

    def _reindex(self) -> None:
        name = self._selected_name()
        qm = self.query_manager
        if not name or qm.index_store is None:
            return
        n = qm.rebuild(name)  # reads the file again (article store + index)
        QMessageBox.information(self, t("Yeniden indeksle"), t("{name}: {n} kelime indekslendi.", name=name, n=f"{n:,}"))

    def _info(self) -> None:
        name = self._selected_name()
        if name:
            QMessageBox.information(self, name, info_html(self.query_manager, name))

    def _on_item_changed(self, item: QTableWidgetItem) -> None:
        if item.column() != 0:
            return
        self.query_manager.set_enabled(item.data(Qt.ItemDataRole.UserRole),
                                       item.checkState() == Qt.CheckState.Checked)
        self.refresh()
        self.dictionaries_changed.emit()

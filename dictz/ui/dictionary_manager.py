"""Dictionary manager dialog."""
from __future__ import annotations

from pathlib import Path

from PySide6.QtCore import Qt, Signal
from PySide6.QtWidgets import (
    QDialog, QDialogButtonBox, QFileDialog, QHBoxLayout, QLabel,
    QListWidget, QListWidgetItem, QPushButton, QVBoxLayout, QWidget,
)

from ..core.query_manager import QueryManager
from .icons import icon


class DictionaryManagerDialog(QDialog):
    dictionaries_changed = Signal()

    def __init__(self, query_manager: QueryManager, parent: QWidget | None = None):
        super().__init__(parent)
        self.setWindowTitle("Sözlük yöneticisi")
        self.resize(640, 440)
        self.query_manager = query_manager
        layout = QVBoxLayout(self)
        layout.addWidget(QLabel("Yüklü sözlükler (etkinleştirmek için kutuyu işaretleyin):"))
        self.list = QListWidget()
        self.list.setSelectionMode(QListWidget.SelectionMode.SingleSelection)
        self.list.itemChanged.connect(self._on_item_changed)
        layout.addWidget(self.list, 1)
        row = QHBoxLayout()
        self.add_btn = QPushButton(icon("open"), "Ekle…")
        self.remove_btn = QPushButton(icon("remove"), "Kaldır")
        self.up_btn = QPushButton(icon("up"), "Yukarı")
        self.down_btn = QPushButton(icon("down"), "Aşağı")
        self.add_btn.clicked.connect(self._add)
        self.remove_btn.clicked.connect(self._remove)
        self.up_btn.clicked.connect(lambda: self._move(-1))
        self.down_btn.clicked.connect(lambda: self._move(1))
        row.addWidget(self.add_btn)
        row.addWidget(self.remove_btn)
        row.addStretch(1)
        row.addWidget(self.up_btn)
        row.addWidget(self.down_btn)
        layout.addLayout(row)
        buttons = QDialogButtonBox(QDialogButtonBox.StandardButton.Close)
        buttons.rejected.connect(self.accept)
        buttons.accepted.connect(self.accept)
        layout.addWidget(buttons)
        self._refresh()

    def _refresh(self) -> None:
        self.list.blockSignals(True)
        self.list.clear()
        for name in self.query_manager.loaded_names():
            d = self.query_manager.dictionaries[name]
            text = f"{name}  —  {d.metadata.format}  ·  {d.metadata.word_count:,} kelime"
            it = QListWidgetItem(text)
            it.setData(Qt.ItemDataRole.UserRole, name)
            it.setFlags(it.flags() | Qt.ItemFlag.ItemIsUserCheckable)
            it.setCheckState(Qt.CheckState.Checked if self.query_manager.is_enabled(name) else Qt.CheckState.Unchecked)
            self.list.addItem(it)
        self.list.blockSignals(False)

    def _selected_name(self) -> str | None:
        it = self.list.currentItem()
        return it.data(Qt.ItemDataRole.UserRole) if it else None

    def _add(self) -> None:
        file_filter = ("Tüm sözlükler (*.ifo *.dsl *.dsl.dz *.bgl);;"
                       "StarDict (*.ifo);;DSL / Lingvo (*.dsl *.dsl.dz);;"
                       "Babylon (*.bgl);;Tüm dosyalar (*)")
        paths, _ = QFileDialog.getOpenFileNames(self, "Sözlük dosyası seçin", "", file_filter)
        if not paths:
            return
        for p in paths:
            try:
                self.query_manager.add_dictionary(Path(p))
            except Exception as e:
                from PySide6.QtWidgets import QMessageBox
                QMessageBox.critical(self, "Yüklenemedi", f"<b>{p}</b><br><br>{type(e).__name__}: {e}")
        self._refresh()
        self.dictionaries_changed.emit()

    def _remove(self) -> None:
        name = self._selected_name()
        if not name:
            return
        self.query_manager.remove_dictionary(name)
        self._refresh()
        self.dictionaries_changed.emit()

    def _move(self, delta: int) -> None:
        row = self.list.currentRow()
        if row < 0:
            return
        new_row = row + delta
        if not 0 <= new_row < self.list.count():
            return
        it = self.list.takeItem(row)
        self.list.insertItem(new_row, it)
        self.list.setCurrentItem(it)
        if self.query_manager.index_store is not None:
            for i in range(self.list.count()):
                nm = self.list.item(i).data(Qt.ItemDataRole.UserRole)
                self.query_manager.index_store.set_sort_order(nm, i)
        self.dictionaries_changed.emit()

    def _on_item_changed(self, item: QListWidgetItem) -> None:
        name = item.data(Qt.ItemDataRole.UserRole)
        enabled = item.checkState() == Qt.CheckState.Checked
        self.query_manager.set_enabled(name, enabled)
        self.dictionaries_changed.emit()

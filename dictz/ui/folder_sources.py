"""Dictionary folders editor (like GoldenDict's Sources → Files): every folder
with its own "subfolders" box. Used in Preferences → Dictionaries and in the
dictionary manager. Folders are kept in ``settings.json``
(``dictionaries.folders`` + ``dictionaries.flat_folders``) and scanned at
every start."""
from __future__ import annotations

from PySide6.QtCore import Qt, Signal
from PySide6.QtWidgets import (
    QFileDialog,
    QHBoxLayout,
    QHeaderView,
    QPushButton,
    QTableWidget,
    QTableWidgetItem,
    QVBoxLayout,
    QWidget,
)

from ..core.i18n import t


class FolderSources(QWidget):
    changed = Signal()
    rescan_requested = Signal()

    def __init__(self, sources: list[tuple[str, bool]], default_recursive: bool = True,
                 parent: QWidget | None = None):
        super().__init__(parent)
        self.default_recursive = default_recursive
        lay = QVBoxLayout(self)
        lay.setContentsMargins(0, 0, 0, 0)
        self.table = QTableWidget(0, 2)
        self.table.setHorizontalHeaderLabels([t("Klasör"), t("Alt klasörler")])
        self.table.verticalHeader().setVisible(False)
        self.table.setSelectionBehavior(QTableWidget.SelectionBehavior.SelectRows)
        self.table.setEditTriggers(QTableWidget.EditTrigger.NoEditTriggers)
        hh = self.table.horizontalHeader()
        hh.setSectionResizeMode(0, QHeaderView.ResizeMode.Stretch)
        hh.setSectionResizeMode(1, QHeaderView.ResizeMode.ResizeToContents)
        self.table.setMinimumHeight(130)
        self.table.itemChanged.connect(lambda _it: self.changed.emit())
        lay.addWidget(self.table)
        row = QHBoxLayout()
        add = QPushButton(t("Klasör ekle…"))
        rm = QPushButton(t("Kaldır"))
        scan = QPushButton(t("Şimdi tara"))
        add.clicked.connect(self._add)
        rm.clicked.connect(self._remove)
        scan.clicked.connect(self.rescan_requested.emit)
        for b in (add, rm, scan):
            row.addWidget(b)
        row.addStretch(1)
        lay.addLayout(row)
        self.set_value(sources)

    def _append(self, folder: str, recursive: bool) -> None:
        r = self.table.rowCount()
        self.table.insertRow(r)
        it = QTableWidgetItem(folder)
        it.setToolTip(folder)
        self.table.setItem(r, 0, it)
        box = QTableWidgetItem()
        box.setFlags(Qt.ItemFlag.ItemIsUserCheckable | Qt.ItemFlag.ItemIsEnabled | Qt.ItemFlag.ItemIsSelectable)
        box.setCheckState(Qt.CheckState.Checked if recursive else Qt.CheckState.Unchecked)
        self.table.setItem(r, 1, box)

    def _add(self) -> None:
        d = QFileDialog.getExistingDirectory(self, t("Sözlük klasörü seçin"))
        if d and d not in [f for f, _ in self.value()]:
            self.table.blockSignals(True)
            self._append(d, self.default_recursive)
            self.table.blockSignals(False)
            self.changed.emit()

    def _remove(self) -> None:
        rows = sorted({i.row() for i in self.table.selectedItems()}, reverse=True)
        for r in rows:
            self.table.removeRow(r)
        if rows:
            self.changed.emit()

    def value(self) -> list[tuple[str, bool]]:
        return [(self.table.item(r, 0).text(), self.table.item(r, 1).checkState() == Qt.CheckState.Checked)
                for r in range(self.table.rowCount())]

    def set_value(self, sources: list[tuple[str, bool]]) -> None:
        self.table.blockSignals(True)
        self.table.setRowCount(0)
        for f, rec in sources:
            self._append(f, rec)
        self.table.blockSignals(False)

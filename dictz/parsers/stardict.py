"""StarDict format parser."""
from __future__ import annotations

import gzip
import struct
from pathlib import Path
from typing import Iterator

from .base import Dictionary, DictionaryEntry, sort_synonyms, synonym_targets


class StarDictError(Exception):
    """Raised when a StarDict file is malformed or unsupported."""


class StarDictDictionary(Dictionary):
    def __init__(self, path: Path | str):
        super().__init__(path)
        if self.path.suffix.lower() != ".ifo":
            ifo_candidate = self.path.with_suffix(".ifo")
            if ifo_candidate.exists():
                self.path = ifo_candidate
            else:
                raise StarDictError(f"No .ifo file found alongside {self.path}")

        self._basepath = self.path.with_suffix("")
        self._idx_offset_bits = 32
        self._index: dict[str, tuple[int, int]] = {}
        self._sorted_words: list[str] = []
        self._idx_order: list[str] = []  # every .idx entry in file order (the .syn refers to it)
        self._synonyms: list[tuple[str, str]] = []
        self._dict_data: bytes | None = None
        self.metadata.format = "stardict"

    def load(self) -> None:
        if self._loaded:
            return
        self._parse_ifo()
        self._parse_idx()
        self._parse_syn()
        self._load_dict_data()
        self.metadata.word_count = len(self._index)
        self._sorted_words = sorted(self._index.keys(), key=str.lower)
        self._loaded = True

    def _parse_ifo(self) -> None:
        ifo_path = self._basepath.with_suffix(".ifo")
        text = ifo_path.read_text(encoding="utf-8", errors="replace")
        lines = text.splitlines()
        if not lines or not lines[0].startswith("StarDict's dict ifo file"):
            raise StarDictError(f"Not a valid StarDict .ifo: {ifo_path}")
        kv: dict[str, str] = {}
        for line in lines[1:]:
            if "=" not in line:
                continue
            key, _, value = line.partition("=")
            kv[key.strip()] = value.strip()
        self.metadata.name = kv.get("bookname", self.path.stem)
        self.metadata.description = kv.get("description", "")
        self.metadata.author = kv.get("author", "")
        offset_bits = kv.get("idxoffsetbits", "32")
        try:
            self._idx_offset_bits = int(offset_bits)
        except ValueError:
            self._idx_offset_bits = 32
        if self._idx_offset_bits not in (32, 64):
            raise StarDictError(f"Unsupported idxoffsetbits: {self._idx_offset_bits}")

    def _parse_idx(self) -> None:
        idx_path = self._basepath.with_suffix(".idx")
        if not idx_path.exists():
            gz_path = self._basepath.with_suffix(".idx.gz")
            if gz_path.exists():
                with gzip.open(gz_path, "rb") as f:
                    data = f.read()
            else:
                raise StarDictError(f"Missing .idx file: {idx_path}")
        else:
            data = idx_path.read_bytes()
        offset_size = self._idx_offset_bits // 8
        entry_tail_size = offset_size + 4
        offset_fmt = ">I" if self._idx_offset_bits == 32 else ">Q"
        pos = 0
        n = len(data)
        while pos < n:
            null_pos = data.find(b"\x00", pos)
            if null_pos == -1 or null_pos + entry_tail_size > n:
                break
            try:
                word = data[pos:null_pos].decode("utf-8")
            except UnicodeDecodeError:
                word = data[pos:null_pos].decode("utf-8", errors="replace")
            tail_start = null_pos + 1
            offset = struct.unpack(offset_fmt, data[tail_start : tail_start + offset_size])[0]
            size = struct.unpack(">I", data[tail_start + offset_size : tail_start + entry_tail_size])[0]
            if word and word not in self._index:
                self._index[word] = (offset, size)
            self._idx_order.append(word)
            pos = tail_start + entry_tail_size

    def _parse_syn(self) -> None:
        """``.syn``: ``synonym\\0`` + the 32-bit number of an ``.idx`` entry
        (in file order). A synonym that is a headword itself is left out."""
        data = b""
        for ext in (".syn", ".syn.dz"):
            p = Path(str(self._basepath) + ext)
            if p.is_file():
                if ext.endswith(".dz"):
                    with gzip.open(p, "rb") as f:
                        data = f.read()
                else:
                    data = p.read_bytes()
                break
        pairs = []
        pos, n = 0, len(data)
        while pos < n:
            nul = data.find(b"\x00", pos)
            if nul == -1 or nul + 5 > n:
                break
            word = data[pos:nul].decode("utf-8", errors="replace")
            (i,) = struct.unpack(">I", data[nul + 1:nul + 5])
            pos = nul + 5
            if word and i < len(self._idx_order) and word not in self._index:
                target = self._idx_order[i]
                if target in self._index:
                    pairs.append((word, target))
        self._synonyms = sort_synonyms(pairs)

    def synonyms(self) -> list[tuple[str, str]]:
        if not self._loaded:
            self.load()
        return list(self._synonyms)

    def _load_dict_data(self) -> None:
        dict_path = self._basepath.with_suffix(".dict")
        dz_path = Path(str(self._basepath) + ".dict.dz")
        if dict_path.exists():
            self._dict_data = dict_path.read_bytes()
        elif dz_path.exists():
            with gzip.open(dz_path, "rb") as f:
                self._dict_data = f.read()
        else:
            raise StarDictError(f"Missing .dict or .dict.dz file near {self._basepath}")

    def lookup(self, word: str, exact: bool = True) -> list[DictionaryEntry]:
        if not self._loaded:
            self.load()
        results: list[DictionaryEntry] = []
        if word in self._index:
            results.append(self._entry_for(word))
        if not results:
            word_lower = word.lower()
            for indexed_word in self._sorted_words:
                if indexed_word.lower() == word_lower:
                    results.append(self._entry_for(indexed_word))
                    break
        if not results and self._synonyms:
            results = [self._entry_for(t) for t in synonym_targets(self._synonyms, word)]
        if not exact and not results:
            word_lower = word.lower()
            for indexed_word in self._sorted_words:
                if indexed_word.lower().startswith(word_lower):
                    results.append(self._entry_for(indexed_word))
                    if len(results) >= 50:
                        break
        return results

    def _entry_for(self, word: str) -> DictionaryEntry:
        offset, size = self._index[word]
        assert self._dict_data is not None
        raw = self._dict_data[offset : offset + size]
        try:
            definition = raw.decode("utf-8")
        except UnicodeDecodeError:
            definition = raw.decode("utf-8", errors="replace")
        return DictionaryEntry(
            headword=word,
            definition=definition,
            dictionary_id=self.metadata.name,
            score=1.0,
        )

    def iter_entries(self) -> Iterator[DictionaryEntry]:
        if not self._loaded:
            self.load()
        for word in self._sorted_words:
            yield self._entry_for(word)

    def close(self) -> None:
        self._dict_data = None
        self._index.clear()
        self._sorted_words.clear()
        self._idx_order.clear()
        self._synonyms = []
        super().close()

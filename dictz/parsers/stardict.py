"""StarDict format parser.

StarDict files come in groups of 3-4 files sharing a basename:
    name.ifo   — plain text header with metadata
    name.idx   — binary index: [word\\0][offset:4 bytes BE][size:4 bytes BE]...
    name.dict  — concatenated definitions (raw bytes)
    name.dict.dz — same, compressed with dictzip (gzip variant w/ random access)
    name.syn   — optional synonyms index (binary)

Reference: https://github.com/huzheng001/stardict-3/blob/master/dict/doc/StarDictFileFormat
"""
from __future__ import annotations

import gzip
import struct
from pathlib import Path
from typing import Iterator

from .base import Dictionary, DictionaryEntry


class StarDictError(Exception):
    """Raised when a StarDict file is malformed or unsupported."""


class StarDictDictionary(Dictionary):
    """Native StarDict parser. Reads .ifo, .idx, .dict (or .dict.dz)."""

    def __init__(self, path: Path | str):
        super().__init__(path)
        # path can point to any of the sibling files; we normalize to the .ifo
        if self.path.suffix.lower() != ".ifo":
            ifo_candidate = self.path.with_suffix(".ifo")
            if ifo_candidate.exists():
                self.path = ifo_candidate
            else:
                raise StarDictError(f"No .ifo file found alongside {self.path}")

        self._basepath = self.path.with_suffix("")  # strip .ifo
        self._idx_offset_bits = 32  # default; .ifo may override to 64
        self._index: dict[str, tuple[int, int]] = {}  # word -> (offset, size)
        self._sorted_words: list[str] = []  # for prefix search
        self._dict_data: bytes | None = None
        self.metadata.format = "stardict"

    # ------------------------------------------------------------------ load

    def load(self) -> None:
        if self._loaded:
            return
        self._parse_ifo()
        self._parse_idx()
        self._load_dict_data()
        self.metadata.word_count = len(self._index)
        self._sorted_words = sorted(self._index.keys(), key=str.lower)
        self._loaded = True

    def _parse_ifo(self) -> None:
        """Parse the .ifo header file. Format: key=value per line."""
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

        # Required fields
        self.metadata.name = kv.get("bookname", self.path.stem)
        self.metadata.description = kv.get("description", "")
        self.metadata.author = kv.get("author", "")
        self.metadata.source_language = kv.get("sametypesequence", "")  # not lang, but ok placeholder

        # idxoffsetbits: defaults to 32, can be 64 for huge dicts
        offset_bits = kv.get("idxoffsetbits", "32")
        try:
            self._idx_offset_bits = int(offset_bits)
        except ValueError:
            self._idx_offset_bits = 32

        if self._idx_offset_bits not in (32, 64):
            raise StarDictError(f"Unsupported idxoffsetbits: {self._idx_offset_bits}")

    def _parse_idx(self) -> None:
        """Parse .idx file. Format per entry: word\\0 + offset + size."""
        idx_path = self._basepath.with_suffix(".idx")
        if not idx_path.exists():
            # Try compressed .idx.gz
            gz_path = self._basepath.with_suffix(".idx.gz")
            if gz_path.exists():
                with gzip.open(gz_path, "rb") as f:
                    data = f.read()
            else:
                raise StarDictError(f"Missing .idx file: {idx_path}")
        else:
            data = idx_path.read_bytes()

        offset_size = self._idx_offset_bits // 8
        entry_tail_size = offset_size + 4  # offset + 4-byte size
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

            # Store; if duplicate headword, keep the first occurrence
            if word and word not in self._index:
                self._index[word] = (offset, size)

            pos = tail_start + entry_tail_size

    def _load_dict_data(self) -> None:
        """Load the .dict (or .dict.dz) into memory.

        For .dict.dz we decompress the whole thing. A more memory-efficient
        approach would use dictzip's random-access capability, but for v0.0.1
        we keep things simple — most dictionaries are < 200 MB decompressed.
        """
        dict_path = self._basepath.with_suffix(".dict")
        dz_path = Path(str(self._basepath) + ".dict.dz")

        if dict_path.exists():
            self._dict_data = dict_path.read_bytes()
        elif dz_path.exists():
            # dictzip is a gzip variant; standard gzip can read it
            with gzip.open(dz_path, "rb") as f:
                self._dict_data = f.read()
        else:
            raise StarDictError(f"Missing .dict or .dict.dz file near {self._basepath}")

    # ---------------------------------------------------------------- lookup

    def lookup(self, word: str, exact: bool = True) -> list[DictionaryEntry]:
        if not self._loaded:
            self.load()

        results: list[DictionaryEntry] = []

        # Case-sensitive direct hit first
        if word in self._index:
            results.append(self._entry_for(word))

        # Case-insensitive scan (StarDict indexes are case-sensitive, but
        # users expect case-insensitive lookup by default)
        if not results:
            word_lower = word.lower()
            for indexed_word in self._sorted_words:
                if indexed_word.lower() == word_lower:
                    results.append(self._entry_for(indexed_word))
                    break

        if not exact and not results:
            # Prefix search (case-insensitive), limit to 50 to avoid flooding
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
        super().close()

# dictz catalogs (`catalog.json`)

A **dictz catalog** is one JSON file listing downloadable dictionaries. The
Dictionary Center reads it from any address — a web server, GitHub, a share,
a `file:` path — when it is added as a source of type `dictz-catalog`.
dictz's own default server, [github.com/bayramkotan/dictz-data](https://github.com/bayramkotan/dictz-data),
is a dictz catalog too. Both editions (Python and Rust) read and write the
same format.

## The file

```json
{
 "catalog": {
  "schema": 1,
  "name": "dictz-data",
  "updated": "2026-11-21T10:00:00Z",
  "base_url": "https://raw.githubusercontent.com/bayramkotan/dictz-data/main/",
  "license": "",
  "include": ["turkic/catalog.json"]
 },
 "dictionaries": [
  {
   "id": "en-tr-wordlist",
   "name": "English–Turkish Word List",
   "description": "A free word list.",
   "source_lang": "en",
   "target_lang": ["tr"],
   "format": "stardict",
   "headwords": 12345,
   "version": "1.2",
   "date": "2026-05-01",
   "license": "CC-BY-SA-4.0",
   "license_url": "https://creativecommons.org/licenses/by-sa/4.0/",
   "author": "dictz",
   "homepage": "https://example.org/wordlist",
   "page": "",
   "tags": ["general"],
   "sample": "apple, book",
   "files": [
    {"url": "files/en-tr-wordlist.tar.xz", "size": 2048, "sha256": "…"}
   ]
  }
 ]
}
```

A bare list (`[ {...}, {...} ]`) is read as the `dictionaries` list.

### `catalog` (the head)

| Key | Meaning |
|---|---|
| `schema` | `1`. dictz warns about other numbers (`dct catalog check`). |
| `name` | shown nowhere yet; for people reading the file |
| `updated` | ISO time of the last build |
| `base_url` | relative file addresses are relative to this (default: the catalog's own address) |
| `license` | licence of every entry that has none |
| `include` | more catalogs, relative to this one; read one level deep |

### A dictionary

Only `files` (or `resolve`) is needed; everything else makes the catalog
better. dictz fills what is missing (`normalize` in `dictz/center/model.py`,
the same rules in `dictz-rs/src/center/model.rs`).

| Key | Type | Meaning |
|---|---|---|
| `id` | text | unique in the catalog. dictz shows it as `<source>:<id>`. Default: the name in lower case with `-` |
| `name` | text | default: the first file's name |
| `description` | text | one or two sentences |
| `source_lang` | text | ISO 639 code (`en`, `eng`) or an English name (`English`); dictz stores the shortest code |
| `target_lang` | text or list | the same; several for a multilingual dictionary |
| `kind` | text | `bilingual`, `monolingual`, `multilingual`; default from the languages |
| `format` | text | `stardict`, `dsl`, `babylon` (dictz opens these), `mdict`, `dictd`, `xdxf`, `slob`, `tabfile`, `mobi`, `epub`, `zim`, `lingoes`, `json`; default from the file names |
| `supported` | true / false | default: `true` for the three formats dictz opens |
| `headwords` | number | word count |
| `version`, `date` | text | an installed dictionary has an **update** when either differs from the catalog |
| `size` | number | bytes; default: the sum of the files |
| `license` | text | a licence id from the table below (other spellings are recognised: `gplv3`, `CC BY-SA 4.0`, `public domain` …) |
| `license_url` | text | default: the licence's own page |
| `author`, `homepage`, `page` | text | `page` is the dictionary's own page (opened by *Sayfası*) |
| `tags` | list | free words, searched by the catalog filter |
| `sample` | text | a few headwords |
| `files` | list | what to download, in order |
| `resolve` | text | instead of `files`: `archive-org:<identifier>` reads the item's files at install time |

### A file

A file is an object or simply its address.

| Key | Meaning |
|---|---|
| `url` | absolute, or relative to `base_url` / the catalog |
| `name` | default: the last part of the address |
| `size` | bytes (the download is checked against it) |
| `sha256`, `sha512`, `md5` | checksums; when one is given and the setting `center.verify_checksums` is on, a file that does not match is not installed |
| `archive` | `zip`, `tar`, `tar.gz`, `tar.xz`, `tar.bz2` (unpacked by dictz), `7z`, `rar` (not unpacked); default from the name |

A StarDict set (`.ifo`, `.idx`, `.dict.dz`, `.syn`) or an MDict pair
(`.mdx` + `.mdd`) is **one** dictionary with several files. A DSL may bring
its media archive (`X.dsl.dz.files.zip`) as a second file.

## Licences

`dct licenses` prints the table, the window shows it under *İçe / Dışa aktar →
Lisanslar ve anlamları…*. Each id has a status:

| Status | Meaning | Ids |
|---|---|---|
| **open** | an open licence | `CC0-1.0`, `public-domain`, `CC-BY-3.0`, `CC-BY-4.0`, `CC-BY-SA-3.0`, `CC-BY-SA-4.0`, `GPL-2.0-only`, `GPL-2.0-or-later`, `GPL-3.0-only`, `GPL-3.0-or-later`, `LGPL-2.1-or-later`, `AGPL-3.0-or-later`, `GFDL-1.3-or-later`, `MIT`, `BSD-3-Clause`, `Apache-2.0`, `ODbL-1.0`, `WordNet`, `EDRDG`, `freedict`, `wiktionary` |
| **free** | free to use, but with limits or not a named licence | `CC-BY-NC-4.0`, `CC-BY-NC-SA-4.0`, `free` |
| **nonfree** | personal use only | `personal` |
| **unknown** | the source does not say | `unknown` |

The catalog filter *Açık lisans* shows only **open**, *Açık + serbest* open
and free. dictz asks before it downloads an unknown or personal-use
dictionary. The table is a summary, not legal advice: the licence's own
text is binding.

## Build a catalog for your own server

```bash
dct catalog build ~/my-dicts --base-url https://example.org/dicts/ \
    --name my-dicts --default-license CC-BY-SA-4.0     # → ~/my-dicts/catalog.json
dct catalog check ~/my-dicts/catalog.json            # errors and warnings
```

or in the window: *Sözlük Merkezi → İçe / Dışa aktar → Kendi sunucun için
katalog oluştur*.

For every dictionary (and archive) in the folder and its subfolders the
builder writes the files with size and SHA-256, the address (`--base-url` +
the file's path) and, read from the dictionary itself, its name, languages,
word count, author, description and a few sample headwords (`--no-read`
skips opening them).

### Sidecar files

A `<file>.json` next to a dictionary's first file overrides any field except
`id` and `files`:

```
my-dicts/
  en-tr.ifo  en-tr.idx  en-tr.dict.dz
  en-tr.ifo.json        ← {"license": "CC-BY-SA-4.0", "homepage": "https://…", "description": "…"}
  ota-tr.dsl.dz
  ota-tr.dsl.dz.json    ← {"license": "GPL-3.0-or-later", "author": "…"}
```

Publish the folder with its `catalog.json` (a GitHub repository, a web
server, a WebDAV share …) and add the address in *Kaynaklar (sunucular) →
Ekle…* or with `dct sources add URL`. Make it the default with *Varsayılan
sunucu yap* / `dct sources default ID`.

## The other source types

| Type | Address | Notes |
|---|---|---|
| `dictz-catalog` | a `catalog.json` | this document |
| `freedict` | `https://freedict.org/freedict-database.json` | every release of every pair |
| `github-releases` | `https://github.com/OWNER/REPO` | assets of the latest releases; `filter` picks file names |
| `github-folder` | `https://github.com/OWNER/REPO[/tree/BRANCH/PATH]` | files in the repository; `depth` = folders deep |
| `http-index` | any web folder listing (Apache, nginx …) | follows sub-folders `depth` deep |
| `webdav` | a WebDAV folder | sign-in: user + password, token or a header |
| `nextcloud-share` | `https://HOST/index.php/s/TOKEN` | the share's password, if it has one |
| `archive-org` | `https://archive.org` | `filter` is the search query; files are read at install time |
| `local` | a folder on this computer or a network drive | |
| `telegram` | `https://t.me/s/CHANNEL` | listed; the files open in Telegram |

For sources that do not say, `source_lang`, `target_lang` and `license` set
the defaults, and `name_pattern` reads languages from file names with the
groups `(?P<src>…)` and `(?P<dst>…)`, e.g. `^(?P<src>[a-z]{3})-(?P<dst>[a-z]{3})`.

## Where dictz keeps it

In the data folder (`dct paths`):

| File | What is in it |
|---|---|
| `center/sources.json` | the sources: order, on/off, default, every field above |
| `center/credentials.json` | passwords and tokens, only on this computer (mode 600); `DICTZ_SOURCE_<ID>_PASSWORD` / `_TOKEN` override it |
| `center/cache/<source>.json` | each source's entries, read last time |
| `center/installed.json` | what was installed: entry, version, folder, files, checksums |
| `center/downloads.json` | the download history (last 500) |
| `center/downloads/*.part` | unfinished downloads; they continue where they stopped |
| `dictionaries/<source>/<dictionary>/` | the installed dictionaries (setting `center.install_folder`) |

All are plain JSON written atomically with a `.bak` copy, so both editions
can share one data folder.

# dictz-data

The default dictionary server of [dictz](https://github.com/bayramkotan/dictz):
free and open dictionaries, each with its licence, listed in one
[`catalog.json`](catalog.json) that the Dictionary Center reads
(`https://raw.githubusercontent.com/bayramkotan/dictz-data/main/catalog.json`).

This folder is the starting point of that repository; it lives in
`docs/dictz-data/` of the dictz repository until it is copied out.

## Layout

```
catalog.json                 ← built by `dct catalog build`, do not edit by hand
LICENSES.md                  ← every dictionary, its licence and where it came from
<pair>/<dictionary files>    ← e.g. en-tr/, tr-en/, ota-tr/
<pair>/<file>.json           ← sidecar: licence, author, homepage, description …
```

## Adding a dictionary

1. Check the licence. Only dictionaries whose licence allows redistribution
   go here (open licences: CC0, CC BY, CC BY-SA, GPL, MIT, …; see
   [docs/CATALOG.md](https://github.com/bayramkotan/dictz/blob/main/docs/CATALOG.md#licences)).
2. Put its files in a folder (StarDict `.ifo/.idx/.dict.dz`, DSL `.dsl.dz`,
   Babylon `.bgl`, or an archive of them).
3. Write a sidecar `<first file>.json` with at least `license`, `author` and
   `homepage`; add the dictionary to `LICENSES.md`.
4. Rebuild and check the catalog:

   ```bash
   dct catalog build . --base-url https://raw.githubusercontent.com/bayramkotan/dictz-data/main/ \
       --name dictz-data -o catalog.json
   dct catalog check catalog.json
   ```

5. Commit. dictz users get it at the next catalog refresh.

Files over 100 MB do not fit in a GitHub repository: attach them to a
GitHub release of this repository, list them in a hand-written
`releases/catalog.json` (absolute `url`, `size`, `sha256`, `license`) and add
`"include": ["releases/catalog.json"]` to the head of `catalog.json` after
each build.

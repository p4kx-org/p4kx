# Content search scans on demand and keeps no index

Searching for text in the contents of an Archive, not only in paths and Record names, can work two
ways: scan the sources for each search, or build an index once and query it. An external tool,
reviewed for facts only, keeps a SQLite FTS5 index with the trigram tokenizer over the 48,249 XML
documents of LIVE 4.10 and skips files above 64 MiB; a substring such as `Gladi` finds
`AEGS_Gladius` with it.

## Decision

`p4kx-search`, a service crate, scans on demand and keeps nothing on disk. Three sources, each
searched by a parallel scan that streams:

- **Records.** The canonical tree of every Record, in Record order, as the Record preview and the
  Query read it (`TreeBuilder::build`, one builder per task). The string, localisation key and
  enum values are matched; a hit names the field path with the index of every array element, like
  `Components[3].AttachDef.Tags`. A Record reports at most `per_item` hits (3 by default).
- **Localisation.** The `global.ini` of one language at a time is read, its lines are searched as
  the file lists them, and the file is dropped before the next language is read; the key is the
  text before the `=` and only the text after it is matched. A key with a comma suffix is a line
  of its own.
- **Entries.** The lines of text Entries and of `CryXml` Entries, converted to XML as the preview
  converts them, so that a line number and a byte offset of a hit are those of the preview. Plain
  UTF-8 is matched incrementally across complete lines. Per-entry literal state is proportional
  to the compiled needle, not the line length; excerpt state retains only its bounded context.
  Original UTF-8 and binary classification continue through the whole admitted line, including
  bytes after the first match. `CryXml` and
  UTF-16 are read whole, up to 32 MiB, under a budget of 256 MiB for all tasks together, counting
  what is derived from them and the per-entry literal/excerpt state (ADR-0155). Without
  a Filter the Entries with one of 15 text extensions (`xml`, `mtl`, `cdf`, `adb`, `entxml`,
  `chrparams`, `animevents`, `ini`, `cfg`, `json`, `txt`, `lua`, `csv`, `eco`, `tbl`) are searched,
  with a Filter every Entry whose path matches; the `global.ini` files are left to the
  localisation source when that is searched too. An Entry above 64 MiB, one with binary data (a
  control character other than tab, line feed, form feed and carriage return, or invalid UTF-8),
  an encrypted one without the Archive key and one that cannot be read are skipped and counted.

Matching is a case-insensitive substring match: the text is lower-cased character by character
with Unicode rules (`GLÄD` finds `Glädius`, `K` the Kelvin sign), `ß` is not folded to `ss`, and
text that is ASCII on both sides takes a byte loop for a whole-string search. Entry matching reuses
the same compiled folded literal with prefix fallbacks. Matches start only at an original Unicode
character boundary and may end inside that character's lowercase expansion; their byte range still
includes the complete original character.

**Order, bounds and cancellation.** Each source visits its items in a fixed order (Records in
Record order, languages in `Language::ALL` order and lines in file order, Entries in path order),
runs a wave of tasks of a few per thread in parallel, appends their results in task order and
stops once more than `limit` hits exist (1,000 by default, at most 10,000, per source), so the
hits, `has_more` and the counts depend on the data alone, not on the number of threads. No wave
starts after the limit. Record/localisation tasks retain their source polling boundaries.
Entry scanning polls between Entries and blocks of at most 64 KiB of decoded UTF-8 input; this
is a cancellation work stride, not a search or line cut. One true Entry-search stop response is
latched for that invocation, so later false responses cannot restore a partial result. Nothing runs inside an `OnceLock` initialiser: the build
of the path order of the DataForge happens before the tasks start. A hit holds an excerpt of at
most 32 characters before and 64 after the match with control characters and bidirectional
controls replaced, so text from an Archive cannot drive a terminal; the CLI and the window show
paths, keys and field names through the same filter.

**The Archive key.** An encrypted Entry needs the user's key (ADR-0027). Without it, or with a
wrong one, it is counted as `entries_key_required` and the CLI prints a hint; the search of the
other sources goes on. A search never asks for a missing key: an encrypted Entry is counted before it
is opened when the Archive has no key, so `Archive::take_key_request` stays false, and the window
shows the count as an alert with a button that opens the key dialog for the user to decide. With
a key set the Entries are opened as before and a wrong key is counted like a missing one. An encrypted DataForge is an error with exit code 7, an Archive without a
DataForge is reported as unavailable for the Records source.

**Measured** on LIVE 4.10 without the Archive key, in release mode on the development machine:
all 117,709 Records in 0.7 s (1.2 s with the DataForge read from the Archive), the 90,946 texts
of the English `global.ini` in 0.03 s and the texts of all languages in 0.15 s, and the 49,149
text Entries, 4,275 encrypted ones skipped, in 1.6 to 2.0 s. A search that ends at its limit
ends sooner.

## Considered Options

- **A persistent SQLite FTS5 trigram index** like the external tool. A scan of every source is
  already below the time of opening a dialog, and an index would first have to be built by one:
  seconds for the Records and Entries alone, more for the text a trigram index must hold, and
  then it is stale after every Build and every Archive change, so it needs a place per
  Channel and Build, an invalidation, a size limit and a cleanup. It would also write decrypted
  content of encrypted Entries to the user's disk, which the tool otherwise never does outside an
  Extraction the user asks for (ADR-0027 keeps even a hash of the key out of storage). It adds a
  native dependency, and the trigram tokenizer cannot answer a search for one or two characters.
- **A lazily built index in memory** after the model of the References index (ADR-0029). Trigram
  postings over the Records and Entries would take several hundred mebibytes per open Archive for
  a build that costs as much as one search, and it pays only after several searches. The scan
  needs no memory beyond its waves.
- **Rendering every Record to XML and matching the text**, as a reviewed tool does for each query.
  It allocates the whole XML of 117,709 Records per search; matching the leaf values of the tree
  finds the same values without it and names the field path.

Revisit when a source grows to a scan of more than a few seconds, or when searches over many
Archives at once are asked for.

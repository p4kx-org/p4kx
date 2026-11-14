# Exports state whether the Archive key was there

## Decision

An Archive counts the distinct encrypted Entries that were asked for without a key, or with one
that does not fit, and the first such Entry (`Archive::key_demand`, shared with the containers
opened from it). Unlike the one-shot request flag a window uses to ask for the key, reading the
record resets nothing.

The catalog and the ship base state it in their `source` block: `archiveKey` is `given` (a key was
set and no Entry failed with it), `missing` (no key was set) or `wrong` (a key was set and did not
fit), and `encryptedEntriesUnread` is the number of distinct encrypted Entries the export could not
read. `CatalogSummary::archive_key` and `BuiltShipBase::archive_key` give the same to library
callers, and the catalog summary of `p4kx catalog --json` carries `archive_key` and
`encrypted_entries_unread`.

The CLI prints one note on stderr from one place: `crate::key::open` registers the demand of every
Archive it opens, and `main` prints the aggregated note after the command, so no command prints
its own. The note names the cause (no key, or a key that does not fit), the number of Entries,
the first Entry and how to give the key. It is not printed when the command already failed with a
key error and printed that hint, and never for a command that asked for no encrypted Entry.

A missing key does not change the exit code of a command; whatever decides it today still does.

The MCP server counts per call: a `DemandScope` of the Archive's demand is opened when a tool call
takes the Archive and read when the call has run, in `noted::take`, for every tool at once. A
result whose call left encrypted Entries unread carries a text block with the hint of
`keys.rs` and `_meta.archive_key` and `_meta.encrypted_entries_unread`; with a key set the `_meta`
says `given`. The scope counts every distinct Entry asked for while it is open, whoever asked, so
a call running at the same time on the same Archive counts into it.

A result the server keeps across calls (the audio index and the Loadout trees) is built inside a
`DemandScope`, and its `KeyStamp` keeps the `ArchiveKeyUse` of that build with the key generation
(`Archive::key_generation`, raised when a different key is set or a key is cleared) it ran with.
A call that reuses it adds that use to its own report once, whichever thread reuses it, so its note
and `_meta` equal those of the call that built it; the building call does not add it again. A kept
result built with encrypted Entries unread is built again on next use once the generation is no
longer the one it was built with; a complete one is never built again for the key.

## Consequences

The catalog stays version 3 under ADR-0339: the two fields are additive in the unreleased version
and required in its JSON Schema. The ship base file stays at its version: its `source` block gains
the same two fields, which a reader ignoring unknown fields does not notice.

The issue codes of the contract are not split by cause; `source.archiveKey` covers the whole
export, and the count is per Archive since it was opened, so a key set later does not erase an
earlier demand.

The note does not group the Entries by folder: it names only the first, which keeps it to one line.

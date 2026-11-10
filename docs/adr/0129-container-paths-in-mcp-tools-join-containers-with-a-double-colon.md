# Container paths in the MCP tools join containers with a double colon, and a preview returns an image block

Decided for issue #286. Builds on ADR-0038 and ADR-0023.

The MCP server read only the Entries of the Archive itself. The rooms, props and materials of an
Entity live in socpak containers inside it, so a client could not look into them, and it had no way
to see a texture or to resolve a Loadout.

## Decision

- **A container path is `outer.socpak::inner.socpak::path`.** `::` does not occur in
  real Archives, so it separates the containers from the path inside the last one; an Entry that
  has it in its path is unreachable through container paths. Every
  segment before the last names a container Entry, matched ignoring case and separator kind like any
  Entry. `list_entries` takes it as `folder` (`outer.socpak::` is the top level), `search_entries`
  as `container`, and `read_entry_text`, `read_entry_bytes`, `image_preview` and `mtl_summary` as
  `path`. An empty container name is an error.
- **Listings return paths a client can pass on unchanged.** An Entry or folder inside a container is
  shown as the Entry path of its containers, `::` and its own path.
- **Containers stay open per Archive.** The last four opened containers of an Archive are kept with
  their folder tree and path index, and are dropped with the Archive. The limits of
  `Archive::open_container` (depth 8, 512 MiB decoded, entry count, compression ratio) apply.
- **`image_preview` returns the image as a PNG image content block** plus structured metadata, not
  only a path, because the client reads the image from the result and the server writes no file. It
  is bounded: at most 2048x2048 pixels (counting every image of the mip it decodes), 128 MiB of
  texture data, 1 MiB of PNG and two decodes at a time. The description carries the ADR-0038 sentence that media must not be redistributed.
- **Budgets:** `entity_loadout` and `mtl_summary` list in pages bounded by the page size and by
  the 256 KiB cap on the whole response, including fields around the list. Ports are at most 32
  levels deep (16 by default). Retained material and Loadout strings are complete. The existing
  Budget admits each actual retained text occurrence before its output copy; one material or
  port and its retained nested values share that item's budget. The response envelope has the
  same raw-text admission. Actual display formatting runs once through a bounded writer;
  independent formatter errors remain errors. Exact JSON admission still decides the page.
  An oversized later item ends the page with a cursor that retries that item. An item that
  cannot fit first is explicitly refused; other errors remain fatal.
  All admitted Entity notes are retained through the same bounded display and envelope admission;
  a note beyond the response budget causes an explicit refusal instead of disappearing.
  Shader parameters are retained in full within the same item and response admission.
  Remaining item lists retain their stated counts beside their previews: 64 texture slots,
  layers and Entity issues, 16 item types per port and 16 sub-types per item type.
  These preview and depth limits require separate completeness/retrieval assessment; counts
  alone do not establish their sufficiency. Texture paths remain relative to `Data`. Existing
  input byte limits apply independently when returned names or paths are reused.
  No tool runs code or changes state.

## Considered Options

- **A separate `container` argument on every tool:** rejected for the read tools, since one path
  string is what a listing returns and what a client copies. `search_entries` has no path, so it
  takes `container`.
- **`!` or `|` as the separator:** rejected. `!` is used by Java-style jar paths but is legal in
  Entry names; `::` is not found in any Archive path and reads clearly.
- **Return only the temporary path of a written PNG:** rejected. The server never writes and the
  client may run on another machine.

## Consequences

- A client reaches every file of a socpak without extracting it.
- The tools return pixels of game textures to the client, which may forward them; the notice in
  each description and in the server instructions says so.

## Amendment (issue #622)

- **The kept containers have a byte budget.** At most four containers and 256 MiB are kept per
  Archive; the least recently used are dropped first. A container is charged the decoded bytes it
  keeps in memory (`Archive::resident_len`) plus 256 bytes per Entry for its path index and folder
  tree. That per-Entry figure is an estimate, not a measurement. A container read in place from
  the file is charged no bytes; a stored container inside a decoded parent shares the parent's
  buffer and is charged all of it, because it keeps that buffer alive after the parent is
  dropped. A container charged more than 128 MiB is not kept: the calls that opened it use it and
  it is dropped with them. Two Archives are kept, so at most 512 MiB of kept containers are held.
- **Peak memory.** The gate bounds the decodes that run at the same time (two), not the memory
  they leave behind: a container that is too large to be kept lives as long as the calls using
  it. The worst case is one chain of at most 512 MiB (`MAX_CONTAINER_LEN`) per worker, so
  workers x 512 MiB besides the kept containers.
- **A container is decoded outside every lock and under its own permit.** Calls that need a
  container being decoded wait for that decode on their blocking thread, share its result and,
  if it fails, its error, so a broken container is decoded once. The permit comes from a gate of
  container decodes (two at a time) that is separate from the model gate of the chunk file,
  model and texture reads (two at a time), so long model reads do not keep decodes from running.
  A permit is taken only on a blocking thread that holds its worker permit, in the order worker
  permit, then one permit. A call holding a model permit must not open a container, since it
  would then wait for a decode while keeping a model permit; `image_preview` therefore takes its
  model permit after its path is resolved, and `Containers::open` asserts in debug builds that
  the thread holds none.
- **A path inside a container is found through its path index**, built once per container, not by
  a scan of its Entries per segment.
- **`entity_loadout` resolves a tree once.** The resolved tree of an Entity is kept per Archive,
  keyed by the Entity as written and `stop_at_docked_vehicles`. The trees kept hold at most 64Ki
  ports together (and at most eight trees), the least recently used going first; a tree with
  more ports is not kept. They are dropped with the Archive, which is opened anew when its file
  changes. Paging, and a changed
  `max_depth`, walk the kept tree. A page still costs one walk of the tree, but no resolving.
  Calls that miss at the same time each resolve, and the first tree kept wins.

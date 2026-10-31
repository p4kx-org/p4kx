app-title = P4KX
menu-archive = Archive
action-open = Open Archive…
action-open-dataforge = Open DataForge file…
action-add-folder = Add folder…
action-add-archive = Add Archive…
menu-recent = Recent Archives
recent-none = No recent Archives
channels-heading = Channels
channels-none = No Channels found. Add a folder or an Archive through the Archive menu.
sidebar-show = Show sidebar
sidebar-hide = Hide sidebar
menu-language = Language
language-de-DE = Deutsch
language-en-GB = English (UK)
menu-help = Help
font-licence = Font licence
update-check = Check for a new version at start
update-available = Version { $version } is available
dialog-archive-filter = Star Citizen Archive
dialog-dataforge-filter = DataForge database
column-name = Name
column-compression = Compression
column-encryption = Encryption
column-size = Size
select-all = Select all shown Entries
select-entry = Select { $name }
compression-store = Stored
compression-deflate = Deflate
compression-zstd = Zstandard
compression-other = Method { $method }
encrypted-yes = Yes
encrypted-no = No
expand-folder = Expand { $name }
collapse-folder = Collapse { $name }
search-label = Search
search-hint = Filter by path: a glob with * or ?, otherwise any part of the path
drop-hint = Drop a Data.p4k here to open it
empty-title = No Archive open
drop-release = Release to open
quick-filters = Quick filters
close-dialog = Close dialog
shortcut-reload = F5
shortcut-extract-selected = Ctrl+E
status-empty = Open an Archive or drop a Data.p4k onto the window.
status-opening = Opening { $path }…
status-failed = Cannot open { $path }: { $reason }
status-opening-stopped = Opening { $path } stopped without a result.
status-entries =
    { $count ->
        [one] { $number } Entry
       *[other] { $number } Entries
    }
status-shown = { $number } shown
status-selected = { $number } selected
status-selected-hidden = { $number } selected, { $hidden } not shown
status-searching = Searching…
status-build = Build { $build }
status-build-unknown = Build unknown
preview-tab-text = Text
preview-tab-hex = Hex
preview-copy = Copy
preview-none = Select an Entry to preview it.
preview-loading = Reading { $path }…
preview-too-large = { $path } is { $size }. Entries larger than { $limit } are not previewed; extract it to look inside.
preview-truncated = Truncated: showing the first { $shown } of { $total } bytes.
preview-failed = Cannot read { $path }: { $reason }
preview-stopped = Reading { $path } stopped without a result.
preview-archive-changed = The Archive changed on disk. Reload it to preview Entries.
archive-changed =
    { $stopped ->
        [yes] { $path } changed on disk, probably updated by the RSI Launcher. A running Extraction was stopped; reload the Archive to extract or preview Entries again.
       *[no] { $path } changed on disk, probably updated by the RSI Launcher. Reload the Archive to extract or preview Entries again.
    }
archive-issues =
    { $kind ->
        [one] The directory of the Archive has one defect in an Entry. The Entry stays listed, but its data may not be readable; an Extraction reports it if it fails, and `p4kx info` names it.
       *[other] The directory of the Archive has { $count } defects in its Entries. The Entries stay listed, but their data may not be readable; an Extraction reports those that fail, and `p4kx info` names them.
    }
game-running = Star Citizen is running. Use this tool with the game closed: reading continues, but the RSI Launcher can change the Archive when it updates, and the tool never writes into the installation.
archive-test-build =
    { $kind ->
        [one] This Archive belongs to the { $channel } Channel, which carries Star Citizen test builds. Material of test builds is confidential under the Star Citizen Terms of Service: do not share or publish anything taken from it.
       *[other] These Archives belong to the { $channel } Channels, which carry Star Citizen test builds. Material of test builds is confidential under the Star Citizen Terms of Service: do not share or publish anything taken from them.
    }
archive-test-build-nda = The content of EPTU may also be covered by a non-disclosure agreement and must not be shared.
action-reload = Reload
extract-selected = Extract selected…
extract-shown = Extract shown…
extract-all = Extract all…
extraction-setup-title = Extract Entries
extraction-scope-heading = Entries
extraction-scope-selected = Selected ({ $number })
extraction-scope-shown = Shown ({ $number })
extraction-scope-all = Whole Archive ({ $number })
extraction-exclude-heading = Leave out
extraction-exclude-hint = One Filter per line: a glob with * or ?, otherwise any part of the path
filter-too-long = A Filter is longer than { $max } bytes.
filter-too-many-wildcards = A Filter has more than { $max } wildcards (* and ?).
extraction-exclude-too-many = There are more than { $max } Filters.
extraction-exclude-empty = A line holds only spaces; remove it.
extraction-exclude-nothing-left = These Filters leave no Entry to extract.
extraction-target-heading = Target
extraction-target-new = New folder inside
extraction-target-existing = Existing folder
extraction-target-new-name = Creates { $name }
extraction-target-in-installation = This folder lies inside the Star Citizen installation { $installation }. The tool never writes there; choose a folder elsewhere.
extraction-folder-none = No folder chosen
extraction-choose-folder = Choose folder…
extraction-policy-heading = Files that already exist
extraction-policy-skip = Skip
extraction-policy-overwrite = Overwrite
extraction-policy-if-changed = Overwrite only if changed
extraction-conversion = Convert CryXml and DataForge to XML, textures and sounds
extraction-texture-merge = Merge split textures into one DDS
extraction-texture-png = Export textures as PNG (high-range as OpenEXR)
extraction-texture-mip-largest = Largest mip
extraction-texture-mip-level = Mip level
extraction-texture-raw = Keep the raw texture parts
extraction-audio-label = Sounds as
extraction-audio-ogg = Ogg
extraction-audio-wav = WAV
extraction-audio-raw = WEM, unchanged
extraction-audio-banks = Ogg falls back to WAV for a codec without an Ogg form. Soundbanks are also unpacked into a folder per bank, each WEM named by its ID.
extraction-start = Extract
extraction-close = Close
extraction-confirm-all =
    { $count ->
        [one] Extract the whole Archive, { $number } Entry of { $size }?
       *[other] Extract the whole Archive, { $number } Entries of { $size }?
    }
extraction-confirm-all-action = Extract whole Archive
extraction-back = Back
extraction-running-title = Extracting
extraction-progress = { $done } of { $total } Entries, { $percent } %
extraction-cancelling = Cancelling…
extraction-cancel = Cancel
extraction-summary-title = Extraction finished
extraction-summary-cancelled-title = Extraction cancelled
extraction-summary-changed-title = Extraction stopped: the Archive changed
extraction-summary-changed = The Archive changed on disk during the Extraction. Files written before may be stale; reload the Archive and extract again.
extraction-summary-counts = Extracted { $extracted }, skipped { $skipped }, failed { $failed }, cancelled { $cancelled }
extraction-summary-counts-converted = Extracted { $extracted } ({ $converted } converted), skipped { $skipped }, failed { $failed }, cancelled { $cancelled }
extraction-summary-excluded-parts =
    { $count ->
        [one] { $number } part Entry left out of a texture that stayed was still read to convert the texture.
       *[other] { $number } part Entries left out of textures that stayed were still read to convert the textures.
    }
extraction-summary-records = Records written { $written }, skipped { $skipped }, failed { $failed }, cancelled { $cancelled }; { $errors } error values in { $erroneous } Records
extraction-summary-target = Target: { $path }
extraction-summary-failed = Failed ({ $number })
extraction-summary-skipped = Skipped ({ $number })
extraction-summary-cancelled = Cancelled ({ $number })
extraction-summary-unconverted = Not converted ({ $number })
extraction-summary-failure = { $path }: { $reason }
extraction-summary-records-failed = Records not written ({ $number })
extraction-summary-record-failure = { $path } ({ $guid }): { $reason }
rename-heading = Renamed Records ({ $number })
rename-line = { $path } ({ $guid }): { $reason }
rename-name = the Record name is no valid file name ({ $rule }); the GUID names the file
rename-path = the record path is no valid file name ({ $rule }); the file name is escaped
rename-rule-empty = the name is empty
rename-rule-dot-part = the name is a dot part
rename-rule-forbidden-character = the character { $char } is not allowed in a file name
rename-rule-trailing-dot-or-space = the name ends with a dot or a space
rename-rule-device-name = the name is a Windows device name
rename-rule-too-many-bytes = the name is { $bytes } bytes in UTF-8, more than { $max }
extraction-failed = Cannot prepare the target in { $path }: { $reason }
extraction-stopped = The Extraction into { $path } stopped without a result.
extraction-failed-title = Extraction failed
comparison-tab-archive = Browse
comparison-tab-comparison = Comparison
comparison-old-archive = Old Archive
comparison-new-archive = New Archive
comparison-pick-archive = Choose an Archive
comparison-choose-file = Choose file…
comparison-levels = Levels
comparison-level-entries = Entries
comparison-level-records = Records
comparison-entries-skipped = The Entries were not compared; switch on Entries to list them.
comparison-start = Compare
comparison-cancel = Cancel
comparison-opening = Opening both Archives…
comparison-progress = { $done } of { $total } Entries, { $percent } %
comparison-preparing = Preparing the list…
comparison-cancelled = The Comparison was cancelled.
comparison-failed = The Comparison failed: { $reason }
comparison-stopped = The Comparison stopped without a result.
comparison-empty-title = No Comparison yet
comparison-empty-hint = Choose an old and a new Archive, then compare them.
comparison-fact-old = Old
comparison-fact-new = New
comparison-side = { $channel } · { $build }
comparison-channel-unknown = No Channel
comparison-added = Added
comparison-removed = Removed
comparison-changed = Changed
comparison-directories = By directory ({ $number })
comparison-root = (root)
comparison-column-directory = Directory
comparison-column-change = Change
comparison-column-path = Path
comparison-column-old-size = Old size
comparison-column-new-size = New size
comparison-no-size = –
comparison-save-report = Save report…
comparison-report-filter = JSON report
comparison-report-saving = Saving the report to { $path }…
comparison-report-saved = Report saved to { $path }
comparison-report-failed = Cannot save the report to { $path }: { $reason }
comparison-no-differences = No Entry differs between the Archives.
comparison-no-matches = No change matches the filter.
comparison-shown = { $number } shown
comparison-row = { $change }: { $path }

schema-tab = Schema
schema-label-structs = Structs
schema-label-properties = Properties
schema-label-enums = Enums
schema-label-options = Enum options
schema-counts = { $label }: { $added } added, { $removed } removed, { $changed } changed
schema-column-kind = Kind
schema-column-name = Name
schema-column-old = Old
schema-column-new = New
schema-column-old-records = Old Records
schema-column-new-records = New Records
schema-kind-struct = Struct
schema-kind-parent = Parent
schema-kind-property = Property
schema-kind-enum = Enum
schema-kind-option = Option
schema-no-differences = The schema does not differ between the DataForges.
schema-failed = The schemas could not be compared: { $reason }
schema-row ={ $change }: { $kind } { $name }

diff-previous = Previous difference
diff-next = Next difference
diff-position = Difference { $current } of { $total }
diff-head-old = Old
diff-head-new = New
diff-hunk = Lines { $first }–{ $last }
diff-hunk-empty = No lines
diff-loading = Reading the line diff of { $path }…
diff-binary = { $path } is binary, so there is no line diff.
diff-too-large = { $path } is too large for a line diff: a side holds more than { $size } or { $lines } lines.
diff-failed = Cannot diff { $path }: { $reason }
diff-stopped = The line diff of { $path } stopped without a result.
diff-equal = The lines of both sides are equal; only the stored bytes differ.
diff-line-context = Line { $line }: { $text }
diff-line-removed = Removed line { $line }: { $text }
diff-line-added = Added line { $line }: { $text }
diff-marker-removed = −
diff-marker-added = +
query-tab = Query
query-label = Query
query-hint = from EntityClassDefinition at "*/spaceships/*" select Category
query-run = Run Query
query-cancel = Cancel
query-loading = Reading DataForge…
query-indexing = Indexing References…
query-selecting = Selecting Records…
query-progress = { $done } of { $total } Records, { $percent } %
query-cancelled = The Query was cancelled.
query-failed = The Query failed: { $reason }
query-stopped = The Query stopped without a result.
query-archive-changed = The Archive changed on disk. Reload it to run Queries.
query-invalid = Line { $line }, column { $column }: { $reason }
query-empty-title = No Archive open
query-empty-hint = Open an Archive to run Queries over its DataForge.
query-prompt = Write a Query and run it with Ctrl+Enter, e.g. from AmmoParams select speed where speed >= 1500
query-no-rows = No Record matches the Query.
query-rows =
    { $count ->
        [one] { $number } Record
       *[other] { $number } Records
    }
query-rows-failed =
    { $count ->
        [one] { $number } Record, { $failed } failed
       *[other] { $number } Records, { $failed } failed
    }
query-unresolved =
    { $count ->
        [one] , { $number } unresolved Reference
       *[other] , { $number } unresolved References
    }
query-export-csv = Export CSV…
query-export-json = Export JSON…
query-filter-csv = CSV file
query-filter-json = JSON file
query-exporting = Exporting the result to { $path }…
query-exported =
    { $count ->
        [one] { $number } Record exported to { $path }
       *[other] { $number } Records exported to { $path }
    }
query-export-failed = Cannot export the result to { $path }: { $reason }
query-export-cancel = Cancel export
query-export-cancelled = The export to { $path } was cancelled.
query-page = Page { $page } of { $pages }
query-page-first = First page
query-page-previous = Previous page
query-page-next = Next page
query-page-last = Last page
query-completion-more =
    { $count ->
        [one] { $number } more name, keep typing
       *[other] { $number } more names, keep typing
    }
query-save = Save Query…
query-saved-title = Saved Queries
query-saved-none = No saved Queries yet. Save the Query in the editor to rerun it with one click.
query-saved-run = Run the saved Query { $name }
query-saved-rename = Rename
query-saved-delete = Delete
query-save-title = Save Query
query-rename-title = Rename saved Query
query-delete-title = Delete saved Query
query-name-label = Name
query-name-hint = e.g. Heavy ships
query-save-action = Save
query-rename-action = Rename
query-delete-action = Delete
query-delete-question = The saved Query “{ $name }” is deleted. The Query in the editor stays.
query-name-replaces = Replaces the saved Query of this name.
query-name-empty = Enter a name.
query-name-too-long = The name is longer than { $max } characters.
query-name-taken = Another saved Query has this name.
query-name-control = The name must not hold line breaks or other control characters.
query-name-full = The saved Queries are full: at most { $max } Queries and { $size } in all. Delete one first.
query-name-bad-text = The Query in the editor is empty or too long to save.

dataforge-node = DataForge
dataforge-node-loading = Loading…
preview-tab-dataforge = DataForge
dataforge-loading = Loading DataForge from { $path }…
dataforge-unsupported = DataForge version { $version } is not supported by this version of P4KX.
dataforge-failed = Cannot load DataForge from { $path }: { $reason }
dataforge-stopped = Loading DataForge from { $path } stopped without a result.
dataforge-archive-changed = The Archive changed on disk. Reload it to load DataForge.
dataforge-version = Version
dataforge-records = Records
record-search-label = Search Records
record-search-hint = Filter by record path or type: a glob with * or ?, otherwise any part
record-search-prompt = Type part of a record path or type name to find Records.
record-search-no-matches = No Record matches the search.
record-search-matches =
    { $count ->
        [one] { $number } Record
       *[other] { $number } Records
    }
dataforge-structs = Structs
dataforge-enums = Enums
dataforge-first-records = First Records
column-type = Type
column-record-path = Record path
record-change-progress = { $done } of { $total } Records, { $percent } %
record-change-moved = Moved
record-change-unreadable = Unreadable
record-change-moved-path = { $old } to { $new }
record-change-no-differences = No Record differs between the DataForges.
record-change-head-field = Field
record-change-field-count =
    { $count ->
        [one] { $number } change
       *[other] { $number } changes
    }
record-change-loading = Comparing the fields of { $path }…
record-change-failed = Cannot compare the fields of { $path }: { $reason }
record-change-stopped = Comparing the fields of { $path } stopped without a result.
record-change-only-new = The Record exists only in the new Archive.
record-change-only-old = The Record exists only in the old Archive.
record-change-only-path = Only the record path changed; the fields are equal.
record-change-no-field-changes = No field differs.
record-change-unreadable-old = The old Record cannot be read: { $reason }
record-change-unreadable-new = The new Record cannot be read: { $reason }
record-change-record-name = Record name
record-change-record-tag = Record tag
record-change-record-type = Record type
record-change-value-missing = –
record-change-value-null = null
record-change-value-struct = { $type } (struct)
record-change-value-array =
    { $count ->
        [one] { $number } element
       *[other] { $number } elements
    }
record-change-value-reference-at = { $path } · { $at }
record-change-value-error = Error: { $reason }
record-change-field-row = { $field }: { $old } to { $new }
change-extraction-action-old = Extract old ({ $number })…
change-extraction-action-new = Extract new ({ $number })…
change-extraction-scope-old = Changed Entries of the old Archive, in their old version ({ $number })
change-extraction-scope-new = Added and changed Entries of the new Archive ({ $number })
change-extraction-scope-filtered = Only the changes matching the Filter { $filter }
change-extraction-summary-changed = The Archive changed on disk during the Extraction. Files written before may be stale; compare the Archives again and extract the changes again.

preview-tab-image = Image
image-mip = Mip
image-format = Format
image-size = Size
image-mips = Mips
image-faces = Cubemap faces
image-layers = Layers
image-dimensions = { $width } × { $height }
image-none = –
image-shown = Mip { $level }: { $width } × { $height } pixels
image-decoding = Decoding mip { $level } of { $path }…
image-unsupported = The preview cannot decode { $format } textures. The Text and Hex tabs still show the data of { $path }.
image-failed = Cannot decode { $path }: { $reason }
image-unshowable = Every mip of { $path } is larger than the preview can show.
container-opening = Opening…
container-unreadable = Cannot open
container-unreadable-detail = Cannot open { $name }: { $reason }
container-over-budget = Not opened: memory limit
container-over-budget-detail = { $name } was not opened: the open containers already hold { $limit }. Collapse other containers and expand it again.
container-stopped = Opening stopped
container-stopped-detail = Opening { $name } stopped without a result. Expand it again to retry.
container-archive-changed = Archive changed
container-archive-changed-detail = The Archive changed on disk. Reload it to open { $name }.
container-status-entries =
    { $count ->
        [one] Container: { $number } Entry
       *[other] Container: { $number } Entries
    }

menu-export = Export
record-export-action = Record export…
record-export-title = Export Records
record-export-scope-heading = Records
record-export-scope-tree = Selected in the Record tree: { $name } ({ $number })
record-export-scope-tree-none = Selected in the Record tree (nothing selected)
record-export-scope-filter = Record path filter
record-export-filter-hint = A glob with * or ?, otherwise any part of the record path
record-export-form-heading = Form
record-export-form-json = JSON, one file per Record under its record path
record-export-form-ndjson = NDJSON, one file { $name } with one Record per line
record-export-form-flat-csv = Values as CSV, { $name }, one row per value
record-export-form-flat-csv-raw = Values as CSV, exact: no byte order mark, no ' before text
record-export-form-flat-ndjson = Values as NDJSON, { $name }, one object per value
record-export-tags = Write the tag path next to every tag Reference (JSON and NDJSON only)
record-export-start = Start export
record-export-running-title = Exporting Records
record-export-reading = Reading DataForge…
record-export-selecting = Selecting Records…
record-export-progress = { $done } of { $total } Records, { $percent } %
record-export-summary-title = Record export finished
record-export-summary-cancelled-title = Record export cancelled
record-export-summary-cancelled = The Record export was cancelled before every Record was written.
record-export-summary-counts = Records written { $written }, skipped { $skipped }, failed { $failed }, cancelled { $cancelled }
record-export-summary-errors = Records with error values: { $erroneous }, error values: { $errors }
record-export-summary-no-match = No Record matches the filter.
record-export-summary-target = Target: { $path }
record-export-summary-file-failed = Cannot write { $path }: { $reason }
record-export-summary-failed = Failed ({ $number })
record-export-summary-skipped = Skipped ({ $number })
record-export-summary-failure = { $path } ({ $guid }): { $reason }
record-export-failed-title = Record export failed
record-export-failed = The Record export into { $path } failed: { $reason }
record-export-stopped = The Record export into { $path } stopped without a result.

catalog-action = Basetool catalog…
catalog-file-filter = Basetool catalog (JSON)
catalog-file-filter-compressed = Basetool catalog (JSON, zstd-compressed)
catalog-running-title = Creating the Basetool catalog
catalog-reading = Reading DataForge…
catalog-progress = { $done } of { $total } Records checked, { $percent } %
catalog-crafting-progress = Reading the crafting data: { $done } of { $total } Records, { $percent } %
catalog-warning = { $kind ->
    [unreadable_reward_records] { $count } Records that could hold a reward source cannot be read, so their reward sources are missing.
    [unknown_modifier_ranges] { $count } modifier ranges have a type P4KX does not know, so they are missing from the blueprint modifiers.
    [ambiguous_dismantle] The DataForge holds { $count } dismantle processes instead of one, so no dismantle return is linked.
    [unreadable_vehicle_files] { $count } ships have a vehicle file that cannot be read, so their paints are missing.
    [unplaced_starmap_containers] { $count } system containers or placements cannot be read, so the starmap objects below them have no position.
    [unreadable_shop_registry] The shop registry cannot be read, so the catalog has no shops although the Build has some.
    [missing_shop_registry] The Archive holds no shop registry at the known path, so the catalog has no shops.
    [unreadable_shop_containers] { $count } shop containers cannot be read, so the shops in them may lack kind and brand.
    [unreadable_shop_inventories] { $count } shop inventory files cannot be read, so the shops that use them have no item list.
    [missing_shop_inventories] { $count } shop inventory files that shop containers name are not in the Archive, so the shops that use them have no item list.
    [no_shop_components] No shop component was found in the .soc files of the shop containers, so no shop has an item list.
   *[other] { $message }
}
catalog-warning-more = and { $number } more
catalog-writing = Writing { $path }…
catalog-summary-title = Basetool catalog created
catalog-kinds-heading = Kinds
catalog-summary-kind = { $kind ->
    [manufacturers] Manufacturers
    [items] Items
    [ships] Ships
    [commodities] Commodities
    [blueprints] Blueprints
    [resourceGroups] Resource groups
    [blueprintCategories] Blueprint categories
    [paints] Paints
    [loadouts] Loadouts
   *[other] { $kind }
}: { $written } written, { $skipped } skipped, { $complete ->
    [yes] complete
   *[no] partial
}
catalog-summary-loadouts = Loadouts: { $ports } ports, { $unresolved } unresolved, { $docked } docked vehicles, { $outside } Entities without an entry in items or ships
catalog-summary-skipped = Records skipped as development leftovers: { $number }
catalog-playability-heading = Playability
catalog-summary-playability = { $kind ->
    [items] Items
    [ships] Ships
   *[other] { $kind }
}: { $player } player, { $ai } AI, { $mission } mission, { $special } special, { $unclassified } unclassified
catalog-summary-failed = Records that could not be read and were left out ({ $number })
catalog-summary-failure = { $path } ({ $guid }): { $reason }
catalog-texts-heading = Texts
catalog-summary-texts = { $language ->
    [en] English
    [de] German
   *[other] { $language }
}: { $resolved } resolved, { $missing } missing, { $placeholders } placeholders
catalog-summary-texts-unavailable = { $language ->
    [en] English
    [de] German
   *[other] { $language }
}: the Archive holds no global.ini, { $missing } missing
catalog-summary-missing = Missing texts: { $number }
catalog-summary-size = Size: { $size } of the upload limit of { $limit }
catalog-summary-size-compressed = Size: { $compressed } compressed (of the upload limit of { $limit }), { $size } uncompressed
catalog-near-limit = The catalog is close to the basetool upload limit of { $limit }.
catalog-over-limit = The catalog is over the basetool upload limit of { $limit }; the basetool will reject the upload.
catalog-near-limit-plain = The plain JSON catalog is close to the basetool upload limit of { $limit }. Write the catalog as .json.zst for the upload; plain .json is for inspection.
catalog-over-limit-plain = The plain JSON catalog is over the basetool upload limit of { $limit }; the basetool will reject it. Write the catalog as .json.zst for the upload; plain .json is for inspection.
catalog-decompressed-near-limit = The decompressed catalog is close to the basetool limit of { $limit } for decompressed content.
catalog-decompressed-over-limit = The decompressed catalog is over the basetool limit of { $limit } for decompressed content; the basetool will reject it.
catalog-summary-file = File: { $path }
catalog-cancelled-title = Basetool catalog cancelled
catalog-cancelled = The Basetool catalog was cancelled. No file was written.
catalog-failed-title = Basetool catalog failed
catalog-failed = The Basetool catalog { $path } could not be created: { $reason }
catalog-stopped = The Basetool catalog { $path } stopped without a result.
catalog-output-is-archive = The Basetool catalog cannot be written to { $path }, because that is the open Archive. Choose another file.
catalog-output-gzip = The Basetool catalog is not written to { $path }, because gzip is not supported. Use a name ending in .json.zst for a compressed catalog or .json for a plain one.
preview-tab-xml = XML
xml-converting = Converting { $path } to XML…
xml-too-large = { $path } is { $size }. CryXml Entries larger than { $limit } are not converted in the preview; extract it with Conversion on to read it.
xml-failed = Cannot convert { $path } to XML: { $reason }
xml-stopped = Converting { $path } stopped without a result.
xml-truncated = Truncated: showing the first { $lines } lines, the first { $limit } of the XML.
xml-view = Converted XML

scroll-text = Text lines
scroll-hex = Hexadecimal rows
scroll-image = Picture
scroll-diff = Line differences
scroll-fields = Changed fields
scroll-lines = Record fields
scroll-localization = Localisation rows
scroll-saved-queries = Saved Queries
scroll-soundbank = Embedded WEMs
scroll-summary = DataForge summary
scroll-list = { $heading }, rows

preview-tab-audio = Audio
audio-codec = Codec
audio-channels = Channels
audio-sample-rate = Sample rate
audio-duration = Duration
audio-hertz = { $rate } Hz
audio-unknown = –
audio-play = Play
audio-pause = Pause
audio-position = Position
audio-volume = Volume
audio-time = { $position } / { $duration }
audio-decoding = Decoding { $path }…
audio-unsupported = The preview cannot play { $codec } sounds. The Text and Hex tabs still show the data of { $path }.
audio-too-long = { $path } is too long to preview: its decoded sound would exceed { $limit }.
audio-failed = Cannot decode { $path }: { $reason }
audio-output-failed = The sound cannot be played: { $reason }
audio-column-id = ID
audio-prefetched = Prefetched
audio-prefetched-note = WEM { $id } is only prefetched in this soundbank and cannot be played here. The loose WEM Entry with the same ID holds the whole sound.
audio-plugin = Plug-in
audio-plugin-note = WEM { $id } is plug-in media, such as a convolution reverb impulse, and holds no sound to play.
audio-midi = MIDI
audio-midi-note = Media { $id } is a MIDI file and holds no sound to play.
audio-bank-summary =
    { $size } · { $count ->
        [one] { $number } WEM
       *[other] { $number } WEMs
    }
audio-bank-empty = { $path } embeds no WEMs.
audio-bank-failed = Cannot read the soundbank { $path }: { $reason }
audio-wem-name = WEM { $id } of { $bank }
audio-wem-row = WEM { $id }, { $codec }, { $duration }
audio-unsupported-wem = The preview cannot play { $codec } sounds ({ $name }).

texture-parts =
    { $count ->
        [one] +{ $number } part
       *[other] +{ $number } parts
    }
texture-expand = Show the parts of { $name }
texture-collapse = Hide the parts of { $name }

preview-tab-record = Record
record-back = Back
record-forward = Forward
record-type = Type
record-guid = GUID
record-name = Name
record-head-value = Value
record-loading = Reading the Record { $path }…
record-failed = Cannot read the Record { $path }: { $reason }
record-stopped = Reading the Record { $path } stopped without a result.
record-archive-changed = The Archive changed on disk. Reload it to read the Record.
record-dangling = The Reference points to the Record { $guid }, which does not exist in this DataForge.
record-follow = Follow the Reference to { $path }
record-row = { $field }: { $value }
record-no-fields = The Record has no fields.
record-xml-failed = The Record XML of { $path } cannot be written: { $reason }

record-language-menu = Record texts
record-language-keys-only = Keys only
record-language-unavailable = The Archive holds no texts in { $language }, so only the keys are shown.
record-language-key-row = { $key }: { $text }
record-language-key-row-missing = { $key } (no text in this language)
record-language-load-failed = The texts in { $language } cannot be read, so only the keys are shown: { $reason }
localization-tab = Localisation
localization-label = Search
localization-hint = Key or text, e.g. item_Name or Aurora
localization-loading = Reading the localisation files…
localization-no-texts = The Archive holds no global.ini.
localization-failed = The localisation files cannot be read: { $reason }
localization-stopped = Reading the localisation files stopped without a result.
localization-archive-changed = The Archive changed on disk. Reload it to read the localisation files.
localization-empty-title = No Archive open
localization-empty-hint = Open an Archive to look up its localisation keys and texts.
localization-column-key = Key
localization-language-en = English
localization-language-de = German
localization-language-fr = French
localization-language-it = Italian
localization-language-es-ES = Spanish (Spain)
localization-language-es-419 = Spanish (Latin America)
localization-language-pt-BR = Portuguese (Brazil)
localization-language-ja = Japanese
localization-language-ko = Korean
localization-language-zh-Hans = Chinese (simplified)
localization-language-zh-Hant = Chinese (traditional)
localization-language-other = Other language
localization-missing = Missing
localization-keys =
    { $count ->
        [one] { $number } key
       *[other] { $number } keys
    }
localization-keys-shown =
    { $count ->
        [one] { $number } of { $total } keys
       *[other] { $number } of { $total } keys
    }
localization-searching = Searching…
localization-no-match = No key or text matches the search.
localization-copy-key = Copy key
localization-copy-text = Copy text
localization-notes = Some global.ini files were not read cleanly:
tags-tab = Tags
tags-label = Search
tags-hint = Tag path, e.g. Weapon.Gun
tags-loading = Reading the tags…
tags-no-tags = The DataForge of the Archive holds no tags.
tags-failed = The tags cannot be read: { $reason }
tags-stopped = Reading the tags stopped without a result.
tags-archive-changed = The Archive changed on disk. Reload it to read the tags.
tags-empty-title = No Archive open
tags-empty-hint = Open an Archive to browse its tag tree.
tags-column-tag = Tag
tags-column-guid = GUID
tags-column-legacy = Legacy ID
tags-count =
    { $count ->
        [one] { $number } tag
       *[other] { $number } tags
    }
tags-count-shown =
    { $count ->
        [one] { $number } of { $total } tags
       *[other] { $number } of { $total } tags
    }
tags-no-match = No tag path matches the search.
tags-copy-path = Copy tag path
tags-copy-guid = Copy GUID
sounds-tab = Sounds
sounds-entity-label = Entity
sounds-entity-hint = Class name, GUID or record path of an Entity
sounds-show = Show sounds
sounds-use-record = Use selected Record
sounds-search-label = Search
sounds-search-hint = Trigger or event name, * and ? allowed
sounds-export = Export sounds…
sounds-empty-title = No Archive open
sounds-empty-hint = Open an Archive to browse the sounds of its Entities.
sounds-loading = Reading the audio of the Archive…
sounds-failed = The audio of the Archive cannot be read: { $reason }
sounds-stopped = Reading the audio stopped without a result.
sounds-archive-changed = The Archive changed on disk. Reload it to read its audio.
sounds-issues =
    { $count ->
        [one] { $number } audio file could not be read; the triggers and sounds in it are missing.
       *[other] { $number } audio files could not be read; the triggers and sounds in them are missing.
    }
sounds-list-hint = Show an Entity, or search for a trigger or event, to list audio triggers.
sounds-list-loading = Listing the audio triggers…
sounds-not-an-entity = The selected Record is no Entity.
sounds-list-failed = The audio triggers cannot be listed: { $reason }
sounds-list-archive-changed = The Archive changed on disk. Reload it to list the audio triggers.
sounds-no-match = No trigger or event name matches the search.
sounds-no-triggers = The Entity names no audio trigger.
sounds-column-trigger = Trigger
sounds-column-state = Status
sounds-column-events = Wwise events
sounds-state-ok = Resolved
sounds-state-no-definition = No definition
sounds-state-no-event = No event
sounds-state-no-soundbank = No soundbank
sounds-count-entity =
    { $count ->
        [one] { $name }: { $number } audio trigger, { $resolved } with a soundbank
       *[other] { $name }: { $number } audio triggers, { $resolved } with a soundbank
    }
sounds-count-search =
    { $count ->
        [one] { $number } trigger
       *[other] { $number } triggers
    }
sounds-count-search-more = The first { $number } of { $total } triggers are shown. Make the text longer to narrow the search.
sounds-scroll-triggers = Audio triggers
sounds-scroll-variants = Sounds of the trigger
sounds-select-hint = Select a trigger to see its sounds.
sounds-variants-loading = Resolving the sounds…
sounds-undefined = No ATL definition has this trigger.
sounds-no-variants = The trigger leads to no sound.
sounds-variants-count =
    { $count ->
        [one] { $name }: { $number } sound
       *[other] { $name }: { $number } sounds
    }
sounds-breaks-heading =
    { $count ->
        [one] { $number } place where the way to a sound breaks
       *[other] { $number } places where the way to a sound breaks
    }
sounds-break-line = { $name }: { $reason }
sounds-break-line-in = { $name } in { $bank }: { $reason }
sounds-column-event = Wwise event
sounds-column-wem = WEM ID
sounds-column-source = Source
sounds-column-language = Language
sounds-column-soundbank = Soundbank
sounds-source-embedded = Embedded
sounds-source-prefetch = Prefetched
sounds-source-streamed = Streamed
sounds-source-plugin = Plug-in
sounds-source-external = External
sounds-source-midi = MIDI
sounds-source-other = Other
sounds-language-fallback = { $language } (fallback)
sounds-variant-row = { $name }: { $bank }, WEM { $id }
sounds-detail = { $path }, WEM { $id }, { $source }
sounds-select-variant = Select a sound to play it.
sounds-note-prefetch = Only the start of this streamed sound is embedded and its loose WEM Entry is missing, so it cannot be played.
sounds-note-plugin = A source plug-in generates this sound; no WEM holds it.
sounds-note-external = An external source supplies this sound at runtime; no file holds it.
sounds-note-midi = This sound is a MIDI file and cannot be played here.
sounds-note-missing = No soundbank or loose WEM Entry holds this WEM.
sounds-note-no-entry = The Archive has no Entry for the file that holds this sound, so there is no player.
sounds-language-label = Language
sounds-export-title = Export sounds
sounds-export-scope = Triggers
sounds-export-selected = The selected trigger: { $name }
sounds-export-selected-none = The selected trigger (none selected)
sounds-export-all =
    { $count ->
        [one] The { $number } trigger of the list with a soundbank
       *[other] All { $number } triggers of the list with a soundbank
    }
sounds-export-format = Sounds as
sounds-export-start = Export
sounds-export-running = Exporting sounds
sounds-export-planning = Planning the export…
sounds-export-progress = { $done } of { $total } sounds, { $percent } %
sounds-summary-title = Sounds exported
sounds-summary-cancelled-title = Sound export stopped
sounds-failed-title = Sound export failed
sounds-export-failed = The sounds cannot be exported into { $path }: { $reason }
sounds-export-stopped = Exporting the sounds into { $path } stopped without a result.
sounds-summary-cancelled = The export was stopped. The sounds not yet handled are not written.
sounds-summary-counts = Sounds written { $written }, kept { $skipped }, failed { $failed }
sounds-summary-target = Target: { $path }
sounds-summary-unconverted =
    { $count ->
        [one] { $number } sound was written unchanged because it cannot be converted
       *[other] { $number } sounds were written unchanged because they cannot be converted
    }
sounds-summary-failed =
    { $count ->
        [one] { $number } sound was not exported
       *[other] { $number } sounds were not exported
    }
sounds-summary-file = { $name }: { $reason }
sounds-summary-failure = { $name }, WEM { $id }: { $reason }
sounds-summary-failure-plain = { $name }: { $reason }
tags-irregular =
    { $count ->
        [one] The tag tree differs from what the TagDatabase lists in { $number } place. The command p4kx dcb tags counts it.
       *[other] The tag tree differs from what the TagDatabase lists in { $number } places. The command p4kx dcb tags counts them.
    }
preview-tab-model = Model
model-reset = Reset view
model-hint = Drag: orbit · right-drag or Shift-drag: pan · wheel: zoom · arrows: orbit, Shift and arrows: pan · + and -: zoom · Home: reset
model-vertices = Vertices
model-triangles = Triangles
model-extent = Size
model-dimensions = { $width } × { $depth } × { $height } m
model-view = 3D view of { $path }: { $vertices } vertices, { $triangles } triangles
model-loading = Reading { $path }…
model-failed = Cannot read { $path }: { $reason }
model-file-too-large = { $path } or its geometry companion is { $size }, above the { $limit } the viewer reads.
model-empty = { $path } has no triangles to show.
model-too-large = { $path } has { $vertices } vertices and { $triangles } triangles, more than the viewer shows: { $max_vertices } vertices and { $max_triangles } triangles.
model-not-finite = { $path } has no usable coordinates.
model-gpu-none = The window has no graphics device for the 3D viewer. Everything else works as usual.
model-gpu-texture-side = The graphics device takes textures of { $limit } pixels a side at most, too few for the 3D viewer. Everything else works as usual.
model-gpu-buffer-size = The graphics device takes buffers of { $limit } bytes at most, too few for the 3D viewer. Everything else works as usual.
model-gpu-setup = The graphics device refused the 3D viewer: { $reason } Everything else works as usual.
model-gpu-lost = The graphics device was lost. Restart P4KX to see models again.
model-gpu-failed = The graphics device failed to draw the model. Select another model or restart P4KX.
model-gpu-rejected = { $path } is too large for the graphics device.
model-skeleton = Skeleton
model-loop = Loop
model-play = Play
model-pause = Pause
model-position = Animation position
model-time = { $position } / { $duration }
model-animations = Animations
model-bind-pose = Bind pose


preview-tab-incoming = Referenced by
scroll-incoming = Referring Records
incoming-head-field = Field
incoming-head-record = Referring Record
incoming-loading = Finding the Records that refer to { $path }…
incoming-failed = Cannot find the Records that refer to { $path }: { $reason }
incoming-stopped = Finding the Records that refer to { $path } stopped without a result.
incoming-archive-changed = The Archive changed on disk. Reload it to find the Records that refer to a Record.
incoming-none = No Record refers to this Record.
incoming-follow = Open the Record { $path }, which refers to this Record in { $field }
incoming-summary = { $references } from { $records }
incoming-reference-count =
    { $count ->
        [one] { $number } Reference
       *[other] { $number } References
    }
incoming-record-count =
    { $count ->
        [one] { $number } Record
       *[other] { $number } Records
    }
incoming-truncated = The first { $shown } of { $total } References are listed.
incoming-incomplete = Some Records could not be read or the index is full, so the list may miss References.

preview-key-required = { $path } is encrypted. Enter the Archive key to read it.
preview-key-rejected = The Archive key does not decrypt { $path }. Enter the right key to read it.
key-menu = Archive key…
key-title = Archive key
key-needed = An encrypted Entry needs the key of this Archive. Enter it once; it is stored as a file in your user profile.
key-rejected = The key in use does not decrypt this Archive. Enter the right key.
key-intro = Encrypted Entries can only be read with the key of the Archive. Enter it once; it is stored as a file in your user profile.
key-label = Key
key-hint = 32 hexadecimal digits
key-show = Show the key
key-stored = A key is stored: { $masked }
key-not-stored = No key is stored.
key-save = Save
key-cancel = Cancel
key-remove = Remove stored key
key-error-length = A key has 32 hexadecimal digits; you entered { $digits }.
key-error-character = Character { $position } is not a hexadecimal digit.
key-error-wrong = This key does not decrypt the Archive.
key-error-check = The key could not be checked: { $reason }
key-error-store = The key could not be stored: { $reason }
key-error-remove = The stored key could not be removed: { $reason }
key-checking = Checking the key against the Archive…
key-note-variable = The environment variable P4KX_ARCHIVE_KEY does not hold a valid key and is ignored.
key-note-store = The stored key cannot be used. Enter the key again to replace it. ({ $reason })
about-title = About P4KX
about-version = Version { $version }
about-copyright = Copyright (c) 2026 the P4KX contributors
about-licence = Licensed under MIT OR Apache-2.0.
about-source = Source code:
about-fan-project = P4KX is an unofficial Star Citizen fan project, not affiliated with the Cloud Imperium group of companies and not endorsed by it.
about-no-game-data = It ships no game data and reads only your own Star Citizen installation.
about-responsibility = You are responsible for complying with the Star Citizen End User Licence Agreement and Terms of Service when you use it.
about-owners = All content not authored by the P4KX contributors or its users is the property of its respective owners.
about-trademark = Star Citizen®, Roberts Space Industries® and Cloud Imperium® are registered trademarks of Cloud Imperium Rights LLC.
preview-tab-model-info = Info
model-info-geometry = Geometry file
model-info-geometry-in-file = In the model file
model-info-nodes = Nodes
model-info-meshes = Meshes
model-info-lods = Levels of detail
model-info-materials = Material library
model-info-material-ids = Material IDs
model-info-material-ids-none = None
model-info-bones = Bones
model-info-animations = Animations
model-info-animations-unread = Not read
model-info-animations-reading = Reading…
model-info-chunks = Chunks
model-info-chunks-value = { $total } ({ $read } read, { $skipped } skipped, { $unknown } unknown)
model-info-materials-unnamed = None named
model-info-materials-loaded = { $library }, { $count ->
    [one] one sub-material
   *[other] { $number } sub-materials
  }
model-info-materials-single = { $library }, one material for every ID
model-info-materials-missing = { $library } (does not exist)
model-info-materials-unreadable = { $library } (cannot be read: { $reason })
model-info-lods-heading = Levels of detail
model-info-column-lod = LOD
model-info-column-vertices = Vertices
model-info-column-triangles = Triangles
model-info-column-meshes = Meshes
model-info-chunks-heading = Chunks not read ({ $number })
model-info-column-file = File
model-info-column-type = Type
model-info-column-type-id = Type ID
model-info-column-usage = Usage
model-info-usage-skipped = Known, not read
model-info-usage-unknown = Unknown, skipped
model-info-file-model = Model file
model-info-file-geometry = Geometry file
model-info-file-lod = LOD { $level } geometry
model-info-type-unknown = Unknown
model-export-action = Export as GLB…
model-export-title = Export as GLB
model-export-model = Model: { $name }
model-export-lod-heading = Level of detail
model-export-lod = LOD { $level }, { $triangles ->
    [one] one triangle
   *[other] { $number } triangles
  }
model-export-materials-heading = Materials
model-export-materials-colours = Plain colours, a small and quick file
model-export-materials-textures = With textures, a larger file
model-export-texture-mip-heading = Texture size
model-export-texture-mip = { $level ->
    [0] Full size
   *[other] 1/{ $divisor } of the size per side, mip { $level }
  }
model-export-animations-heading = Animations
model-export-animations = { $count ->
    [one] Write the animation of the model
   *[other] Write the { $number } animations of the model
  }
model-export-animations-unknown = Write the animations of the model
model-export-animations-large = They are read in full, and the file can be large.
model-export-vertex-colours = Write the vertex colours
model-export-file-heading = File
model-export-choose-file = Choose file…
model-export-file-none = No file chosen
model-export-file-filter = glTF binary files
model-export-start = Export
model-export-running-title = Exporting model
model-export-reading = Reading { $name }…
model-export-progress = { $done } of { $total } written, { $percent } %
model-export-summary-title = Model exported
model-export-summary-cancelled-title = Export cancelled
model-export-summary-cancelled = The export was cancelled. No file was written.
model-export-summary-file = Written to { $path }
model-export-summary-counts = Nodes { $nodes }, meshes { $meshes }, materials { $materials }, vertices { $vertices }, triangles { $triangles }, size { $size }
model-export-summary-bones = Bones { $bones }, skinned meshes { $skinned }
model-export-summary-library-missing = The material library { $library } does not exist; the default material is used.
model-export-summary-library-unreadable = The material library { $library } cannot be read ({ $reason }); the default material is used.
model-export-summary-default-materials = { $count ->
    [one] One material ID has no sub-material in the library; the default material is used for it.
   *[other] { $number } material IDs have no sub-material in the library; the default material is used for them.
  }
model-export-summary-unnamed-materials = { $count ->
    [one] One entry of the material library is no material; the default material is used for it.
   *[other] { $number } entries of the material library are no material; the default material is used for them.
  }
model-export-summary-missing-layers = { $count ->
    [one] One layer file of the material library is missing or unreadable; its materials keep the colours of their own file.
   *[other] { $number } layer files of the material library are missing or unreadable; their materials keep the colours of their own file.
  }
model-export-summary-hidden = { $count ->
    [one] One submesh was left out because its material draws nothing.
   *[other] { $number } submeshes were left out because their material draws nothing.
  }
model-export-summary-chunks = Chunks not exported ({ $number })
model-export-summary-chunk-unknown = Skipped unknown chunk { $index } { $type_id } ({ $file })
model-export-summary-chunk-skipped = Chunk { $index } { $type } ({ $type_id }) is not read ({ $file })
model-export-summary-textures = Missing textures ({ $number })
model-export-summary-texture = { $material }: { $role ->
    [diffuse] diffuse
    [normal] normal
    [specular] specular
   *[other] { $role }
  } { $file } ({ $reason })
model-export-summary-animations = Animations { $written }, channels { $channels }
model-export-summary-animations-unread = Animations not written ({ $number })
model-export-summary-animation-unread = { $name } ({ $source }): { $reason }
model-export-summary-animation-file-unread = { $source }: { $reason }
model-export-summary-animation-unmatched = { $name } ({ $source }): no track matches a bone, { $tracks ->
    [0] no track skipped
    [one] one track skipped
   *[other] { $number } tracks skipped
  }
model-export-failed-title = Export failed
model-export-failed = { $name } could not be exported: { $reason }
model-export-no-record = The DataForge has no Record { $guid }.
model-export-stopped = The export of { $name } stopped without a result.
model-export-key-required = { $name } is encrypted and the Archive has no key. Enter the Archive key and export again.
model-export-key-rejected = The Archive key does not decrypt { $name }. Enter the right key and export again.
model-export-archive-changed = The Archive changed on disk, so { $name } was not exported. Reload the Archive and export again.
scroll-hits = Search hits
content-search-tab = Search
content-search-label = Search
content-search-hint = Text to find, e.g. Gladi
content-search-run = Search
content-search-cancel = Cancel
content-search-sources = Look in
content-search-source-records = Records
content-search-source-localization = Localisation
content-search-source-entries = Entries
content-search-empty-title = No Archive open
content-search-empty-hint = Open an Archive to search the contents of its Records, localisation texts and Entries.
content-search-prompt = Type a text and press Search. It is found as part of a longer text, whatever its case.
content-search-searching = Searching…
content-search-cancelled = The search was cancelled.
content-search-failed = The search failed: { $reason }
content-search-stopped = The search stopped without a result.
content-search-archive-changed = The Archive changed on disk. Reload it to search it.
content-search-no-match = Nothing matches “{ $text }”.
content-search-none-here = This source has no hit.
content-search-segment = { $number }
content-search-segment-more = { $number }+
content-search-hits =
    { $count ->
        [one] { $number } hit in { $seconds } s
       *[other] { $number } hits in { $seconds } s
    }
content-search-more = More matches exist. Only the first { $number } of a source are kept; make the text longer to narrow the search.
content-search-skipped =
    { $count ->
        [one] { $number } Entry was not searched: { $binary } binary, { $large } too large, { $unreadable } unreadable.
       *[other] { $number } Entries were not searched: { $binary } binary, { $large } too large, { $unreadable } unreadable.
    }
content-search-key-required =
    { $count ->
        [one] { $number } Entry is encrypted and was not searched: the Archive has no key, or the key does not match.
       *[other] { $number } Entries are encrypted and were not searched: the Archive has no key, or the key does not match.
    }
content-search-key-button = Enter the Archive key
content-search-no-dataforge = The Archive holds no DataForge, so no Record was searched.
content-search-column-record = Record path
content-search-column-field = Field
content-search-column-text = Text
content-search-column-language = Language
content-search-column-key = Key
content-search-column-entry = Entry
content-search-column-line = Line
content-search-row = { $place }: { $text }
entities-tab = Entities
entities-label = Search
entities-hint = Class name or record path, e.g. AEGS_Gladius
entities-empty-title = No Archive open
entities-empty-hint = Open an Archive to pick a ship, vehicle or weapon.
entities-loading = Listing the Entities…
entities-none = The DataForge of the Archive holds no ships, vehicles or weapons.
entities-failed = The Entities cannot be listed: { $reason }
entities-stopped = Listing the Entities stopped without a result.
entities-archive-changed = The Archive changed on disk. Reload it to list the Entities.
entities-filter-all = All
entities-filter-ships = Ships
entities-filter-vehicles = Vehicles
entities-filter-weapons = Weapons
entities-count =
    { $count ->
        [one] { $number } Entity
       *[other] { $number } Entities
    }
entities-count-shown = { $number } of { $total } Entities
entities-no-match = No Entity matches the search.
entities-loadout-heading = Loadout
entities-loadout-none = Select an Entity to see its Loadout.
entities-loadout-resolving = Resolving the Loadout…
entities-loadout-failed = The Loadout cannot be resolved: { $reason }
entities-loadout-no-record = The Loadout cannot be resolved: the DataForge has no Record { $guid }.
entities-loadout-lost = The Loadout cannot be resolved: the worker stopped unexpectedly.
entities-loadout-stopped = Resolving the Loadout stopped without a result.
entities-loadout-archive-changed = The Archive changed on disk. Reload it to resolve the Loadout.
entities-loadout-no-ports = This Entity has no ports.
entities-loadout-mismatch = { $entity } does not fit the port { $port }, applied all the same: { $reason }
entities-changes = { $excluded } left out, { $replaced } replaced
entities-no-changes = The default Loadout
entities-reset = Reset changes
entities-export = Export…
entities-include = Include { $port }
entities-replace = Replace…
entities-restore = Restore
entities-status-excluded = Left out
entities-status-replaced = Replaced
entities-status-docked = Docked vehicle
entities-status-empty = Empty
entities-status-unresolved = Unresolved
entities-expand-port = Expand { $name }
entities-collapse-port = Collapse { $name }
entities-variants =
    { $count ->
        [one] { $number } variant
       *[other] { $number } variants
    }
entities-show-variants = Show the variants of { $name }
entities-hide-variants = Hide the variants of { $name }
entities-base-row = { $name }, { $variants }
entities-variant-row = { $name }, variant
entities-replace-title = Replace { $port }
entities-replace-takes = The port takes { $types }, { $single ->
    [yes] size
   *[other] sizes
  } { $sizes }.
entities-replace-takes-types = The port takes { $types }.
entities-replace-takes-sizes = The port takes { $single ->
    [yes] size
   *[other] sizes
  } { $sizes }.
entities-replace-takes-any = The port takes any Entity.
entities-replace-hint = Class name or record path
entities-replace-count = { $number } Entities, { $total } of them fit
entities-replace-none = No Entity matches the search.
entities-replace-misfit = Does not fit, { $facts }
entities-facts-sized = { $type }, size { $size }
entities-facts = { $type }
entities-facts-none = No item type
entities-view-label = 3D view of { $name }: { $parts } parts, { $triangles } triangles
entities-view-lod = Detail
entities-view-materials = Materials
entities-view-colours = Colours
entities-view-textures = Textures
entities-view-reading = Reading models… { $count }
entities-view-flattening = Preparing models… { $done } of { $total }
entities-view-texture-progress = Reading textures… { $done } of { $total }
entities-view-interior = Interior
entities-view-interior-progress = Reading the Interior… container { $done } of { $total }, { $models } of { $of } models
entities-view-stop = Stop
entities-view-stopped = Building the view was stopped.
entities-view-restart = Build again
entities-view-empty = No part of this Loadout has a model the viewer can show.
entities-view-archive-changed = The Archive changed on disk. Reload it to see the Loadout.
entities-view-key = The models are encrypted and the Archive key is missing or wrong.
entities-view-failed = The models cannot be read: { $reason }
entities-view-parts = Parts
entities-view-meshes = Meshes
entities-view-triangles = Triangles
entities-view-show-all = Show all ({ $count })
entities-view-issues = Parts not resolved: { $count }
entities-hide = Hide
entities-show = Show
entities-status-hidden = Hidden
assembly-export-title = Export Entity
assembly-export-form-heading = Output
assembly-export-form-glb = GLB file
assembly-export-form-package = Blender package
assembly-export-form-package-note = A folder with scene.json, one GLB file per mesh and the textures as PNG, for the Blender add-on.
assembly-export-folder-heading = Package folder
assembly-export-choose-folder = Choose folder…
assembly-export-folder-none = No folder chosen
assembly-export-package-replaces = The folder holds an earlier package. It is replaced once the new one is complete.
assembly-export-package-not-a-package = The folder is not empty and holds no package written by P4KX. Choose a new or an empty folder.
assembly-export-package-no-name = The path names no folder. Choose another one.
assembly-export-running-title-package = Exporting Blender package
assembly-export-writing-package = Writing the package of { $name }…
assembly-export-summary-title-package = Blender package written
assembly-export-summary-package-counts = Parts { $parts } ({ $modelled } with a model), nodes { $nodes }, meshes { $meshes }, textures { $textures }, materials { $materials }, palettes { $palettes }, files { $files }, vertices { $vertices }, triangles { $triangles }, size { $size }
assembly-export-summary-package-textures = { $count ->
    [one] One texture could not be written; its materials keep the plain colours.
   *[other] { $number } textures could not be written; their materials keep the plain colours.
  }
assembly-export-title-interior = Export Interior
assembly-export-action-interior = Export Interior…
assembly-export-entity = Entity: { $name }
assembly-export-container = Container: { $name }
assembly-export-interior-heading = Interior
assembly-export-interior = Include the rooms and props of the object containers
assembly-export-changes = Loadout: { $excluded } left out, { $replaced } replaced
assembly-export-changes-none = Loadout: the default one
assembly-export-picks-heading = Loadout alternatives
assembly-export-picks-hint = Body=1, Eggs=2 or 1
assembly-export-picks-note = For creatures and NPCs, whose game picks one alternative per group at random. Name a group and its index, separated by commas; a bare index applies to every group that has it. Counting starts at 0; groups left out keep the first alternative.
assembly-export-picks-invalid = Write each pick as group=index, for example Body=1, or as an index alone, separated by commas.
assembly-export-lod-heading = Level of detail
assembly-export-lod = LOD { $level }
assembly-export-materials-heading = Materials
assembly-export-materials-colours = Plain colours, a small and quick file
assembly-export-materials-textures = With textures, a larger file
assembly-export-texture-mip-heading = Texture size
assembly-export-texture-mip = { $level ->
    [0] Full size
   *[other] 1/{ $divisor }
  }
assembly-export-vertex-colours = Write the vertex colours
assembly-export-file-heading = File
assembly-export-choose-file = Choose file…
assembly-export-file-none = No file chosen
assembly-export-file-filter = glTF binary files
assembly-export-start = Export
assembly-export-running-title = Exporting Entity
assembly-export-running-title-interior = Exporting Interior
assembly-export-resolving = Resolving the Loadout of { $name }…
assembly-export-reading = Reading the models of { $name }…
assembly-export-reading-interior = Reading the Interior of { $name }… { $done } of { $total } model files
assembly-export-writing = Writing the file, { $size } so far…
assembly-export-summary-title = Entity exported
assembly-export-summary-title-interior = Interior exported
assembly-export-summary-interior-opened = Interior: containers opened { $containers }, parts { $parts }, models { $models } ({ $modelled } with geometry), lights { $lights }
assembly-export-summary-interior = Interior: object containers { $containers }, parts { $parts }, models { $models } ({ $modelled } with geometry), lights { $lights }
assembly-export-summary-cancelled-title = Export cancelled
assembly-export-summary-cancelled = The export was cancelled. No file was written.
assembly-export-summary-file = Written to { $path }
assembly-export-summary-mismatch = { $entity } does not fit the port { $port }, written all the same: { $reason }
assembly-export-summary-counts = Parts { $parts } ({ $modelled } with a model), nodes { $nodes }, meshes { $meshes }, materials { $materials }, vertices { $vertices }, triangles { $triangles }, size { $size }
assembly-export-summary-images = { $images ->
    [0] No textures are written; every material has its plain colour.
    [one] One texture is written for { $textured } of { $materials } materials.
   *[other] { $images } textures are written for { $textured } of { $materials } materials.
  }
assembly-export-summary-fallbacks ={ $count ->
    [one] One model lacks the chosen level of detail and is written at the nearest lower one.
   *[other] { $number } models lack the chosen level of detail and are written at the nearest lower one.
  }
assembly-export-summary-hidden = { $count ->
    [one] One submesh was left out because its material draws nothing.
   *[other] { $number } submeshes were left out because their material draws nothing.
  }
assembly-export-summary-missing-layers = { $count ->
    [one] One layer file of the material libraries is missing or unreadable; its materials keep the colours of their own file.
   *[other] { $number } layer files of the material libraries are missing or unreadable; their materials keep the colours of their own file.
  }
assembly-export-report-heading = Parts that could not be resolved in full ({ $number })
assembly-export-report-all = Every part was resolved.
assembly-export-report-skipped = Left out: { $path } ({ $name }): { $reason }
assembly-export-report-note = Note: { $path } ({ $name }): { $reason }
characters-tab = Characters
characters-empty-hint = Open an Archive to compose a Character.
characters-body-heading = Body
characters-body-male = Male
characters-body-female = Female
characters-body-from-head = The head decides the body.
characters-head-heading = Head
characters-head-none = No head chosen.
characters-head-line = { $name }, head rig { $rig }
characters-head-reading = Reading the CHF file…
characters-head-failed = The CHF file cannot be used: { $reason }
characters-head-from-archive = Choose from the Archive…
characters-head-from-file = Open a CHF file…
characters-head-remove = Remove the head
characters-armour-heading = Armour and clothing
characters-loadout-none = No Loadout chosen.
characters-loadout-line = Loadout: { $name }
characters-loadout-choose = Choose a Loadout…
characters-loadout-clear = Remove the Loadout
characters-pieces-none = No pieces added.
characters-piece-add = Add a piece…
characters-piece-remove = Remove { $name }
characters-choose-title-piece = Add a piece
characters-choose-title-loadout = Choose a Loadout
characters-choose-title-head = Choose a head
characters-choose-hint = Name or folder
characters-choose-count = { $number } of { $total }
characters-choose-none = Nothing matches the search.
characters-expressions-heading = Expressions
characters-expressions-load = Load the expressions of the head
characters-expressions-no-head = Choose a head to load its expressions.
characters-expressions-none = The head has no expressions.
characters-expression-neutral = Neutral face
characters-expression-strength = Strength { $percent } %
characters-export-action = Export…
characters-export-setup-title = Export Character
characters-export-setup-name = Character: { $name }
characters-export-package-note = A folder with scene.json and one GLB file of the Character: the skeleton, the shape keys and the materials, with their textures when the textures are chosen, and the animations if you choose them, for the Blender add-on.
characters-export-animations-heading = Animations
characters-export-animations-none = None
characters-export-animations-all = All
characters-export-busy = The Character is being built again. Export it once that is done.
characters-export-package-counts = Parts { $parts }, bones { $bones }, skinned meshes { $skinned }, shape keys { $morphs }, animations { $animations }, vertices { $vertices }, triangles { $triangles }, files { $files }, size { $size }
characters-export-counts = Parts { $parts }, bones { $bones }, vertices { $vertices }, triangles { $triangles }, size { $size }
characters-report-heading = Pieces that could not be fitted in full ({ $number })
characters-report-all = Every piece fits.
characters-report-skipped = Left out: { $name }: { $reason }
characters-report-note = Note: { $name }: { $reason }
characters-report-replaced = { $name } of the Loadout was replaced by { $by }, which has the same item type.
characters-export-running-title = Exporting the Character
characters-export-done-title = Character exported
characters-view-preparing = Preparing the Character…
characters-export-skipped =
    { $count ->
        [one] One piece could not be fitted and is not in the file.
       *[other] { $number } pieces could not be fitted and are not in the file.
    }
characters-chf-filter = CHF character files
ui-tab = UI
ui-mode-label = Source
ui-mode-files = Files
ui-mode-ships = Ships
ui-search-label = Search
ui-files-hint = GFx or SWF path
ui-ships-hint = Class name or path
ui-empty-title = No Archive open
ui-empty-hint = Open an Archive to draw its UI files and the screens of its ships.
ui-files-loading = Looking up the UI files…
ui-files-none = The Archive holds no GFx or SWF files.
ui-files-failed = The UI files cannot be listed: { $reason }
ui-files-stopped = Listing the UI files stopped without a result.
ui-files-archive-changed = The Archive changed on disk. Reload it to list the UI files.
ui-files-count =
    { $count ->
        [one] { $number } file
       *[other] { $number } files
    }
ui-files-count-shown = { $number } of { $total } files
ui-ships-none = The DataForge of the Archive holds no ships or vehicles.
ui-ships-count =
    { $count ->
        [one] { $number } ship or vehicle
       *[other] { $number } ships and vehicles
    }
ui-ships-count-shown = { $number } of { $total } ships and vehicles
ui-no-match = Nothing matches the search.
ui-screens-heading = Screens
ui-screens-pick = Pick a ship to list its screens.
ui-screens-loading = Resolving the Loadout…
ui-screens-failed = The screens cannot be listed: { $reason }
ui-screens-stopped = Listing the screens stopped without a result.
ui-screens-archive-changed = The Archive changed on disk. Reload it to list the screens.
ui-screens-none = This ship shows no screens.
ui-screen-own = The ship itself
ui-screen-size = { $width } × { $height } px
ui-size-label = Size
ui-size-stage = Stage
ui-size-wide-1024 = 1024 px wide
ui-size-wide-2048 = 2048 px wide
ui-size-full-hd = 1920 × 1080
ui-background-label = Background
ui-background-transparent = Transparent
ui-background-movie = Movie colour
ui-background-black = Black
ui-background-white = White
ui-power-label = State
ui-power-at-rest = At rest
ui-power-on = Powered on
ui-variables-label = Variables
ui-variables-hint = powerstate=1 isradar=1
ui-variables-invalid = Not a setting of the form NAME=VALUE: { $settings }
ui-format-label = Format
ui-format-png = PNG
ui-format-svg = SVG
ui-export = Export…
ui-export-running = Writing { $file }…
ui-export-done = Wrote { $file }: { $width } × { $height } px
ui-export-done-incomplete =
    { $count ->
        [one] Wrote { $file }: { $width } × { $height } px, with { $number } thing left out (see the Left out tab)
       *[other] Wrote { $file }: { $width } × { $height } px, with { $number } things left out (see the Left out tab)
    }
ui-export-failed = The export failed: { $reason }
ui-export-stopped = The export stopped without a result.
ui-export-is-archive = The file is the open Archive, which is never overwritten.
ui-export-archive-changed = The Archive changed on disk. Nothing was exported.
ui-export-written-archive-changed = Wrote { $file }, but the Archive changed on disk meanwhile. Check the file or export it again.
ui-export-dismiss = Close
ui-filter-png = PNG images
ui-filter-svg = SVG documents
ui-rendering = Drawing…
ui-render-failed = The picture cannot be drawn: { $reason }
ui-render-stopped = Drawing stopped without a result.
ui-render-archive-changed = The Archive changed on disk. Reload it to draw the picture.
ui-pick-file = Pick a file to see its picture.
ui-pick-screen = Pick a screen to see its picture.
ui-too-large = The picture is { $width } × { $height } px, too large to show here. Export it instead.
ui-shown = Picture: { $width } × { $height } pixels
ui-detail-contents = Contents
ui-detail-report = Left out
ui-contents-none = Nothing to show yet.
ui-report-none = Everything was drawn.
ui-report-more = { $number } more lines are not shown here.
ui-note-lossless = lossless, { $size }
ui-note-embedded = embedded image
ui-note-external = external image, { $size }
ui-note-part = part of another bitmap
ui-note-font =
    { $count ->
        [one] { $name }, { $number } glyph
       *[other] { $name }, { $number } glyphs
    }
ui-note-sprite =
    { $count ->
        [one] { $number } frame
       *[other] { $number } frames
    }
ui-fact-format = Format
ui-fact-stage = Stage
ui-fact-frames = Frames
ui-fact-tags = Tags
ui-fact-shapes = Shapes
ui-fact-bitmaps = Bitmaps
ui-fact-fonts = Fonts
ui-fact-sprites = Sprites
ui-fact-entity = Entity
ui-fact-port = Port
ui-fact-size = Size
ui-fact-canvases = Canvases
ui-fact-textures = Textures
ui-fact-vectors = Vector graphics
ui-fact-movies = Movies
ui-fact-keys = Keys
ui-fact-widgets = Widgets
ui-fact-texts-drawn = Texts drawn
ui-fact-texts-left-out = Texts left out
ui-kind-shape = Shape
ui-kind-morph-shape = Morph shape
ui-kind-bitmap = Bitmap
ui-kind-font = Font
ui-kind-sprite = Sprite
ui-kind-import = Import
ui-kind-canvas = Canvas
ui-kind-texture = Texture
ui-kind-vector = Vector graphic
ui-kind-movie = Movie
ui-kind-key = Localisation key
ui-kind-element = Widgets drawn
ui-kind-element-undrawn = Widgets not drawn
ui-report-skipped = Skipped
ui-report-effect = Drawn without effect
ui-report-left-out = Left out
ui-report-missing-canvas = Canvas not drawn
ui-report-missing-texture = Texture not in the Archive
ui-report-unknown-variable = Game variable that the screen does not read
ui-report-truncated = Stopped early: a limit of the composition was reached
ui-report-import = Import not found
ui-report-tag = Unreadable tag
ui-report-bitmap = Unreadable bitmap
ui-report-warning = Note

folder-conversion-action = Convert folder…
folder-conversion-running-title = Converting folder
folder-conversion-progress = { $done } of { $total } files, { $percent } %
folder-conversion-scanning = Reading the folder…
folder-conversion-summary-title = Folder converted
folder-conversion-summary-cancelled-title = Conversion cancelled
folder-conversion-summary-counts = Written { $extracted } ({ $converted } converted), kept { $skipped }, failed { $failed }, cancelled { $cancelled }, left out without a conversion { $left }
folder-conversion-summary-input = Input folder: { $path }
folder-conversion-failed-title = Conversion failed
folder-conversion-failed = Cannot convert { $path }: { $reason }
folder-conversion-stopped = The conversion of { $path } stopped without a result.

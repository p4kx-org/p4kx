# Canonical catalog TextForms

Catalog schema 3 stores one canonical text row per existing `(Domain, key)`, retaining the exact
authored key and existing IDs. Public names and public short UI format templates use
`texts.publicNames`; protected descriptions and lore use `texts.gameTexts`. The existing protected
owner/reference/table context protects both forms. The shared TextForms type has no global
`x-gameText` annotation. Encryption origin is an independent axis.

## Language policy and source diagnostics

`texts.languagePolicy` records `selection: "availableSupported"` and `fallback: "none"`.
The CLI and GUI Catalog writers use this shared policy. Each actual supported language appears
in the canonical text rows when the Archive contains its selected `global.ini`.

`texts.languageSources` contains one sourced cell for each supported language code, including
when the catalog has no canonical text rows. A present cell includes the exact Entry `path`,
the decoder-selected `encoding`, the `lossyDecoding` indicator and the `skippedMalformedLines` count.
The index counts nonblank lines without `=` or with an empty key as malformed. The count
describes the whole Entry. Individual text forms retain their own winning value or absence.

The path cites directory metadata. The other three fields cite the corresponding decoding or
indexing operation on the actual Entry in the `globalIni` source namespace. A language with no
matching Entry has state `absent`; the source set is complete if the root directory survey
established the absence. Archive reads and index failures use the existing error path.
[Catalog design](adr/0356-catalog-v3-uses-canonical-source-typed-tables.md),
[per-value provenance](adr/0357-source-encryption-provenance-is-explicit-per-data-value.md),
[localization limits](adr/0394-localization-is-complete-by-default.md).

## One lookup, two values

The existing sourced `translations` map contains only actually available supported languages.
Each language maps directly to this grouping object, without a third sourced wrapper:

```text
translations.value[language] = {
    sourceText: { state, value, containsEncryptedSource, sourceUsesRefs, sourceSetComplete },
    displayText: { state, value, containsEncryptedSource, sourceUsesRefs, sourceSetComplete }
}
```

`sourceText` is the exact winning decoded INI RHS after `=`. It retains edge whitespace, Unicode,
literal backslash-n and format directives. INI line delimiters and the original encoded byte
stream are separate from this logical value. `displayText` is the existing `display_text` result:
literal backslash-n becomes LF, then leading and trailing Unicode whitespace is trimmed. Neither
form evaluates format directives, interpolates values or invents translations.

Both forms come from one original lookup. The existing optional-`@`, ASCII-case-insensitive key
lookup and plain/suffixed winner policy stay unchanged. An exact authored key is retained even
when lookup uses that normalized index key. One key used as both a template and a visible name
has one public row with both forms, independent of registration order. The same key used in the
protected domain retains that domain's separate existing row and ID.

| Original outcome | sourceText | displayText |
| --- | --- | --- |
| Winning RHS | present, exact decoded text | present, existing display result |
| Empty RHS | present empty string | present empty string |
| Padding-only RHS | present original padding | present empty string |
| Missing key in an available language | absent | absent |
| Placeholder key | absent, without locale lookup | absent, without locale lookup |

A missing language Entry has no map member. Missing German text never acquires English fallback.
A placeholder retains its exact original key and reference. Empty present values are not absence.

## Original contributions

Each form has the actual authored key Selector and, after a lookup, the actual language
`LocalizationLookup` Selector. The lookup's selector reference points to that original key use.
A winner adds the actual locale Payload. The source form adds Conversion only when the original
`IniText` receipt records BOM removal, UTF16 transcoding or lossy decoding. Valid plain UTF8
requires no Entry-decoding Conversion. The display form uses the actual display conversion at
the winning lookup. It never gives source text a trimming or line-break-display contribution.

The decoder receipt describes the original Entry, not a claim that this winning slice had
replacement characters. Equal input/coordinate/role Conversion handles may deduplicate across
decoding and display steps. No synthetic stage location or serialized reference creates source
authority. Missing keys contribute actual lookup Selectors without a winning Payload or text
Conversion; placeholders contribute no locale uses. The translations aggregate unions the actual
form inputs. Each nullable marker still follows the shared conservative provenance rules.

## Consumer recipe and counters

A consumer that used the normalized language value reads:

```text
translations.value[language].displayText.value
translations.value[language].displayText.state
```

A format-template consumer such as the canonical GameplayProperty `unitFormat` reads the
corresponding `sourceText.value` and `sourceText.state`. This shared text contract does not itself
add that property's source fields. Consumers inspect the selected form's direct origin metadata;
they never infer it solely from the row or the translations aggregate.

Summary resolved/unresolved counts still describe the old display result once per original
lookup occurrence. A present empty display counts as unresolved without becoming absent.
Placeholders count once per original registration, not per form or language. Repeated uses of
one canonical row retain their original occurrence accounting. Independent legacy library display
results stay unchanged; the catalog emits no legacy language-cell alias or schema-2 adapter.

## Wire size and version

Each language has two independently sourced values rather than one, adding one carrier and a
second serialized text when present. There is still one language map and one registry/ID.
Equal forms remain two explicit JSON values; source text can borrow the original RHS and display
text can borrow unless its existing escape decoding allocates. No measured size, compression or
memory claim follows from this shape. Existing catalog budgets, serialization and refusal rules
remain binding. Only the unpublished catalog schema 3 changes; independent Media and Archive
format versions remain unchanged.

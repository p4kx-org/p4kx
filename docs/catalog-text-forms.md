# Canonical catalog TextForms

Catalog schema 3 stores one canonical text row per existing `(Domain, key)`, retaining the exact
authored key and existing IDs. Public names and public short UI format templates use
`texts.publicNames`; protected descriptions and lore use `texts.gameTexts`. The existing protected
owner/reference/table context protects both forms. The shared TextForms type has no global
`x-gameText` annotation. Encryption origin is an independent axis.

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

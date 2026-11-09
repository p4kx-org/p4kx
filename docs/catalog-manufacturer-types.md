# Canonical Manufacturer definitions

Catalog schema 3 emits only `definitions.manufacturers`, with `availability` and `rows`.
There is no root `manufacturers` array or compatibility view. Kind `manufacturers`, logical row
counts, Filter completeness and publication health retain their meanings. The source family is
`SCItemManufacturer`; a declared family with zero selected rows is `available`, an absent family is
`absent`, and undecidable declarations without positive evidence are `unavailable`. Availability
never replaces the separate read/projection health or Filter completeness.

## Sources and row identity

Select the same exact Record type and existing `libs/foundry/records/scitemmanufacturer/` prefix,
with its ASCII-case/separator matching, Filter and whole-word development exclusions. Keep all
selected manufacturers, including currently unreferenced ones. `PLACEHOLDER` is not excluded.
Rows retain canonical GUID-text order; nominal GUID and original physical `recordIndex` stay
separate. Duplicate identities are diagnosed through the existing physical target index; no row
or GUID is invented and no first-wins deduplication occurs. A nil authored header GUID remains a
present raw fact and receives the existing typed malformed-source identity diagnostic; it blocks
normal publication and stays inspectable in diagnostic output.

Every field below is an immediate sourced-value carrier with required `state`, `value`,
`containsEncryptedSource`, `sourceUsesRefs` and `sourceSetComplete`. Non-present source states
retain a required null value. Present empty strings remain values. The nullable encryption marker
is a direct conservative filter index over actual used inputs, independently of text protection.

| Row field | Meaning/source |
| --- | --- |
| `guid` | Original Record GUID, canonical lower-case UUID text |
| `recordName` | Exact unstripped Record name |
| `className` | Existing type-prefix removal; original name retained when removal does not apply |
| `recordPath` | Original Record path without `.xml` |
| `recordIndex` | Original physical DataForge Record index, independent of output sorting |
| `sourceType` | Actual selected Record type |
| `code` | Raw `Code` String, including an authored empty string; no enum/code/name fallback |
| `localization` | Actual owned `Localization` state, including absent/null/unreadable/invalid |

A present `localization.value` contains sourced `sourceType`, `name` and `description`.
The type is the actual owned struct type. `Name` and `Description` are authored Locale fields;
wrong concrete values are not accepted as literal text. Missing/unreadable parents do not acquire
invented child fields. Unsupported additional declared fields retain exact diagnostic locations.
The reader uses the shared Catalog Record/field state and source-binding implementation.

## Canonical text and protection

`localization.value.name.value` references `texts.publicNames[].id.value`.
`localization.value.description.value` references `texts.gameTexts[].id.value`.
Both the description owner and protected reference/target retain `x-gameText`; the public Name is
not hidden when a key is also used as a protected Description. There are no duplicated inline
`key`/`en`/`de` objects or flat secondary name/description fields.

Canonical rows retain the exact authored key, including a present empty Locale key. That recovers
a fact the collapsed nullable text shape could not distinguish. An empty key has a present typed
reference and key and unresolved translations. An actually present empty or padding-only winning
translation is `present` with an empty normalized value; no source line is `absent`. Placeholders
retain their authored key without invented translations. Missing languages stay absent from the
available language map; no fallback is guessed. Existing decoding, trimming, duplicate-line and
winning-Entry rules apply to every available supported language.

Text summaries count actual authored lookup occurrences, not unique canonical rows. An additional
retained empty key therefore adds unresolved occurrences. Deduplication can reduce bytes while
retaining those counts. The existing text identifier algorithm is unchanged.

A raw header, Code or authored text reference uses its own actual DataForge fields. The className
derivation uses the actual name and type inputs. A localized translation also uses its own winning
language payload/selector/conversion evidence. Merely loaded encrypted locales and untouched
reference targets do not taint unrelated Manufacturer facts. Unknown input evidence remains
unknown; read success or a supplied key cannot manufacture unencrypted proof.

## Inbound links and verification

Items keep the authored `AttachDef.Manufacturer` GUID, and ships keep `Vehicle.manufacturer`.
The existing final-row snapshot, target-kind classification and physical target index resolve them
against actual emitted Manufacturer rows. Filtered, policy-excluded, outside-scope, failed and
ambiguous targets retain their distinct statuses. Reading a link never auto-includes its target or
changes item-variant identity inputs. Shop path brands are not inferred as Manufacturer GUIDs.

Synthetic producer checks cover raw/state/identity fields, text and source-use closure, mixed
plain/encrypted/unknown origin, available languages, filtered joins, aliases, required schema
members and deterministic output. GUI transport fixtures use distinct synthetic description keys
so canonical deduplication preserves the intended measured budget workload and fixed limits.
Independent real-Archive text verification follows these references and cannot skip a family
merely because inline `key` objects disappeared. Existing identified-Build, source, floor and native
acceptance gates remain required; user-deferred installed/GUI runs are separate evidence.

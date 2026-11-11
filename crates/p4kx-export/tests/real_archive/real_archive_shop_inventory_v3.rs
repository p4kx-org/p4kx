// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Build-bound checks of offline shop inventory files and physical targets.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::error::Error;
use std::io::Read as _;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode};
use p4kx_extraction::{Archive, ArchiveWatch, Entry, ObservedEntry, SourceEncryption};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;
use serde_json::value::RawValue;

#[path = "shop_inventory_survey_report.rs"]
mod survey_report;

use survey_report::report;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
type TokenUses<'a> = HashMap<&'a str, &'a Value>;

const FOLDER: &str = "data\\scripts\\shopinventories\\";
const NUMBER_FIELDS: [(&str, &str); 4] = [
    ("buyPrice", "BuyPrice"),
    ("sellPrice", "SellPrice"),
    ("currentInventory", "CurrentInventory"),
    ("maxInventory", "MaxInventory"),
];

#[derive(Debug, Default)]
struct Survey {
    layouts: BTreeMap<&'static str, usize>,
    rentals: BTreeMap<&'static str, usize>,
    number_tokens: usize,
    readable: usize,
    malformed: usize,
    unreadable: usize,
}

#[derive(Default)]
struct Relations {
    resolved: HashSet<u64>,
    placeholder: HashSet<u64>,
    referenced: HashSet<u64>,
    statuses: BTreeMap<String, usize>,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn physical_shop_inventories_and_resolution_targets_match_the_installed_build() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let watch = ArchiveWatch::new(&archive);
    watch.check()?;
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = p4kx_export::build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    verify_build(&archive, &catalog)?;

    let definitions = rows(&catalog, "/definitions/shopInventories/rows")?;
    let relations = rows(&catalog, "/declarations/shopInventoryResolution/rows")?;
    let originals: Vec<_> = archive
        .entries()
        .iter()
        .filter(|entry| inventory(entry.path()))
        .collect();
    assert!(
        !originals.is_empty(),
        "the Build exercises the physical inventory dataset"
    );
    assert_eq!(
        definitions.len(),
        originals.len(),
        "one row per actual scoped Entry"
    );
    let physical = physical_index(definitions)?;
    let relations = targets(relations, &physical)?;
    let uses = token_index(&catalog)?;
    let mut survey = Survey::default();
    for entry in originals {
        watch.check()?;
        let original = archive
            .observed_entry(entry)
            .ok_or("physical archive occurrence")?;
        let row = physical
            .get(&u64::from(original.entry_index()))
            .ok_or("actual inventory row")?;
        verify_file(&archive, original, row, &uses, &mut survey)?;
    }
    watch.check()?;
    report(&archive, definitions.len(), &relations, &survey)
}

fn verify_build(archive: &Archive, catalog: &Value) -> TestResult {
    let build = archive.build().ok_or("complete actual Build identity")?;
    for (field, value) in [
        ("branch", build.branch()),
        ("version", build.version()),
        ("changeNumber", build.change_number()),
        ("buildId", build.build_id()),
    ] {
        assert!(
            !value.trim().is_empty()
                && catalog
                    .pointer(&format!("/source/build/{field}"))
                    .and_then(Value::as_str)
                    == Some(value),
            "the catalog retains the actual Build {field}"
        );
    }
    Ok(())
}

fn inventory(path: &str) -> bool {
    let canonical = path.replace('/', "\\").to_ascii_lowercase();
    canonical
        .strip_prefix(FOLDER)
        .and_then(|tail| tail.strip_suffix(".json"))
        .is_some_and(|stem| !stem.is_empty() && !stem.contains('\\'))
}

fn physical_index(rows: &[Value]) -> TestResult<BTreeMap<u64, &Value>> {
    let mut result = BTreeMap::new();
    for row in rows {
        let index = row["entryIndex"]
            .as_u64()
            .ok_or("physical directory index")?;
        assert!(
            result.insert(index, row).is_none(),
            "one row per original occurrence"
        );
    }
    Ok(result)
}

fn targets(relations: &[Value], physical: &BTreeMap<u64, &Value>) -> TestResult<Relations> {
    let mut result = Relations::default();
    for relation in relations {
        let status = text(relation.get("status").ok_or("resolution status")?, "value")?;
        *result.statuses.entry(status.to_owned()).or_default() += 1;
        for target in rows(relation, "/physicalTargets/value")? {
            let target = target.get("value").ok_or("sourced physical target")?;
            let index = target["entryIndex"]
                .as_u64()
                .ok_or("physical target index")?;
            let original = physical
                .get(&index)
                .ok_or("actual target inventory definition")?;
            assert!(
                text(target, "inputRef")? == text(original, "inputRef")?,
                "each target refers to the same original physical input"
            );
            result.referenced.insert(index);
            match status {
                "resolved" => {
                    result.resolved.insert(index);
                }
                "placeholder" => {
                    result.placeholder.insert(index);
                }
                _ => {}
            }
        }
    }
    Ok(result)
}

fn token_index(catalog: &Value) -> TestResult<TokenUses<'_>> {
    let mut result = HashMap::new();
    for used in rows(catalog, "/provenance/uses")? {
        if used.pointer("/location/namespace").and_then(Value::as_str) == Some("archiveJson") {
            assert!(
                result.insert(text(used, "id")?, used).is_none(),
                "one exact observed JSON token per source-use reference"
            );
        }
    }
    Ok(result)
}

fn bytes(archive: &Archive, entry: &Entry) -> Option<Vec<u8>> {
    let mut reader = archive.open_entry(entry).ok()?.reject_overlong();
    let mut decoded = Vec::new();
    reader.read_to_end(&mut decoded).ok()?;
    Some(decoded)
}

fn verify_file(
    archive: &Archive,
    original: ObservedEntry<'_>,
    row: &Value,
    uses: &TokenUses<'_>,
    survey: &mut Survey,
) -> TestResult {
    let entry = original.entry();
    assert!(
        row.pointer("/sourcePath/value").and_then(Value::as_str) == Some(entry.path()),
        "exact original path at the physical directory position"
    );
    let document = row.get("document").ok_or("source document")?;
    let Some(bytes) = bytes(archive, entry) else {
        survey.unreadable += 1;
        assert_eq!(
            document["state"], "unreadable",
            "actual decoded source failure"
        );
        return Ok(());
    };
    if serde_json::from_slice::<&RawValue>(&bytes).is_err() {
        survey.malformed += 1;
        assert_eq!(document["state"], "invalid", "actual invalid source JSON");
        return Ok(());
    }
    survey.readable += 1;
    assert_eq!(document["state"], "present", "complete actual decoded JSON");
    let expected = match original.stored_encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    assert_eq!(
        document["containsEncryptedSource"].as_bool(),
        expected,
        "the document marker follows its actual payload declaration"
    );
    let body = document.get("value").ok_or("present document")?;
    let arena = Arena {
        bytes: &bytes,
        id: text(row, "inputRef")?,
        values: rows(body, "/source/values")?,
        uses,
    };
    verify_lines(body, &arena, survey)
}

struct Arena<'source, 'catalog> {
    bytes: &'source [u8],
    id: &'catalog str,
    values: &'catalog [Value],
    uses: &'source TokenUses<'catalog>,
}

impl<'catalog> Arena<'_, 'catalog> {
    fn raw<'node>(
        &'node self,
        field: &Value,
        name: &str,
    ) -> TestResult<(&'node [u8], &'catalog Value)> {
        self.check_selector(field, name)?;
        let index: usize = text(field, "value")?.parse()?;
        let node = self
            .values
            .get(index)
            .ok_or("actual source node reference")?;
        let reference = rows(node, "/sourceUsesRefs")?
            .iter()
            .find_map(Value::as_str)
            .ok_or("actual token use")?;
        let used = self
            .uses
            .get(reference)
            .ok_or("original JSON payload use")?;
        assert_eq!(
            used["role"], "payload",
            "the original node supplies payload"
        );
        Ok((
            self.slot(used)?,
            node.get("value").ok_or("source node payload")?,
        ))
    }

    fn check_selector(&self, field: &Value, expected: &str) -> TestResult {
        for reference in rows(field, "/sourceUsesRefs")?
            .iter()
            .filter_map(Value::as_str)
        {
            if let Some(used) = self.uses.get(reference)
                && used["role"] == "selector"
            {
                let name: String = serde_json::from_slice(self.slot(used)?)?;
                if name == expected {
                    return Ok(());
                }
            }
        }
        Err("the original authored field name must select the token".into())
    }

    fn slot(&self, used: &Value) -> TestResult<&[u8]> {
        assert!(
            text(used, "source")? == self.id,
            "the token belongs to the same physical input"
        );
        let [start, length] = rows(used, "/location/indices")? else {
            return Err("exact decoded byte range".into());
        };
        let start: usize = start.as_str().ok_or("original offset")?.parse()?;
        let length: usize = length.as_str().ok_or("original length")?.parse()?;
        let end = start.checked_add(length).ok_or("original range end")?;
        self.bytes
            .get(start..end)
            .ok_or_else(|| "actual decoded bytes".into())
    }
}

fn verify_lines(body: &Value, arena: &Arena<'_, '_>, survey: &mut Survey) -> TestResult {
    for (layout, path) in [
        ("inventory", "/fields/inventory"),
        ("collectionInventory", "/fields/collection/value/inventory"),
    ] {
        let Some(field) = body.pointer(path) else {
            continue;
        };
        if field["state"] != "present" {
            continue;
        }
        *survey.layouts.entry(layout).or_default() += 1;
        for line in rows(field, "/value")? {
            if let Some(line) = line.get("value").filter(|value| !value.is_null()) {
                verify_line(line, arena, survey)?;
            }
        }
    }
    Ok(())
}

fn verify_line(line: &Value, arena: &Arena<'_, '_>, survey: &mut Survey) -> TestResult {
    for (name, original_name) in NUMBER_FIELDS {
        let field = line.get(name).ok_or("typed numeric inventory field")?;
        if field["state"] != "present" {
            continue;
        }
        let (original, node) = arena.raw(field, original_name)?;
        assert!(
            text(node, "token")?.as_bytes() == original,
            "exact original decimal token"
        );
        survey.number_tokens += 1;
    }
    let rental = line.get("rentalOfferings").ok_or("typed rental field")?;
    let shape = match text(rental, "state")? {
        "present" => rental_shape(arena.raw(rental, "RentalOfferings")?.0)?,
        "absent" => "absent",
        "null" => "null",
        _ => "invalid",
    };
    *survey.rentals.entry(shape).or_default() += 1;
    Ok(())
}

fn rental_shape(bytes: &[u8]) -> TestResult<&'static str> {
    Ok(match bytes.first() {
        Some(b'[') => {
            let actual: Vec<&RawValue> = serde_json::from_slice(bytes)?;
            if actual.is_empty() {
                "emptyArray"
            } else {
                "populatedArray"
            }
        }
        Some(b'{') => "object",
        Some(b'"') => "string",
        Some(b't' | b'f') => "boolean",
        Some(b'-' | b'0'..=b'9') => "number",
        _ => return Err("actual present rental source shape".into()),
    })
}

fn rows<'catalog>(value: &'catalog Value, path: &str) -> TestResult<&'catalog [Value]> {
    value
        .pointer(path)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "required catalog row array".into())
}

fn text<'catalog>(value: &'catalog Value, field: &str) -> TestResult<&'catalog str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| "required catalog text field".into())
}

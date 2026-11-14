// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, Filter, RecordTree, TreeBuilder,
    Value as Original, ValueError,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::{Value, json};

use crate::authored_definition_witness::field;
use crate::catalog_v3_consumer as consumer;
use crate::real_archive_auto_spawns::original::Locations;
use crate::real_archive_auto_spawns::provenance::Origins;

const COMPONENT: &str = "LootGenerationComponentParams";
const ENTITY: &str = "EntityClassDefinition";
const COMPONENTS: &str = "Components";
const TABLE: &str = "/configurations/lootGeneration";

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn loot_generation_components_match_filtered_physical_owners() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let (owners, representatives) = census(&forge, &watch)?;
    if let Some(filter) = std::env::var_os("P4KX_LOOT_GENERATION_COMPONENT_FILTER") {
        let value = filter
            .to_str()
            .ok_or("configured loot owner filter is not UTF-8")?;
        verify(&archive, &entry, &forge, &owners, &Filter::parse(value)?)?;
    } else if let Some((_, _, _, path)) = representatives {
        verify(&archive, &entry, &forge, &owners, &Filter::new(path))?;
    }
    watch
        .check()
        .map_err(|_| "Archive changed during verification")?;
    crate::real_archive_verification::print_line(&format!(
        "Loot component census checked {} physical owners",
        owners.len()
    ));
    Ok(())
}

type Owners<'f> = BTreeMap<u32, &'f str>;
type Representative<'f> = (bool, bool, bool, &'f str);

fn census<'f>(
    forge: &'f DataForge<'f>,
    watch: &ArchiveWatch,
) -> TestResult<(Owners<'f>, Option<Representative<'f>>)> {
    let mut owners = Owners::new();
    let Some(probe) = forge.pointer_probe(COMPONENTS, COMPONENT) else {
        return Ok((owners, None));
    };
    let mut trees = TreeBuilder::new();
    let mut representative = None;
    for record in forge.records() {
        watch.poll().map_err(|_| "Archive changed during census")?;
        if record.type_name()? != ENTITY
            || !trees
                .holds(&probe, &record)
                .map_err(|_| "inspect physical loot ownership")?
        {
            continue;
        }
        let path = record.record_path()?;
        owners.insert(record.index(), path);
        if path.contains(['*', '?']) {
            continue;
        }
        let tree = trees
            .build(&record)
            .map_err(|_| "read original loot owner")?;
        for (_, value) in components(&tree)? {
            let Original::Struct(component) = value else {
                continue;
            };
            let Some(Original::Struct(config)) = component.field("lootConfig") else {
                continue;
            };
            let Some(Original::Struct(constraints)) = config.field("lootConstraints") else {
                continue;
            };
            let candidate = (
                matches!(
                    constraints.field("secondaryChoices"),
                    Some(Original::Struct(_))
                ),
                matches!(constraints.field("themes"), Some(Original::Array(themes)) if !themes.is_empty()),
                !path.to_ascii_lowercase().contains("/entities/scitem/"),
                path,
            );
            if representative.is_none_or(|best| candidate > best) {
                representative = Some(candidate);
            }
        }
    }
    Ok((owners, representative))
}

fn components<'tree, 'f>(
    tree: &'tree RecordTree<'f>,
) -> TestResult<impl Iterator<Item = (usize, &'tree Original<'f>)>> {
    let Some(Original::Array(rows)) = tree.field(COMPONENTS) else {
        return Err("original component array".into());
    };
    Ok(rows.iter().enumerate().filter(
        |(_, row)| matches!(row, Original::Struct(value) if value.type_name() == COMPONENT),
    ))
}

fn verify(
    archive: &Archive,
    input: &DataForgeEntry,
    forge: &DataForge<'_>,
    owners: &Owners<'_>,
    filter: &Filter,
) -> TestResult {
    let options = CatalogOptions {
        filter: Some(filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built =
        build_catalog_from(archive, input, &options).map_err(|_| "build filtered loot catalog")?;
    let catalog: Value =
        serde_json::from_slice(built.json()).map_err(|_| "read filtered loot catalog")?;
    crate::real_archive_shop_inventory_v3::verify_build(archive, &catalog)?;
    let origins = Origins::read(&catalog, input.origin())?;
    let table = catalog.pointer(TABLE).ok_or("physical loot table")?;
    let mut rows = array(field(table, "rows")?)?.iter();
    let mut checked = 0;
    let mut trees = TreeBuilder::new();
    for (index, _) in owners.iter().filter(|(_, path)| filter.matches(path)) {
        let owner = forge.record(*index).ok_or("original loot owner")?;
        let tree = trees
            .build(&owner)
            .map_err(|_| "read selected loot owner")?;
        let locations = Locations::read(tree.root());
        for (slot, original) in components(&tree)? {
            let row = rows.next().ok_or("missing physical loot component")?;
            verify_component(&tree, &locations, &origins, slot, original, row)?;
            checked += 1;
        }
    }
    assert!(
        rows.next().is_none(),
        "no extra component rows in filtered scope"
    );
    assert!(
        checked > 0,
        "verification filter selected actual original components"
    );
    crate::real_archive_verification::print_line(&format!(
        "Loot component Catalog rows reconciled: {checked}"
    ));
    Ok(())
}

fn verify_component<'source>(
    tree: &RecordTree<'source>,
    locations: &Locations<'source>,
    origins: &Origins<'_>,
    slot: usize,
    original: &Original<'source>,
    row: &Value,
) -> TestResult {
    let index = tree.record_index();
    assert!(
        consumer::present(field(row, "ownerGuid")?)?.as_str()
            == Some(tree.guid().to_string().as_str()),
        "actual source GUID"
    );
    assert_eq!(
        consumer::present(field(row, "ownerRecordIndex")?)?,
        &json!(index),
        "actual Record ordinal"
    );
    assert_eq!(
        consumer::present(field(row, "componentOrdinal")?)?,
        &json!(slot),
        "actual Components ordinal"
    );
    let ordinal = locations.ordinal(original)?;
    origins.occurrence(field(row, "componentOrdinal")?, index, ordinal, "$value")?;
    origins.occurrence(field(row, "sourceType")?, index, ordinal, "$type")?;
    let Original::Struct(component) = original else {
        return Err("not a component".into());
    };
    let arena = field(row, "authoredFields")?;
    let values = array(field(arena, "values")?)?;
    let observation = Arena {
        origins,
        locations,
        owner: index,
        values,
    };
    let mut cursor = 0;
    observation.members(component, array(field(arena, "roots")?)?, &mut cursor)?;
    assert_eq!(
        cursor,
        values.len(),
        "every emitted arena node is reached exactly once"
    );
    Ok(())
}

struct Arena<'catalog, 'reader, 'source> {
    origins: &'reader Origins<'catalog>,
    locations: &'reader Locations<'source>,
    owner: u32,
    values: &'catalog [Value],
}

impl<'source> Arena<'_, '_, 'source> {
    fn members(
        &self,
        source: &p4kx_extraction::StructValue<'source>,
        fields: &[Value],
        cursor: &mut usize,
    ) -> TestResult {
        assert_eq!(fields.len(), source.fields().len(), "original member count");
        for (original, exported) in source.fields().iter().zip(fields) {
            let name = field(exported, "name")?;
            assert!(
                consumer::present(name)?.as_str() == Some(original.name()),
                "original member order/name"
            );
            let site = self.locations.ordinal(original.value())?;
            self.origins
                .occurrence(name, self.owner, site, "$fieldName")?;
            let declaration = field(exported, "declaration")?;
            for (key, coordinate) in [
                ("dataType", "$dataType"),
                ("isArray", "$isArray"),
                ("target", "$target"),
            ] {
                self.origins
                    .occurrence(field(declaration, key)?, self.owner, site, coordinate)?;
            }
            assert_eq!(
                consumer::present(field(declaration, "isArray")?)?,
                &json!(original.storage().is_array()),
                "original cardinality"
            );
            assert_eq!(
                consumer::present(field(declaration, "target")?)?,
                &json!(original.storage().target()),
                "original target index"
            );
            self.node(original.value(), field(exported, "value")?, cursor)?;
        }
        Ok(())
    }

    fn node(
        &self,
        source: &Original<'source>,
        reference: &Value,
        cursor: &mut usize,
    ) -> TestResult {
        let index = field(reference, "valueIndex")?
            .as_str()
            .ok_or("original value-index text")?
            .parse::<usize>()?;
        assert_eq!(index, *cursor, "original physical preorder");
        *cursor += 1;
        let node = self.values.get(index).ok_or("original arena node")?;
        let cell = field(node, "value")?;
        let site = self.locations.ordinal(source)?;
        self.origins.occurrence(cell, self.owner, site, "$value")?;
        match source {
            Original::Struct(source) => {
                let payload = typed(cell, "struct")?;
                let concrete = field(payload, "sourceType")?;
                assert!(
                    consumer::present(concrete)?.as_str() == Some(source.type_name()),
                    "exact source subtype"
                );
                self.origins
                    .occurrence(concrete, self.owner, site, "$type")?;
                self.members(source, array(field(payload, "fields")?)?, cursor)?;
            }
            Original::Array(source) => {
                let elements = array(field(typed(cell, "array")?, "elements")?)?;
                assert_eq!(elements.len(), source.len(), "each original array slot");
                for (source, element) in source.iter().zip(elements) {
                    self.node(source, element, cursor)?;
                }
            }
            Original::Reference(target) => self.reference(cell, target, site)?,
            Original::Null => assert_eq!(
                cell.get("state"),
                Some(&json!("null")),
                "original explicit null"
            ),
            Original::Error(error) => check_error(node, cell, error)?,
            Original::Single(value) => check_number(cell, &format!("{:08x}", value.to_bits()))?,
            Original::Double(value) => check_number(cell, &format!("{:016x}", value.to_bits()))?,
            _ => {
                consumer::present(cell)?;
            }
        }
        Ok(())
    }

    fn reference(
        &self,
        cell: &Value,
        source: &p4kx_extraction::Reference<'_>,
        site: usize,
    ) -> TestResult {
        let payload = consumer::present(cell)?;
        if field(payload, "type")? != &json!("recordReference") {
            return Ok(());
        }
        let payload = field(payload, "value")?;
        let guid = field(payload, "guid")?;
        assert!(
            consumer::present(guid)?.as_str() == Some(source.guid().to_string().as_str()),
            "stored target GUID"
        );
        self.origins
            .reference_field(guid, self.owner, site, source.record_index(), None)?;
        assert_eq!(
            consumer::present(field(payload, "targetRecordIndex")?)?,
            &json!(source.record_index()),
            "original linked Record ordinal"
        );
        Ok(())
    }
}

fn check_error(node: &Value, cell: &Value, error: &ValueError) -> TestResult {
    assert_eq!(
        cell.get("state"),
        Some(&json!("unreadable")),
        "typed read failure"
    );
    let original = field(node, "originalError")?;
    assert!(
        original.is_object(),
        "source error has its original details"
    );
    if let ValueError::DanglingReference { guid } = error {
        assert!(
            consumer::present(field(original, "guid")?)?.as_str()
                == Some(guid.to_string().as_str()),
            "stored dangling GUID"
        );
    }
    Ok(())
}

fn check_number(cell: &Value, expected: &str) -> TestResult {
    assert_eq!(
        typed(cell, "number")?
            .pointer("/value/rawBits")
            .and_then(Value::as_str),
        Some(expected),
        "exact original float bits"
    );
    Ok(())
}
fn typed<'a>(cell: &'a Value, tag: &str) -> TestResult<&'a Value> {
    let payload = consumer::present(cell)?;
    assert!(
        field(payload, "type")?.as_str() == Some(tag),
        "retained original payload type"
    );
    Ok(payload)
}

fn array(value: &Value) -> TestResult<&[Value]> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| "original ordered array".into())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForgeEntry, Value as Raw};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::resource_fixture;
use crate::resource_quality_schema_fixture::OverrideEntryTarget;

use super::super::super::{Failure, Result};
use super::Written;
use super::cargo_origins::replace_use_location;

const RESOURCE: u128 = 0x7004;
const RESOURCE_TEXT: &str = "00000000-0000-0000-0000-000000007004";
const CONTAINER_TEXT: &str = "00000000-0000-0000-0000-000000007201";
const CARGO: &str = "FixtureCargoWithEmptyName";

struct Site {
    owner: u32,
    ordinal: usize,
}

fn database() -> DataForgeBuilder {
    resource_fixture::database(OverrideEntryTarget::Concrete)
        .structure(
            TestStruct::new(CARGO)
                .parent("SResourceTypeDefaultCargoContainers")
                .field(
                    "",
                    TestType::ReferenceTo("EntityClassDefinition".to_owned()),
                ),
        )
        .record(
            TestRecord::new(
                RESOURCE,
                "ResourceType",
                "ResourceType.FixtureResourceEmptyName",
                "libs/foundry/records/resourcetypedatabase/fixture_empty_name.xml",
            )
            .value(
                "defaultCargoContainers",
                TestValue::Struct(
                    TestInstance::new(CARGO)
                        .value("", TestValue::Reference(resource_fixture::CONTAINER)),
                ),
            ),
        )
}

fn written() -> Result<(Written, Site)> {
    let mut site = None;
    let written = Written::from_database(&database(), |entry| {
        site = Some(original_site(entry)?);
        Ok(())
    })?;
    let site = site.ok_or_else(|| Failure::check("empty-name-original-check"))?;
    Ok((written, site))
}

fn original_site(entry: &DataForgeEntry) -> Result<Site> {
    let forge = entry
        .open()
        .map_err(|error| Failure::source("empty-name-original-open", error))?;
    let record = forge
        .records()
        .find(|record| record.guid().to_string() == RESOURCE_TEXT)
        .ok_or_else(|| Failure::check("empty-name-original-record"))?;
    let tree = record
        .tree()
        .map_err(|error| Failure::source("empty-name-original-tree", error))?;
    let Some(Raw::Struct(cargo)) = tree.field("defaultCargoContainers") else {
        return Err(Failure::check("empty-name-original-structure"));
    };
    let mut empty = cargo
        .fields()
        .iter()
        .enumerate()
        .filter(|(_, field)| field.name().is_empty());
    let (ordinal, field) = empty
        .next()
        .ok_or_else(|| Failure::check("empty-name-original-field"))?;
    assert!(
        empty.next().is_none(),
        "one original empty declaration is present before export"
    );
    assert_eq!(
        field.name(),
        "",
        "the builder and owning format preserve the authored empty name"
    );
    let Raw::Reference(target) = field.value() else {
        return Err(Failure::check("empty-name-original-reference"));
    };
    assert!(
        target.location().is_none(),
        "the original value is a whole Record reference"
    );
    assert_eq!(
        target.guid().to_string(),
        CONTAINER_TEXT,
        "the empty declaration retains its actual original target before export"
    );
    Ok(Site {
        owner: tree.record_index(),
        ordinal,
    })
}

fn empty_slot<'a>(written: &'a Written, site: &Site) -> Result<&'a Value> {
    written
        .catalog
        .pointer("/definitions/resourceTypes/rows")
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter().find(|row| {
                row.pointer("/guid/value").and_then(Value::as_str) == Some(RESOURCE_TEXT)
            })
        })
        .and_then(|row| row.pointer("/defaultCargoContainers/value"))
        .and_then(Value::as_array)
        .and_then(|slots| slots.get(site.ordinal))
        .ok_or_else(|| Failure::check("empty-name-emitted-slot"))
}

#[test]
fn an_original_empty_cargo_name_passes_the_full_producer_source_oracle() -> Result<()> {
    let (written, site) = written()?;
    let slot = empty_slot(&written, &site)?;
    assert_eq!(
        slot.pointer("/value/slot/value").and_then(Value::as_str),
        Some(""),
        "the empty authored name remains present rather than absent or renamed"
    );
    assert_eq!(
        slot.pointer("/value/entityClass/value/guid/value")
            .and_then(Value::as_str),
        Some(CONTAINER_TEXT),
        "the current original reference survives production"
    );
    written.verify(&written.catalog)?;
    Ok(())
}

#[test]
fn an_empty_cargo_field_cannot_use_an_out_of_prefix_source_label() -> Result<()> {
    let (written, site) = written()?;
    written.verify(&written.catalog)?;
    let slot = empty_slot(&written, &site)?;
    let references = slot
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or_else(|| Failure::check("empty-name-original-uses"))?;
    let [reference] = references.as_slice() else {
        return Err(Failure::check("empty-name-single-original-use"));
    };
    let id = reference
        .as_str()
        .ok_or_else(|| Failure::check("empty-name-use-id"))?;
    let mut changed = written.catalog.clone();
    replace_use_location(
        &mut changed,
        id,
        "payload",
        &json!({
            "type":"subresource", "namespace":"dataforgeCargoSlot",
            "indices":[site.owner.to_string(), site.ordinal.to_string()],
            "field":"outsideCargoContainers."
        }),
    )?;
    assert_eq!(
        changed.get("definitions"),
        written.catalog.get("definitions"),
        "only the source label changes while every canonical value remains identical"
    );
    let Err(failure) = written.verify(&changed) else {
        return Err(Failure::check("outside-cargo-label-accepted"));
    };
    assert_eq!(
        failure.stage, "cargo-slot-field",
        "accepting an empty authored name does not admit an unrelated source prefix"
    );
    Ok(())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Compression, write_catalog_from};
use p4kx_extraction::{Archive, ArchiveWatch, DataForgeEntry};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

use crate::resource_fixture;
use crate::resource_quality_schema_fixture::OverrideEntryTarget;

use super::super::carriers::{Carriers, Uses};
use super::super::{Failure, Result};
use super::{Coverage, check};

const ALPHA: &str = "/definitions/resourceTypes/rows/0";
const MEAN: &str = concat!(
    "/properties/value/1/value/payload/value/qualityDistribution/value/",
    "payload/value/value/value/mean"
);

struct Written {
    _folder: tempfile::TempDir,
    archive: Archive,
    entry: DataForgeEntry,
    catalog: Value,
}

impl Written {
    fn new(target: OverrideEntryTarget) -> Result<Self> {
        let folder =
            tempfile::tempdir().map_err(|error| Failure::source("synthetic-folder", error))?;
        let path = folder.path().join("source.p4k");
        ArchiveBuilder::new()
            .entry(TestEntry::stored(
                "Data/Game2.dcb",
                resource_fixture::database(target).build(),
            ))
            .entry(TestEntry::stored(
                "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml",
                b"<LayerBackups/>",
            ))
            .write_to(&path)
            .map_err(|error| Failure::source("synthetic-archive", error))?;
        let archive =
            Archive::open(path).map_err(|error| Failure::source("synthetic-open", error))?;
        let entry = DataForgeEntry::read(&archive)
            .map_err(|error| Failure::source("synthetic-dataforge", error))?;
        let options = CatalogOptions {
            publication_mode: CatalogPublicationMode::DiagnosticPartial,
            ..CatalogOptions::new(UNIX_EPOCH)
        };
        let (_, bytes) = write_catalog_from(&archive, &entry, &options, Compression::None, || {
            Ok(Vec::new())
        })
        .map_err(|error| Failure::source("synthetic-producer", error))?;
        let catalog = serde_json::from_slice(&bytes)
            .map_err(|error| Failure::source("synthetic-producer-json", error))?;
        Ok(Self {
            _folder: folder,
            archive,
            entry,
            catalog,
        })
    }

    fn verify(&self, catalog: &Value) -> Result<Coverage> {
        let forge = self
            .entry
            .open()
            .map_err(|error| Failure::source("synthetic-reader", error))?;
        check(
            &forge,
            self.entry.origin(),
            catalog,
            &ArchiveWatch::new(&self.archive),
        )
    }

    fn alpha(&self) -> Result<&Value> {
        let alpha = self
            .catalog
            .pointer(ALPHA)
            .ok_or_else(|| Failure::check("synthetic-alpha-row"))?;
        if alpha.pointer("/className/value").and_then(Value::as_str) != Some("FixtureResourceAlpha")
        {
            return Err(Failure::check("synthetic-row-order"));
        }
        Ok(alpha)
    }
}

#[test]
fn actual_producer_concrete_override_sources_pass_the_raw_oracle() -> Result<()> {
    positive(OverrideEntryTarget::Concrete)
}

#[test]
fn actual_producer_base_override_sources_pass_the_raw_oracle() -> Result<()> {
    positive(OverrideEntryTarget::Base)
}

fn positive(target: OverrideEntryTarget) -> Result<()> {
    let written = Written::new(target)?;
    expected_wire(&written)?;
    let coverage = written.verify(&written.catalog)?;
    assert_eq!(
        coverage.packaging_slots, 10,
        "all declared packaging slots were compared"
    );
    assert_eq!(
        coverage.null_box_slots, 8,
        "all declared null box slots were compared"
    );
    assert!(
        coverage.volatility_properties >= 2,
        "actual volatility structures were compared"
    );
    assert!(
        coverage.quality_values >= 4,
        "actual owned quality values were compared"
    );
    Ok(())
}

fn expected_wire(written: &Written) -> Result<()> {
    let alpha = written.alpha()?;
    let uses = Uses::new(&written.catalog, written.entry.origin())?;
    let owner = uses.record_index(alpha)?;
    let carriers = Carriers::new(alpha, &uses, owner)?;
    assert_eq!(
        alpha.pointer("/validateDefaultCargoBox/value"),
        Some(&json!(false)),
        "the canonical source Bool is present"
    );
    expected_slots(alpha, &carriers)?;
    let property = alpha
        .pointer("/properties/value/1/value")
        .ok_or_else(|| Failure::check("synthetic-crafting-wire"))?;
    assert_eq!(
        property.pointer("/sourceType/value"),
        Some(&json!("ResourceTypeCraftingData")),
        "a real crafting property is present"
    );
    assert_eq!(
        property.pointer("/payload/value/type"),
        Some(&json!("craftingData")),
        "the closed payload is the actual crafting shape"
    );
    expected_quality(alpha, property, &carriers)
}

fn expected_quality(alpha: &Value, property: &Value, carriers: &Carriers<'_>) -> Result<()> {
    let quality = property
        .pointer("/payload/value/qualityDistribution/value")
        .ok_or_else(|| Failure::check("synthetic-quality-wire"))?;
    assert_eq!(
        quality.pointer("/sourceType/value"),
        Some(&json!("CraftingQualityDistributionNormal")),
        "the concrete quality source survives"
    );
    assert_eq!(
        quality.pointer("/payload/value/type"),
        Some(&json!("inline")),
        "the actual source is an inline Normal distribution"
    );
    assert_eq!(
        quality.pointer("/payload/value/value/value/min/value"),
        Some(&json!(16_777_217)),
        "the exact authored lower bound survives"
    );
    assert_eq!(
        quality.pointer("/payload/value/value/value/max/value"),
        Some(&json!(i32::MAX)),
        "the exact authored upper bound survives"
    );
    let mean = alpha
        .pointer(MEAN)
        .ok_or_else(|| Failure::check("synthetic-mean-wire"))?;
    carriers.require(mean, "properties[1].qualityDistribution.mean")?;
    assert_eq!(
        mean.pointer("/value/type"),
        Some(&json!("f32")),
        "stored numeric width"
    );
    assert_eq!(
        mean.pointer("/value/rawBits"),
        Some(&json!("41180000")),
        "actual 9.5 source bits"
    );
    assert_eq!(
        mean.pointer("/value/finite"),
        Some(&json!(9.5)),
        "actual finite mean"
    );
    Ok(())
}

fn expected_slots(alpha: &Value, carriers: &Carriers<'_>) -> Result<()> {
    let names = [
        "one_eighthSCU",
        "one_quarterSCU",
        "one_halfSCU",
        "oneSCU",
        "twoSCU",
        "fourSCU",
        "eightSCU",
        "sixteenSCU",
        "twentyFourSCU",
        "thirtyTwoSCU",
    ];
    let slots = alpha
        .pointer("/defaultCargoContainers/value")
        .and_then(Value::as_array)
        .ok_or_else(|| Failure::check("synthetic-slot-wire"))?;
    assert_eq!(
        slots.len(),
        names.len(),
        "every declared source slot retains its position"
    );
    for (slot, name) in slots.iter().zip(names) {
        let path = format!("defaultCargoContainers.{name}");
        carriers.require(slot, &path)?;
        assert_eq!(
            slot["state"], "present",
            "the named slot retains its carrier"
        );
        let label = slot
            .pointer("/value/slot")
            .ok_or_else(|| Failure::check("synthetic-slot-label"))?;
        carriers.require(label, &format!("{path}.$fieldName"))?;
        assert_eq!(
            label["state"], "present",
            "the authored slot name is present"
        );
        assert_eq!(label["value"], name, "declared slot order is exact");
        let entity = slot
            .pointer("/value/entityClass")
            .ok_or_else(|| Failure::check("synthetic-slot-reference"))?;
        carriers.require(entity, &path)?;
        if matches!(name, "one_eighthSCU" | "twoSCU") {
            expected_box(entity);
        } else {
            assert_eq!(
                entity["state"], "null",
                "the undeclared target is explicitly null"
            );
            assert_eq!(
                entity.get("value"),
                Some(&Value::Null),
                "null retains its value key"
            );
        }
    }
    Ok(())
}

fn expected_box(entity: &Value) {
    assert_eq!(
        entity["state"], "present",
        "the authored box Reference is present"
    );
    assert_eq!(
        entity.pointer("/value/status"),
        Some(&json!("present")),
        "the synthetic target resolves"
    );
    assert_eq!(
        entity.pointer("/value/guid/value"),
        Some(&json!("00000000-0000-0000-0000-000000007201")),
        "the authored box GUID survives"
    );
    assert_eq!(
        entity.pointer("/value/targetType/value"),
        Some(&json!("EntityClassDefinition")),
        "the box target retains its actual source type"
    );
    assert_eq!(
        entity.pointer("/value/recordPath/value"),
        Some(&json!("libs/foundry/records/entities/fixture/container")),
        "the Reference names the synthetic container source"
    );
}
#[test]
fn actual_producer_mutations_fail_without_replacement_carriers() -> Result<()> {
    for target in [OverrideEntryTarget::Concrete, OverrideEntryTarget::Base] {
        let written = Written::new(target)?;
        expected_wire(&written)?;
        written.verify(&written.catalog)?;
        let mut changed = written.catalog.clone();
        let root = changed
            .pointer_mut(ALPHA)
            .ok_or_else(|| Failure::check("synthetic-row"))?;
        root["extra"] = root["validateDefaultCargoBox"].clone();
        root["validateDefaultCargoBox"]["value"] = json!(true);
        assert!(
            written.verify(&changed).is_err(),
            "a matching additive carrier cannot repair the real canonical Bool"
        );
        let mut changed = written.catalog.clone();
        let path = format!("{ALPHA}{MEAN}/value/finite");
        *changed
            .pointer_mut(&path)
            .ok_or_else(|| Failure::check("synthetic-mean"))? = json!(1.0);
        assert!(
            written.verify(&changed).is_err(),
            "the real nested source mean is checked"
        );
        let mut changed = written.catalog.clone();
        let path = format!("{ALPHA}{MEAN}/containsEncryptedSource");
        *changed
            .pointer_mut(&path)
            .ok_or_else(|| Failure::check("synthetic-mean-origin"))? = json!(true);
        assert!(
            written.verify(&changed).is_err(),
            "the real nested immediate origin is checked"
        );
    }
    Ok(())
}

#[test]
fn original_filename_and_metadata_tag_states_are_checked_independently_of_normalized_paths()
-> Result<()> {
    let written = Written::new(OverrideEntryTarget::Concrete)?;
    written.verify(&written.catalog)?;
    for (field, state, value) in [
        ("fileName", "present", json!("changed-original-suffix.XML")),
        ("metadataTag", "null", Value::Null),
        ("recordName", "null", Value::Null),
    ] {
        let mut changed = written.catalog.clone();
        let carrier = changed
            .pointer_mut(&format!("{ALPHA}/{field}"))
            .and_then(Value::as_object_mut)
            .ok_or_else(|| Failure::check("synthetic-original-header"))?;
        carrier.insert("state".to_owned(), json!(state));
        carrier.insert("value".to_owned(), value);
        assert!(
            written.verify(&changed).is_err(),
            "each exact raw header fact is checked against the original read even when normalized recordPath and GUID remain unchanged"
        );
    }
    Ok(())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, DataForgeOrigin, Field, InputScope,
    RecordTree, SourceEncryption, SourceInput, SourceUses, Value as DataValue,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::{Value, json};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

const SCALARS: &[(&str, &str)] = &[
    ("Health", "health"),
    ("DamageCap", "damageCap"),
    ("IsRepairable", "repairable"),
    ("IsSalvagable", "salvageable"),
    ("SalvageDamageModifier", "salvageDamageModifier"),
    ("MirrorSalvageHit", "mirrorSalvageHit"),
    ("DestroySelfOnDeath", "destroySelfOnDeath"),
    ("DetachFromItemPortOnDeath", "detachFromItemPortOnDeath"),
    ("alwaysPropagateHitToParent", "alwaysPropagateHitToParent"),
    ("ExplosionDelayTime", "explosionDelayTime"),
];

const RESISTANCE: &[(&str, &str)] = &[
    ("Multiplier", "multiplier"),
    ("Threshold", "threshold"),
    ("DamageCap", "damageCap"),
    ("DamageCapLimit", "damageCapLimit"),
];

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn item_durability_matches_original_health_only_shield_and_armour_fields() -> Result {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(SystemTime::now())
    };
    let built = build_catalog_from(&archive, &entry, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let forge = entry.open()?;
    let counts = compare(&forge, entry.origin(), &catalog)?;
    watch.check()?;
    for category in ["health-only", "shield", "armour"] {
        let count = counts.get(category).copied().unwrap_or_default();
        assert!(
            count > 0,
            "a representative {category} item must be compared"
        );
        crate::real_archive_verification::print_line(&format!(
            "durability {category}: {count} original scalar fields checked"
        ));
    }
    Ok(())
}

fn compare(
    forge: &DataForge<'_>,
    origin: &DataForgeOrigin,
    catalog: &Value,
) -> Result<BTreeMap<&'static str, usize>> {
    let mut counts = BTreeMap::new();
    let items = indexed(catalog, "/items", "guid")?;
    let uses = indexed(catalog, "/provenance/uses", "id")?;
    let DataForgeOrigin::Archive(entry) = origin else {
        return Err("the DCB must have an observed archive origin".into());
    };
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Archive(entry), InputScope::Payload)?;
    let source = scope.source_ref().as_str();
    let encrypted = match origin.encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };

    for row in rows(catalog, "/configurations/durability/rows")? {
        let guid = text(row, "/ownerGuid/value")?;
        let item = items
            .get(guid)
            .ok_or("durability owner in the item table")?;
        let Some(category) = category(item) else {
            continue;
        };
        if counts.contains_key(category) {
            continue;
        }
        let owner = u32::try_from(number(row, "/ownerRecordIndex/value")?)?;
        let slot = usize::try_from(number(row, "/componentOrdinal/value")?)?;
        let record = forge.record(owner).ok_or("physical Health owner")?;
        assert!(record.guid().to_string() == guid, "the Record GUID matches");
        let tree = record.tree()?;
        let component = component(&tree, slot)?;
        let DataValue::Struct(health) = component else {
            return Err("the original component slot must contain a struct".into());
        };
        assert_eq!(
            health.type_name(),
            "SHealthComponentParams",
            "the physical slot holds Health"
        );
        if !matches!(health.field("Health"), Some(DataValue::Single(_))) {
            continue;
        }
        let trace = Trace {
            ordinals: ordinals(tree.fields()),
            uses: &uses,
            owner: owner.to_string(),
            source,
            encrypted,
        };
        trace.check(&row["componentOrdinal"], component, "conversion")?;
        counts.insert(category, trace.health(row, health)?);
        if counts.len() == 3 {
            break;
        }
    }
    Ok(counts)
}

fn category(item: &Value) -> Option<&'static str> {
    if item["type"] == "Armor" {
        Some("armour")
    } else if item.pointer("/stats/shield").is_some_and(Value::is_object) {
        Some("shield")
    } else if item["stats"].is_null() {
        Some("health-only")
    } else {
        None
    }
}

fn component<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
    slot: usize,
) -> Result<&'tree DataValue<'source>> {
    let Some(DataValue::Array(components)) = tree.field("Components") else {
        return Err("the original Record must declare Components".into());
    };
    components
        .get(slot)
        .ok_or_else(|| "the original component slot".into())
}

fn ordinals<'source>(fields: &[Field<'source>]) -> HashMap<*const DataValue<'source>, usize> {
    let mut pending: Vec<_> = fields.iter().rev().map(Field::value).collect();
    let mut output = HashMap::new();
    while let Some(value) = pending.pop() {
        output.insert(std::ptr::from_ref(value), output.len());
        match value {
            DataValue::Struct(node) => {
                pending.extend(node.fields().iter().rev().map(Field::value));
            }
            DataValue::Array(values) => pending.extend(values.iter().rev()),
            _ => {}
        }
    }
    output
}

struct Trace<'read, 'source> {
    ordinals: HashMap<*const DataValue<'source>, usize>,
    uses: &'read HashMap<&'read str, &'read Value>,
    owner: String,
    source: &'read str,
    encrypted: Option<bool>,
}

impl<'source> Trace<'_, 'source> {
    fn health(&self, row: &Value, health: &p4kx_extraction::StructValue<'source>) -> Result<usize> {
        let mut count = self.fields(row, health, SCALARS)?;
        if let Some(DataValue::Struct(resistance)) = health.field("DamageResistances")
            && let Some(DataValue::Struct(physical)) = resistance.field("PhysicalResistance")
        {
            let exported = row
                .pointer("/damageResistances/value/physical/value")
                .ok_or("the original physical resistance entry")?;
            count += self.fields(exported, physical, RESISTANCE)?;
        }
        Ok(count)
    }

    fn fields(
        &self,
        row: &Value,
        source: &p4kx_extraction::StructValue<'source>,
        fields: &[(&str, &str)],
    ) -> Result<usize> {
        let mut count = 0;
        for &(name, key) in fields {
            let Some(raw) = source.field(name) else {
                continue;
            };
            let cell = row.get(key).ok_or("the mapped original Health field")?;
            let (state, role) = match raw {
                DataValue::Single(value) => {
                    assert!(
                        u32::from_str_radix(text(cell, "/value/rawBits")?, 16)? == value.to_bits(),
                        "{name} preserves the Single bits"
                    );
                    ("present", "payload")
                }
                DataValue::Bool(value) => {
                    assert!(cell["value"] == *value, "{name} preserves the boolean");
                    ("present", "payload")
                }
                DataValue::Null => ("null", "selector"),
                _ => return Err(format!("{name} has an unexpected raw value kind").into()),
            };
            assert_eq!(cell["state"], state, "{name} has the original field state");
            self.check(cell, raw, role)?;
            count += 1;
        }
        Ok(count)
    }

    fn check(&self, cell: &Value, raw: &DataValue<'source>, role: &str) -> Result {
        let ordinal = self
            .ordinals
            .get(&std::ptr::from_ref(raw))
            .ok_or("original value ordinal")?;
        let expected = json!([self.owner, ordinal.to_string()]);
        let references = rows(cell, "/sourceUsesRefs")?;
        assert_eq!(
            cell["containsEncryptedSource"],
            json!(self.encrypted),
            "the marker describes the DCB input"
        );
        assert_eq!(
            cell["sourceSetComplete"], true,
            "the exact source set is complete"
        );
        let mut exact_value = false;
        for id in references {
            let id = id.as_str().ok_or("source use identifier")?;
            let usage = self.uses.get(id).ok_or("resolved source use")?;
            exact_value |= usage["source"] == self.source
                && usage["role"] == role
                && usage.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeValue")
                && usage.pointer("/location/indices") == Some(&expected)
                && usage.pointer("/location/field").and_then(Value::as_str) == Some("$value");
        }
        assert!(
            exact_value,
            "the source use must address the actual DCB Record and exact original field"
        );
        Ok(())
    }
}

fn rows<'catalog>(value: &'catalog Value, path: &str) -> Result<&'catalog [Value]> {
    value
        .pointer(path)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("missing array at {path}").into())
}

fn indexed<'catalog>(
    value: &'catalog Value,
    path: &str,
    key: &str,
) -> Result<HashMap<&'catalog str, &'catalog Value>> {
    rows(value, path)?
        .iter()
        .map(|row| {
            let id = row[key].as_str().ok_or("row identifier")?;
            Ok((id, row))
        })
        .collect()
}

fn text<'row>(value: &'row Value, path: &str) -> Result<&'row str> {
    value
        .pointer(path)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing text at {path}").into())
}

fn number(value: &Value, path: &str) -> Result<u64> {
    value
        .pointer(path)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("missing integer at {path}").into())
}

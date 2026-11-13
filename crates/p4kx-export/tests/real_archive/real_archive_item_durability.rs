// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap, HashSet};
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, DataForgeOrigin, Field, InputScope,
    RecordTree, SourceEncryption, SourceInput, SourceLocation, SourceScope, SourceUseRole,
    SourceUses, Value as DataValue,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::{Value, json};

#[path = "real_archive_durability_sample.rs"]
mod sample;

#[path = "real_archive_item_durability_values.rs"]
mod values;

pub(super) use self::values::{indexed, number, rows, text};

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
    let shield_owners = rows(catalog, "/configurations/shieldGenerators/rows")?
        .iter()
        .map(|row| {
            Ok((
                u32::try_from(number(row, "/ownerRecordIndex/value")?)?,
                text(row, "/ownerGuid/value")?,
            ))
        })
        .collect::<Result<HashSet<_>>>()?;
    let DataForgeOrigin::Archive(entry) = origin else {
        return Err("the DCB must have an observed archive origin".into());
    };
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Archive(entry), InputScope::Payload)?;
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
        let owner = u32::try_from(number(row, "/ownerRecordIndex/value")?)?;
        let Some(category) = category(item, shield_owners.contains(&(owner, guid))) else {
            continue;
        };
        if counts.contains_key(category) {
            continue;
        }
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
        let trace = Trace::of(&tree, &uses, &scope, encrypted);
        trace.check(&row["componentOrdinal"], component, "conversion")?;
        counts.insert(category, trace.health(row, health)?);
        if counts.len() == 3 {
            break;
        }
    }
    Ok(counts)
}

fn category(item: &Value, shield_generator: bool) -> Option<&'static str> {
    if item["type"] == "Armor" {
        Some("armour")
    } else if shield_generator {
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

pub(super) struct Trace<'read, 'source> {
    ordinals: HashMap<*const DataValue<'source>, usize>,
    uses: &'read HashMap<&'read str, &'read Value>,
    owner: u32,
    scope: &'read SourceScope,
    encrypted: Option<bool>,
}

impl<'read, 'source> Trace<'read, 'source> {
    pub(super) fn of(
        tree: &RecordTree<'source>,
        uses: &'read HashMap<&'read str, &'read Value>,
        scope: &'read SourceScope,
        encrypted: Option<bool>,
    ) -> Self {
        Self {
            ordinals: ordinals(tree.fields()),
            uses,
            owner: tree.record_index(),
            scope,
            encrypted,
        }
    }

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

    pub(super) fn check(&self, cell: &Value, raw: &DataValue<'source>, role: &str) -> Result {
        assert!(
            cell.get("sourceSetComplete") == Some(&Value::Bool(true)),
            "the exact source set is complete"
        );
        self.part(cell, raw, "$value", role)
    }

    pub(super) fn ordinal(&self, raw: &DataValue<'source>) -> Result<usize> {
        self.ordinals
            .get(&std::ptr::from_ref(raw))
            .copied()
            .ok_or_else(|| "original value ordinal".into())
    }

    pub(super) fn part(
        &self,
        cell: &Value,
        raw: &DataValue<'source>,
        field: &str,
        role: &str,
    ) -> Result {
        let ordinal = self.ordinal(raw)?;
        self.location(
            cell,
            "dataforgeValue",
            &json!([self.owner.to_string(), ordinal.to_string()]),
            field,
            role,
        )
    }

    pub(super) fn header(&self, cell: &Value, field: &str, role: &str) -> Result {
        self.header_for(cell, self.owner, field, role)
    }

    pub(super) fn header_for(&self, cell: &Value, owner: u32, field: &str, role: &str) -> Result {
        self.location(
            cell,
            "dataforgeRecordHeader",
            &json!([owner.to_string()]),
            field,
            role,
        )
    }

    pub(super) fn struct_name(&self, cell: &Value, index: u16) -> Result {
        self.location(
            cell,
            "dataforgeStruct",
            &json!([index.to_string()]),
            "name",
            "selector",
        )
    }

    pub(super) fn attempted_value(&self, cell: &Value, raw: &DataValue<'source>) -> Result {
        let ordinal = self.ordinal(raw)?;
        self.attempted(
            cell,
            "dataforgeValue",
            vec![u64::from(self.owner), u64::try_from(ordinal)?],
            "$value",
        )
    }

    pub(super) fn attempted_struct_name(&self, cell: &Value, index: u16) -> Result {
        self.attempted(cell, "dataforgeStruct", vec![u64::from(index)], "name")
    }

    fn attempted(&self, cell: &Value, namespace: &str, indices: Vec<u64>, field: &str) -> Result {
        assert!(
            cell.get("sourceSetComplete") == Some(&Value::Bool(false)),
            "the unreadable source set is incomplete"
        );
        self.check_encryption(cell);
        let observed = self
            .scope
            .at(
                SourceLocation::Subresource {
                    namespace: namespace.to_owned(),
                    indices,
                    field: field.to_owned(),
                },
                SourceUseRole::Attempted,
            )
            .map_err(|_| "observe exact attempted source")?;
        let id = observed.reference().as_str();
        assert!(
            self.uses.contains_key(id),
            "the original attempt appears in the source inventory"
        );
        let contributed = rows(cell, "/sourceUsesRefs")?
            .iter()
            .any(|reference| reference.as_str() == Some(id));
        assert!(!contributed, "the failed attempt stays separate");
        Ok(())
    }

    fn check_encryption(&self, cell: &Value) {
        let marker = match self.encrypted {
            Some(false) if cell.get("sourceSetComplete") == Some(&Value::Bool(false)) => None,
            actual => actual,
        };
        assert!(
            cell.get("containsEncryptedSource") == Some(&json!(marker)),
            "the marker describes the DCB input"
        );
    }

    fn location(
        &self,
        cell: &Value,
        namespace: &str,
        indices: &Value,
        field: &str,
        role: &str,
    ) -> Result {
        self.check_encryption(cell);
        let mut exact_use = false;
        for id in rows(cell, "/sourceUsesRefs")? {
            let id = id.as_str().ok_or("source use identifier")?;
            let usage = self.uses.get(id).ok_or("resolved source use")?;
            exact_use |= usage.get("source").and_then(Value::as_str)
                == Some(self.scope.source_ref().as_str())
                && usage.get("role").and_then(Value::as_str) == Some(role)
                && usage.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
                && usage.pointer("/location/indices") == Some(indices)
                && usage.pointer("/location/field").and_then(Value::as_str) == Some(field);
        }
        assert!(
            exact_use,
            "the source use addresses the actual DCB occurrence"
        );
        Ok(())
    }
}

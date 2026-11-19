// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Kind, is_kind_leftover};
use p4kx_extraction::{ArchiveWatch, DataForge, RecordTree, StructValue, Value as Original};

use super::TestResult;

pub(super) const MINING: &str = "MineableParams";
pub(super) const RADAR: &str = "SSCSignatureSystemParams";
const RADAR_GLOBAL: &str = "RadarSystemGlobalParams";
const SCAN_KEY: &str = "@hud_scanning_info_rs_signature";

pub(super) struct SignatureLayout<'source> {
    pub(super) tree: RecordTree<'source>,
    pub(super) slot: usize,
    pub(super) width: usize,
}

impl<'source> SignatureLayout<'source> {
    pub(super) fn selector(&self) -> TestResult<&Original<'source>> {
        let Original::Struct(entry) = types(&self.tree)?
            .get(self.slot)
            .ok_or("original selected radar signature entry")?
        else {
            return Err("original selected radar signature entry is a struct".into());
        };
        entry
            .field("displayName")
            .ok_or_else(|| "original scan Locale".into())
    }
}

pub(super) fn layout<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
) -> TestResult<SignatureLayout<'source>> {
    let mut global = None;
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during radar-global discovery")?;
        if record.type_name().ok() == Some(RADAR_GLOBAL) {
            if global.is_some() {
                return Err(
                    "installed witness requires one unambiguous radar-global Record".into(),
                );
            }
            global = Some(
                record
                    .tree()
                    .map_err(|_| "read original radar-global Record")?,
            );
        }
    }
    let tree = global.ok_or("one original radar-global Record")?;
    let entries = types(&tree)?;
    let mut matches = entries.iter().enumerate().filter_map(|(index, entry)| {
        let Original::Struct(entry) = entry else {
            return None;
        };
        matches!(entry.field("displayName"), Some(Original::Locale(key)) if *key == SCAN_KEY)
            .then_some(index)
    });
    let slot = matches.next().ok_or("one raw scan signature selector")?;
    if matches.next().is_some() {
        return Err("raw scan signature selector is ambiguous".into());
    }
    let width = entries.len();
    Ok(SignatureLayout { tree, slot, width })
}

fn types<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
) -> TestResult<&'tree [Original<'source>]> {
    let Some(Original::Struct(system)) = tree.field("signatureSystemParams") else {
        return Err("the original signature system declaration".into());
    };
    let Some(Original::Array(types)) = system.field("signatureTypeParams") else {
        return Err("the original ordered signature types".into());
    };
    Ok(types)
}

pub(super) fn select_owner<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
    layout: &SignatureLayout<'_>,
) -> TestResult<RecordTree<'source>> {
    let rule = Kind::MineableEntities
        .rule()
        .ok_or("the Mineable Entity source rule")?;
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during Mineable Entity discovery")?;
        if record.type_name().ok() != Some(rule.type_name) {
            continue;
        }
        let Ok(path) = record.record_path() else {
            continue;
        };
        if !rule.matches_path(path) {
            continue;
        }
        let Ok(name) = record.name() else { continue };
        if is_kind_leftover(Kind::MineableEntities, path, name) {
            continue;
        }
        let Ok(tree) = record.tree() else { continue };
        if has_complete_signatures_and_links(&tree, layout) {
            return Ok(tree);
        }
    }
    Err(
        "Archive has no covered Mineable Entity with resolved links and a finite scan signature"
            .into(),
    )
}

fn has_complete_signatures_and_links(tree: &RecordTree<'_>, layout: &SignatureLayout<'_>) -> bool {
    let Some(Original::Array(slots)) = tree.field("Components") else {
        return false;
    };
    let mut mining = components(slots, MINING).peekable();
    let mut radar = components(slots, RADAR).peekable();
    mining.peek().is_some()
        && radar.peek().is_some()
        && slots
            .iter()
            .all(|slot| matches!(slot, Original::Struct(_) | Original::Null))
        && mining.all(|(_, _, item)| {
            ["composition", "globalParams"]
                .iter()
                .all(|name| resolved_link(item, name))
        })
        && radar.all(|(_, _, item)| qualified_radar(item, layout))
}

fn qualified_radar(component: &StructValue<'_>, layout: &SignatureLayout<'_>) -> bool {
    let Some(Original::Struct(properties)) = component.field("radarProperties") else {
        return false;
    };
    let Some(Original::Struct(base)) = properties.field("baseSignatureParams") else {
        return false;
    };
    let Some(Original::Array(signatures)) = base.field("signatures") else {
        return false;
    };
    resolved_link(properties, "contactType")
        && signatures.len() == layout.width
        && matches!(signatures.get(layout.slot), Some(Original::Single(value)) if value.is_finite())
        && signatures.iter().all(|value| matches!(value, Original::Single(_)))
        && signatures.iter().any(|value| {
            matches!(value, Original::Single(number) if number.is_finite() && *number != 0.0)
        })
}

fn resolved_link(source: &StructValue<'_>, name: &str) -> bool {
    matches!(source.field(name), Some(Original::Reference(target)) if target.location().is_none())
}

pub(super) fn components<'tree, 'source>(
    slots: &'tree [Original<'source>],
    kind: &'static str,
) -> impl Iterator<Item = (usize, &'tree Original<'source>, &'tree StructValue<'source>)> {
    slots
        .iter()
        .enumerate()
        .filter_map(move |(index, raw)| match raw {
            Original::Struct(value) if value.type_name() == kind => Some((index, raw, value)),
            _ => None,
        })
}

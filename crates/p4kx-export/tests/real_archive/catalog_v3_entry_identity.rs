// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_export::Kind;
use serde_json::Value;

use super::{TestResult, present};

pub(super) const fn has_physical_entry_identity(kind: Kind) -> bool {
    uses_archive_key(kind)
        || matches!(
            kind,
            Kind::FlareLibraries | Kind::RadarAudio | Kind::SubsumptionEvents
        )
}

const fn uses_archive_key(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::ParticleLibraries
            | Kind::PlanetResources
            | Kind::Creatures
            | Kind::ClimateAssets
            | Kind::GeologyAssets
            | Kind::Ecosystems
            | Kind::GeologySets
            | Kind::GeologyTextureSets
            | Kind::Soils
            | Kind::SoilTextureSets
            | Kind::ShadingClasses
    )
}

pub(super) fn check(catalog: &Value, kind: Kind, rows: &[Value]) -> TestResult<()> {
    if rows.is_empty() {
        return Ok(());
    }
    let mut identities = HashMap::with_capacity(rows.len());
    for row in rows {
        let identity = row.get("identity").ok_or("physical Entry identity")?;
        let id = identity["inputRef"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or("physical Entry source reference")?;
        let index = u32::try_from(
            identity["entryIndex"]
                .as_u64()
                .ok_or("physical Entry index")?,
        )?;
        let path = entry_path(identity, "sourcePath")?;
        let normalized = entry_path(identity, "normalizedPath")?;
        let expected = if uses_archive_key(kind) {
            path.trim_start_matches(['/', '\\'])
                .replace('/', "\\")
                .to_ascii_lowercase()
        } else {
            path.replace('\\', "/")
        };
        if normalized != expected {
            return Err(format!(
                "{kind}: normalized Entry path differs from the physical source path"
            )
            .into());
        }
        if identities.insert(id, (index, path)).is_some() {
            return Err("duplicate physical Entry identity".into());
        }
    }
    let sources = catalog
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("physical source inventory")?;
    for source in sources {
        let Some((index, path)) = source["id"].as_str().and_then(|id| identities.remove(id)) else {
            continue;
        };
        let witnessed = match source["kind"].as_str() {
            Some("archiveEntry") if entry_path(source, "path")? == path => {
                source["entryIndex"].as_u64()
            }
            Some("archiveEntryProof") => source["entryIndex"]
                .as_str()
                .and_then(|value| value.parse().ok()),
            _ => None,
        };
        if witnessed != Some(u64::from(index)) {
            return Err("Entry position disagrees with its bound source".into());
        }
    }
    if !identities.is_empty() {
        return Err("physical Entry identity has no source in the catalog inventory".into());
    }
    Ok(())
}

fn entry_path<'row>(identity: &'row Value, name: &str) -> TestResult<&'row str> {
    let cell = identity.get(name).ok_or("sourced Entry path")?;
    if cell["sourceUsesRefs"].as_array().is_none_or(Vec::is_empty) {
        return Err("physical Entry path has no source uses".into());
    }
    present(cell)?
        .as_str()
        .filter(|path| !path.is_empty())
        .ok_or_else(|| "present Entry path".into())
}

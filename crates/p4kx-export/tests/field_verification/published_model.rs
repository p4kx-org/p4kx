// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::{Deserialize, Serialize};

use super::compare::Rule;
use super::inventory::FieldKey;
use super::path::Path;
use super::units::Unit;

pub(crate) const METADATA: &str = ".p4kx-scunpacked.json";
pub(crate) const MAX_METADATA_BYTES: u64 = 4 << 20;
pub(crate) const MAX_JSON_BYTES: u64 = 64 << 20;
pub(crate) const MAX_DATASET_BYTES: u64 = 256 << 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum Availability {
    Known { value: String },
    Unknown { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Build {
    pub(crate) game_version: String,
    pub(crate) channel: String,
    pub(crate) change_number: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Provenance {
    pub(crate) repository: String,
    pub(crate) data_revision: String,
    pub(crate) published_build: Build,
    pub(crate) generator_revision: Availability,
    pub(crate) dcb_sha256: Availability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Family {
    Items,
    Ships,
    Commodities,
    MineableElements,
    Placements,
}

impl Family {
    pub(crate) const fn group(self) -> &'static str {
        match self {
            Self::Items => "items",
            Self::Ships => "ships",
            Self::Commodities => "commodities",
            Self::MineableElements => "mineableElements",
            Self::Placements => "starmapObjects",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct File {
    pub(crate) path: String,
    pub(crate) bytes: u64,
    pub(crate) sha256: String,
    pub(crate) family: Family,
    pub(crate) rows: Path,
    pub(crate) identity: Path,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Exclusion {
    pub(crate) guid: String,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ResourceJoin {
    pub(crate) reference: Path,
    pub(crate) catalog: Path,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Field {
    pub(crate) source: FieldKey,
    pub(crate) family: Family,
    pub(crate) reference: Path,
    pub(crate) catalog: Path,
    pub(crate) reference_unit: Unit,
    pub(crate) catalog_unit: Unit,
    pub(crate) rule: Rule,
    pub(crate) resource_join: Option<ResourceJoin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Snapshot {
    pub(crate) schema_version: u32,
    pub(crate) provenance: Provenance,
    pub(crate) selection: Vec<String>,
    pub(crate) exclusions: Vec<Exclusion>,
    pub(crate) files: Vec<File>,
    pub(crate) fields: Vec<Field>,
}

pub(crate) struct Publication {
    pub(crate) revision: &'static str,
    pub(crate) game_version: &'static str,
    pub(crate) channel: &'static str,
    pub(crate) change_number: &'static str,
}

pub(crate) const PUBLICATIONS: &[Publication] = &[Publication {
    revision: "1340c8303709904165e2da0fa7060bce0624260b",
    game_version: "4.10.2",
    channel: "LIVE",
    change_number: "12881860",
}];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuildGate {
    SamePublishedBuild,
    Mismatched,
    UnverifiedPublication,
}

pub(crate) fn gate(
    provenance: &Provenance,
    actual: &Build,
    publications: &[Publication],
) -> BuildGate {
    let known = publications
        .iter()
        .find(|publication| publication.revision == provenance.data_revision);
    let Some(known) = known else {
        return BuildGate::UnverifiedPublication;
    };
    if provenance.repository != "StarCitizenWiki/scunpacked-data" {
        return BuildGate::UnverifiedPublication;
    }
    let stated = &provenance.published_build;
    if stated.game_version != known.game_version
        || stated.channel != known.channel
        || stated.change_number != known.change_number
    {
        return BuildGate::UnverifiedPublication;
    }
    if stated == actual {
        BuildGate::SamePublishedBuild
    } else {
        BuildGate::Mismatched
    }
}

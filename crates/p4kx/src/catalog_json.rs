// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_export::{
    ArchiveKeyState, CatalogSummary, CraftingWarning, DECOMPRESSED_LIMIT, ExportHealth,
    KindSummary, LoadoutSummary, PaintSummary, Playability, PlayabilityCounts, SCHEMA_VERSION,
    SizeWarning, StarmapSummary, UPLOAD_LIMIT, VariantCounts,
};
use p4kx_extraction::{Archive, SourceInventory};
use serde::Serialize;

use crate::catalog_failure_json::FailureJson;
use crate::catalog_json_shops::ShopsJson;
use crate::catalog_json_slugs::StoreSlugsJson;
use crate::json;

#[derive(Serialize)]
pub(super) struct SummaryJson<'a> {
    archive: String,
    out: String,
    schema_version: u32,
    archive_key: ArchiveKeyState,
    encrypted_entries_unread: u64,
    export_health: &'a ExportHealth,
    provenance: &'a SourceInventory,
    size: u64,
    compressed_size: Option<u64>,
    upload_limit: u64,
    decompressed_limit: u64,
    size_warning: Option<&'static str>,
    decompressed_warning: Option<&'static str>,
    kinds: Vec<KindJson>,
    skipped: usize,
    failed: Vec<FailureJson<'a>>,
    warnings: Vec<WarningJson>,
    item_classes: ItemClassJson,
    paints: PaintsJson,
    loadouts: LoadoutsJson,
    starmap: StarmapJson,
    shops: ShopsJson<'a>,
    store_slugs: StoreSlugsJson<'a>,
    playability: PlayabilityJson,
    variants: VariantsJson,
    quantizations: QuantizationsJson,
    texts: Vec<TextJson>,
    unresolved: usize,
}

impl From<PaintSummary> for PaintsJson {
    fn from(summary: PaintSummary) -> Self {
        Self {
            paints: summary.paints,
            assigned: summary.assigned,
            unassigned: summary.unassigned,
            ships: summary.ships,
            unreadable_vehicles: summary.unreadable_vehicles,
        }
    }
}

#[derive(Serialize)]
struct LoadoutsJson {
    ships: usize,
    items: usize,
    ports: usize,
    attached: usize,
    docked_vehicles: usize,
    unresolved: usize,
    outside: usize,
    with_issues: usize,
}

impl From<LoadoutSummary> for LoadoutsJson {
    fn from(summary: LoadoutSummary) -> Self {
        Self {
            ships: summary.ships,
            items: summary.items,
            ports: summary.ports,
            attached: summary.attached,
            docked_vehicles: summary.docked_vehicles,
            unresolved: summary.unresolved,
            outside: summary.outside,
            with_issues: summary.with_issues,
        }
    }
}

#[derive(Serialize)]
struct PaintsJson {
    paints: usize,
    assigned: usize,
    unassigned: usize,
    ships: usize,
    unreadable_vehicles: usize,
}

#[derive(Serialize)]
struct StarmapJson {
    objects: usize,
    placed: usize,
    unplaced: usize,
    ambiguous: usize,
    systems: usize,
    unreadable_containers: usize,
    missing_containers: usize,
    malformed_children: usize,
}

impl From<StarmapSummary> for StarmapJson {
    fn from(summary: StarmapSummary) -> Self {
        Self {
            objects: summary.objects,
            placed: summary.placed,
            unplaced: summary.unplaced,
            ambiguous: summary.ambiguous,
            systems: summary.systems,
            unreadable_containers: summary.unreadable_containers,
            missing_containers: summary.missing_containers,
            malformed_children: summary.malformed_children,
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WarningJson {
    UnreadableRewardRecords { count: usize },
    UnknownModifierRanges { count: usize, details: Vec<String> },
    AmbiguousDismantle { count: usize },
    UnreadableVehicleFiles { count: usize, details: Vec<String> },
    UnplacedStarmapContainers { count: usize },
    UnreadableShopRegistry,
    MissingShopRegistry,
    UnreadableShopContainers { count: usize },
    UnreadableShopInventories { count: usize, details: Vec<String> },
    MissingShopInventories { count: usize },
    NoShopComponents,
    Other { message: String },
}

impl From<&CraftingWarning> for WarningJson {
    fn from(warning: &CraftingWarning) -> Self {
        match warning {
            CraftingWarning::UnreadableRewardRecords(count) => {
                Self::UnreadableRewardRecords { count: *count }
            }
            CraftingWarning::UnknownModifierRanges { count, details } => {
                Self::UnknownModifierRanges {
                    count: *count,
                    details: details.clone(),
                }
            }
            CraftingWarning::AmbiguousDismantle(count) => {
                Self::AmbiguousDismantle { count: *count }
            }
            CraftingWarning::UnreadableVehicleFiles { count, details } => {
                Self::UnreadableVehicleFiles {
                    count: *count,
                    details: details.clone(),
                }
            }
            CraftingWarning::UnplacedStarmapContainers(count) => {
                Self::UnplacedStarmapContainers { count: *count }
            }
            CraftingWarning::UnreadableShopRegistry => Self::UnreadableShopRegistry,
            CraftingWarning::MissingShopRegistry => Self::MissingShopRegistry,
            CraftingWarning::UnreadableShopContainers(count) => {
                Self::UnreadableShopContainers { count: *count }
            }
            CraftingWarning::UnreadableShopInventories { count, details } => {
                Self::UnreadableShopInventories {
                    count: *count,
                    details: details.clone(),
                }
            }
            CraftingWarning::MissingShopInventories(count) => {
                Self::MissingShopInventories { count: *count }
            }
            CraftingWarning::NoShopComponents => Self::NoShopComponents,
            other => Self::Other {
                message: other.to_string(),
            },
        }
    }
}

#[derive(Serialize)]
struct ItemClassJson {
    classified: usize,
    other: usize,
    missing: usize,
}

#[derive(Serialize)]
struct PlayabilityJson {
    items: PlayabilityCountsJson,
    ships: PlayabilityCountsJson,
}

#[derive(Serialize)]
struct VariantsJson {
    items: VariantCountsJson,
    ships: VariantCountsJson,
}

#[derive(Serialize)]
struct QuantizationsJson {
    unused: usize,
    unresolved_links: usize,
}

#[derive(Serialize)]
struct VariantCountsJson {
    groups: usize,
    variants: usize,
}

impl From<VariantCounts> for VariantCountsJson {
    fn from(counts: VariantCounts) -> Self {
        Self {
            groups: counts.groups,
            variants: counts.variants,
        }
    }
}

#[derive(Serialize)]
struct PlayabilityCountsJson {
    player: usize,
    ai: usize,
    mission: usize,
    special: usize,
    unclassified: usize,
}

impl From<PlayabilityCounts> for PlayabilityCountsJson {
    fn from(counts: PlayabilityCounts) -> Self {
        Self {
            player: counts.of(Playability::Player),
            ai: counts.of(Playability::Ai),
            mission: counts.of(Playability::Mission),
            special: counts.of(Playability::Special),
            unclassified: counts.unclassified(),
        }
    }
}

#[derive(Serialize)]
struct TextJson {
    language: &'static str,
    available: bool,
    resolved: usize,
    unresolved: usize,
    placeholder: usize,
}

#[derive(Serialize)]
struct KindJson {
    kind: &'static str,
    count: usize,
    skipped: usize,
    complete: bool,
}

impl From<&KindSummary> for KindJson {
    fn from(kind: &KindSummary) -> Self {
        Self {
            kind: kind.kind.name(),
            count: kind.count,
            skipped: kind.skipped,
            complete: kind.complete,
        }
    }
}

pub(super) const fn warning_name(warning: SizeWarning) -> &'static str {
    match warning {
        SizeWarning::NearLimit => "near_limit",
        SizeWarning::OverLimit => "over_limit",
    }
}

pub(super) fn summary_json<'a>(
    archive: &Archive,
    out: &Path,
    summary: &'a CatalogSummary,
) -> SummaryJson<'a> {
    SummaryJson {
        archive: json::path(archive.path()),
        out: json::path(out),
        schema_version: SCHEMA_VERSION,
        archive_key: summary.archive_key().state(),
        encrypted_entries_unread: summary.archive_key().entries_unread(),
        export_health: summary.export_health(),
        provenance: summary.source_inventory(),
        size: summary.size(),
        compressed_size: summary.compressed_size(),
        upload_limit: UPLOAD_LIMIT,
        decompressed_limit: DECOMPRESSED_LIMIT,
        size_warning: summary.size_warning().map(warning_name),
        decompressed_warning: summary.decompressed_warning().map(warning_name),
        kinds: summary.kinds().iter().map(KindJson::from).collect(),
        skipped: summary.skipped(),
        failed: summary.failures().iter().map(FailureJson::from).collect(),
        warnings: summary.warnings().iter().map(WarningJson::from).collect(),
        item_classes: ItemClassJson {
            classified: summary.item_classes().classified,
            other: summary.item_classes().other,
            missing: summary.item_classes().missing,
        },
        paints: summary.paints().into(),
        loadouts: summary.loadouts().into(),
        starmap: summary.starmap().into(),
        shops: summary.shops().into(),
        store_slugs: summary.store_slugs().into(),
        playability: PlayabilityJson {
            items: summary.playability().items.into(),
            ships: summary.playability().ships.into(),
        },
        variants: VariantsJson {
            items: summary.variants().items.into(),
            ships: summary.variants().ships.into(),
        },
        quantizations: QuantizationsJson {
            unused: summary.quantizations().unused,
            unresolved_links: summary.quantizations().unresolved_links,
        },
        texts: summary
            .texts()
            .iter()
            .map(|text| TextJson {
                language: text.language.code(),
                available: text.available,
                resolved: text.resolved,
                unresolved: text.unresolved,
                placeholder: text.placeholder,
            })
            .collect(),
        unresolved: summary.unresolved(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kind_of_every_warning_in_the_json_is_the_kind_the_library_names() {
        let warnings = [
            CraftingWarning::UnreadableRewardRecords(1),
            CraftingWarning::UnknownModifierRanges {
                count: 1,
                details: vec!["c".to_owned()],
            },
            CraftingWarning::AmbiguousDismantle(2),
            CraftingWarning::UnreadableVehicleFiles {
                count: 1,
                details: vec!["a".to_owned()],
            },
            CraftingWarning::UnplacedStarmapContainers(3),
            CraftingWarning::UnreadableShopRegistry,
            CraftingWarning::MissingShopRegistry,
            CraftingWarning::UnreadableShopContainers(4),
            CraftingWarning::UnreadableShopInventories {
                count: 1,
                details: vec!["b".to_owned()],
            },
            CraftingWarning::MissingShopInventories(5),
            CraftingWarning::NoShopComponents,
        ];
        for warning in &warnings {
            let json = serde_json::to_value(WarningJson::from(warning));
            assert_eq!(
                json.as_ref().ok().and_then(|json| json["kind"].as_str()),
                Some(warning.kind()),
                "{warning:?}"
            );
        }
    }
}

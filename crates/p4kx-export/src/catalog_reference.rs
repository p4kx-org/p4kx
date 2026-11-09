// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, Reference};

use crate::selection::Kind;

/// A resolved source Reference whose target is a canonical catalog definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CatalogReference {
    pub(crate) field: String,
    pub(crate) source_type: String,
    pub(crate) target_record_index: u32,
    pub(crate) target_guid: Guid,
    pub(crate) target_record_path: String,
    pub(crate) target_type: String,
    pub(crate) target_kind: Kind,
}

/// Returns the catalog target kind without treating source-only Entity links as failed joins.
pub(crate) fn target_kind(
    source_type: &str,
    parent_type: Option<&str>,
    field: &str,
    reference: &Reference<'_>,
) -> Option<Kind> {
    if reference.location().is_some() {
        return None;
    }
    let name = field.rsplit('.').next()?.split('[').next()?;
    if parent_type == Some("ResourceContainer")
        && name == "entry"
        && (field.starts_with("defaultComposition[") || field.contains(".defaultComposition["))
    {
        return Some(Kind::ResourceTypes);
    }
    match (source_type, name) {
        ("ResourceType", "rttThumbnailEntityClass")
        | ("SResourceTypeDefaultCargoContainers", _)
        | ("BlueprintCategoryDatabaseRecord", "categories") => return None,
        ("ResourceType", "refinedVersion") | ("ResourceTypeGroup", "resources") => {
            return Some(Kind::ResourceTypes);
        }
        ("ResourceTypeGroup", "groups") => return Some(Kind::ResourceGroups),
        ("CraftingQualityDistribution_RecordRef", "qualityDistributionRecord")
        | ("CraftingQualityLocationOverride_RecordRef", "locationOverrideRecord") => {
            return Some(Kind::QualityDistributions);
        }
        ("CraftingQualityQuantization_RecordRef", "qualityQuantizationRecord") => {
            return Some(Kind::QualityQuantizations);
        }
        ("CraftingQualityLocationOverrideEntry", "location") => return Some(Kind::StarmapObjects),
        _ => {}
    }
    actual_kind(reference.type_name())
}

/// Returns the known catalog target family of a concrete source Record type.
pub(crate) fn actual_kind(type_name: &str) -> Option<Kind> {
    match type_name {
        "Tag" => Some(Kind::Tags),
        "SCItemManufacturer" => Some(Kind::Manufacturers),
        "ResourceType" => Some(Kind::ResourceTypes),
        "ResourceTypeGroup" => Some(Kind::ResourceGroups),
        "CraftingQualityDistributionRecord" | "CraftingQualityLocationOverrideRecord" => {
            Some(Kind::QualityDistributions)
        }
        "CraftingQualityQuantizationRecord" => Some(Kind::QualityQuantizations),
        "StarMapObject" => Some(Kind::StarmapObjects),
        _ => None,
    }
}

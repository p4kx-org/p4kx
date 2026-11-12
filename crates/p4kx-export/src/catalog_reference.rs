// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, Reference};

use crate::ExportAccountingError;
use crate::catalog_read::original::ReferenceObservation;
use crate::selection::Kind;

/// A resolved catalog candidate retaining its actual original Reference observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CatalogReference {
    pub(crate) original: ReferenceObservation,
    pub(crate) target_kind: Kind,
}

/// Borrowed descriptive target values used only for final catalog-row resolution.
pub(crate) struct CatalogReferenceTarget<'reference> {
    pub(crate) index: u32,
    pub(crate) guid: Guid,
    pub(crate) path: &'reference str,
    pub(crate) source_type: &'reference str,
    pub(crate) kind: Kind,
}

impl CatalogReference {
    pub(crate) fn target(&self) -> Result<CatalogReferenceTarget<'_>, ExportAccountingError> {
        let data = self.original.data();
        Ok(CatalogReferenceTarget {
            index: data
                .target_record_index
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?,
            guid: data
                .target_guid
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?,
            path: data
                .target_record_path
                .as_ref()
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?
                .as_str(),
            source_type: data
                .target_type
                .as_ref()
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?
                .as_str(),
            kind: self.target_kind,
        })
    }
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
        crate::catalog_v3::faction_definition::SOURCE_TYPE => Some(Kind::Factions),
        crate::catalog_v3::reputation_faction_definition::SOURCE_TYPE => {
            Some(Kind::FactionReputations)
        }
        crate::catalog_v3::reputation_perk_list::SOURCE_TYPE => Some(Kind::ReputationPerks),
        "ResourceType" => Some(Kind::ResourceTypes),
        "ResourceTypeGroup" => Some(Kind::ResourceGroups),
        "CraftingQualityDistributionRecord" | "CraftingQualityLocationOverrideRecord" => {
            Some(Kind::QualityDistributions)
        }
        "CraftingQualityQuantizationRecord" => Some(Kind::QualityQuantizations),
        "StarMapObject" => Some(Kind::StarmapObjects),
        "AmmoParams" => Some(Kind::AmmoDefinitions),
        _ => None,
    }
}

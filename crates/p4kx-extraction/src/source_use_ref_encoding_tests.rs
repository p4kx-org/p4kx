// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use crate::source_details::SourceCoordinateOmission;
use crate::source_inventory::PhysicalSourceRef;
use crate::{DecimalU64, InputScope, Language, SourceEvidenceError, SourceInput, SourceLocation};
use crate::{SourceScope, SourceUse, SourceUseRef, SourceUseRole, SourceUses};

use super::encode;

#[test]
fn compact_coordinates_distinguish_source_use_keys() {
    let locations = locations_with_distinct_coordinates();
    let mut observed = BTreeSet::new();
    for source in ["a0.1", "a0.1.2", "a0.12", ":1é", "é:", "🌑"] {
        let source = PhysicalSourceRef::new(source);
        for scope in [
            InputScope::Payload,
            InputScope::ArchiveDirectoryMetadata,
            InputScope::FileMetadata,
        ] {
            for role in [
                SourceUseRole::Payload,
                SourceUseRole::Selector,
                SourceUseRole::Conversion,
                SourceUseRole::Attempted,
                SourceUseRole::Reference,
            ] {
                for location in &locations {
                    assert!(
                        observed.insert(encode(&source, scope, role, location)),
                        "each distinct original coordinate has a distinct key"
                    );
                }
            }
        }
    }
}

fn locations_with_distinct_coordinates() -> Vec<SourceLocation> {
    let fields = ["", ":", "0.:", "😺", "v", "x", "1:ä", "ö🌑"];
    let namespaces = [
        "dataforgeValue",
        "dataforgeRecordHeader",
        "dataforgeStruct",
        "xml",
        "",
        "v",
        "1:v",
        "?v",
        "🌑",
    ];
    let mut locations = vec![
        SourceLocation::Opaque {
            reason: SourceCoordinateOmission::CaptureLimit,
        },
        SourceLocation::Opaque {
            reason: SourceCoordinateOmission::Unavailable,
        },
        SourceLocation::ArchiveDirectoryPaths {
            start_entry_index: DecimalU64::new(0),
            end_entry_index: DecimalU64::new(u64::MAX),
        },
        SourceLocation::ArchiveDirectoryTreePaths {
            start_entry_index: DecimalU64::new(0),
            end_entry_index: DecimalU64::new(u64::MAX),
        },
    ];
    for field in fields {
        locations.push(SourceLocation::Metadata {
            field: field.into(),
        });
        locations.push(SourceLocation::DataForge {
            record_index: u32::MAX,
            field: field.into(),
        });
        for namespace in namespaces {
            for indices in [vec![], vec![12], vec![1, 2], vec![1, 2, u64::MAX]] {
                locations.push(SourceLocation::Subresource {
                    namespace: namespace.into(),
                    field: field.into(),
                    indices,
                });
            }
        }
    }

    let selectors = ["c2:épsd1:name", "d:.", "🌑"];
    for language in Language::ALL {
        for selector in selectors {
            locations.push(SourceLocation::LocalizationLookup {
                language,
                selector: SourceUseRef::registered(selector.into()),
            });
        }
    }

    locations
}

#[test]
fn lookup_proof_stays_bound_when_source_uses_are_registered_in_reverse_order()
-> Result<(), SourceEvidenceError> {
    let first = SourceUses::with_compact_coordinates();
    let second = SourceUses::with_compact_coordinates();
    let (first_key, first_locale) = key_and_locale(&first)?;
    let (second_key, second_locale) = key_and_locale(&second)?;
    let authored = |field: &str| SourceLocation::DataForge {
        record_index: 4,
        field: field.to_owned(),
    };
    let first_selector = first_key.at(authored("a"), SourceUseRole::Selector)?;
    let _first_other = first_key.at(authored("b"), SourceUseRole::Selector)?;
    let second_other = second_key.at(authored("b"), SourceUseRole::Selector)?;
    let second_selector = second_key.at(authored("a"), SourceUseRole::Selector)?;
    assert_eq!(
        first_selector.reference(),
        second_selector.reference(),
        "fixed physical inputs and authored use are order independent"
    );

    let first_lookup = first_locale.at_localization_lookup(
        Language::English,
        &first_selector,
        SourceUseRole::Payload,
    )?;
    let second_lookup = second_locale.at_localization_lookup(
        Language::English,
        &second_selector,
        SourceUseRole::Payload,
    )?;
    let separate_lookup = second_locale.at_localization_lookup(
        Language::English,
        &second_other,
        SourceUseRole::Payload,
    )?;
    assert_eq!(
        first_lookup.reference(),
        second_lookup.reference(),
        "lookup reuse follows the exact selector"
    );
    assert_ne!(
        second_lookup.reference(),
        separate_lookup.reference(),
        "equal language retains distinct authored key selectors"
    );
    assert_lookup_selector(&first_lookup, &first_selector);
    assert!(
        matches!(
            first_locale.at_localization_lookup(
                Language::English,
                &second_selector,
                SourceUseRole::Payload
            ),
            Err(SourceEvidenceError::ForeignEvidence)
        ),
        "equal reference strings never bind foreign authorities"
    );
    Ok(())
}

fn key_and_locale(
    inventory: &SourceUses,
) -> Result<(SourceScope, SourceScope), SourceEvidenceError> {
    Ok((
        inventory.observe(SourceInput::Unknown, InputScope::Payload)?,
        inventory.observe(SourceInput::Unknown, InputScope::Payload)?,
    ))
}

fn assert_lookup_selector(lookup: &SourceUse, selector: &SourceUse) {
    assert_eq!(
        lookup.description().location(),
        &SourceLocation::LocalizationLookup {
            language: Language::English,
            selector: selector.reference().clone()
        },
        "stored selector references the original registered use"
    );
}

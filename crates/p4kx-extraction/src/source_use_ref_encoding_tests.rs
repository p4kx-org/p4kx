// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use crate::source_details::SourceCoordinateOmission;
use crate::source_inventory::PhysicalSourceRef;
use crate::source_use::LocationView;
use crate::{DecimalU64, InputScope, Language, SourceEvidenceError, SourceInput, SourceLocation};
use crate::{SourceScope, SourceUse, SourceUseRef, SourceUseRole, SourceUses, SourceValue};

use super::encode;

#[path = "source_use_ref_encoding_physical_xml_tests.rs"]
mod physical_xml;

use physical_xml::physical_xml_locations;

#[test]
fn compact_coordinates_distinguish_source_use_keys() {
    let locations = locations_with_distinct_coordinates();
    let mut observed = BTreeSet::new();
    for source in [
        "a0.1", "a0.1.2", "a0.12", "a1.1", "a0x1.1", "a0v1.1", ":1é", "é:", "🌑",
    ] {
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
                        observed.insert(encode(
                            &source,
                            scope,
                            role,
                            LocationView::Original(location)
                        )),
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

    for language in Language::ALL {
        for selector in ["c2:épsd1:name", "d:.", "🌑"] {
            locations.push(SourceLocation::LocalizationLookup {
                language,
                selector: SourceUseRef::registered(selector.into()),
            });
        }
    }

    append_xml_coordinates(&mut locations);
    locations
}

fn append_xml_coordinates(locations: &mut Vec<SourceLocation>) {
    for field in ["$name", "@pos", "@rot", "$absent:@pos", "$absent:@rot"] {
        for indices in [vec![], vec![0], vec![64], vec![1, 0], vec![u64::MAX, 63]] {
            locations.push(xml_location(&indices, field));
        }
    }
    locations.extend(physical_xml_locations());
    for namespace in [
        "cryxmlNode",
        "cryxmlAttribute",
        "textXmlElement",
        "textXmlAttribute",
    ] {
        for indices in [vec![], vec![1], vec![1, 1], vec![1, 2], vec![1, 2, 3]] {
            for field in [
                "$attributes",
                "$children",
                "$name",
                "$node",
                "name",
                "skipPart",
                "Name",
            ] {
                locations.push(subresource(namespace, &indices, field));
            }
        }
    }
}

#[test]
fn common_xml_locations_use_shorter_ids_and_preserve_exact_spelling() {
    let source = PhysicalSourceRef::new("a0.1.2");
    let authored = xml_location(&[273_907, 13], "@pos");
    let encoded = encode(
        &source,
        InputScope::Payload,
        SourceUseRole::Conversion,
        LocationView::Original(&authored),
    );
    let old = "c6:a0.1.2pcsx273907.13.:@pos";
    assert_eq!(
        encoded.as_str(),
        "c6:a0.1.2pcxpBC3z.N",
        "the full XML occurrence has a stable compact key"
    );
    assert_eq!(
        encoded.as_str().len() + 9,
        old.len(),
        "the common token saves exactly nine bytes"
    );

    let uppercase = xml_location(&[273_907, 13], "@Pos");
    let encoded = encode(
        &source,
        InputScope::Payload,
        SourceUseRole::Conversion,
        LocationView::Original(&uppercase),
    );
    assert_eq!(
        encoded.as_str(),
        "c6:a0.1.2pcsx273907.13.:@Pos",
        "unknown spellings retain their original representation"
    );
}

#[test]
fn xml_use_ids_and_cell_origins_are_stable_across_worker_counts() -> Result<(), XmlInventoryError> {
    use rayon::ThreadPoolBuilder;

    let one = ThreadPoolBuilder::new().num_threads(1).build()?;
    let four = ThreadPoolBuilder::new().num_threads(4).build()?;
    assert_eq!(
        one.install(|| xml_inventory(false))?,
        four.install(|| xml_inventory(true))?,
        "the completed cell and inventory serialize equally after reordered parallel registration"
    );
    Ok(())
}

fn xml_inventory(reverse: bool) -> Result<Vec<u8>, XmlInventoryError> {
    use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

    let inventory = SourceUses::with_compact_coordinates();
    let scope = inventory.observe(SourceInput::Unknown, InputScope::Payload)?;
    let mut locations = vec![
        xml_location(&[0], "$name"),
        xml_location(&[64, 0], "@pos"),
        xml_location(&[0, 64], "@rot"),
        xml_location(&[4_096], "$absent:@pos"),
        xml_location(&[u64::MAX], "$absent:@rot"),
        xml_location(&[64, 0], "@Pos"),
    ];
    locations.extend(physical_xml_locations());
    if reverse {
        locations.reverse();
    }
    let uses = locations
        .par_iter()
        .map(|location| scope.at(location.clone(), SourceUseRole::Conversion))
        .collect::<Result<Vec<_>, _>>()?;
    assert_xml_uses(&locations, &uses);
    let cell = inventory.value(SourceValue::Present(true), &uses, true);
    assert_eq!(
        cell.origin().contains_encrypted_source(),
        None,
        "unknown source proof stays unknown"
    );
    assert!(
        cell.origin().source_set_complete(),
        "an observed set may be complete with unknown encryption"
    );
    let frozen = inventory.freeze()?;
    assert_eq!(
        frozen.use_count(),
        locations.len(),
        "one inventory row per original coordinate"
    );
    for reference in cell.origin().source_uses_refs() {
        assert!(
            frozen
                .iter_uses()
                .any(|description| description.id() == reference),
            "the immediate references close over the frozen inventory"
        );
    }
    Ok(serde_json::to_vec(&(cell, frozen))?)
}

fn assert_xml_uses(locations: &[SourceLocation], uses: &[SourceUse]) {
    for (location, observed) in locations.iter().zip(uses) {
        assert_eq!(
            observed.description().location(),
            location,
            "an opaque ID retains its original XML coordinate"
        );
        assert_eq!(
            observed.description().scope(),
            InputScope::Payload,
            "the use retains the original byte scope"
        );
        assert_eq!(
            observed.role(),
            SourceUseRole::Conversion,
            "the contributor role survives"
        );
    }
}

type XmlInventoryError = Box<dyn std::error::Error + Send + Sync>;

fn xml_location(indices: &[u64], field: &str) -> SourceLocation {
    subresource("xml", indices, field)
}

fn subresource(namespace: &str, indices: &[u64], field: &str) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: namespace.to_owned(),
        indices: indices.to_vec(),
        field: field.to_owned(),
    }
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

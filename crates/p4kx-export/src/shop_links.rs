// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{FieldOrigin, SourceUses, SourceValue, SourcedValue};

use crate::CatalogError;
use crate::catalog_v3::starmap_placement::PlacementBinding;
use crate::contract_starmap::{ShopLinkHeader, StarmapObject};
use crate::guid_text::CatalogGuid;
use crate::starmap_containers::proof::{PlacementEvidence, PlacementField};

/// One admitted Starmap owner and its canonical name observations.
pub(crate) struct ShopLinkSubject<'starmap> {
    pub(crate) guid: CatalogGuid,
    pub(crate) class_name: &'starmap str,
    pub(crate) entity_name: Option<&'starmap str>,
    pub(crate) header: Option<&'starmap ShopLinkHeader>,
    pub(crate) placement: Option<&'starmap PlacementEvidence>,
}

impl<'starmap> From<&'starmap StarmapObject<'_>> for ShopLinkSubject<'starmap> {
    fn from(object: &'starmap StarmapObject<'_>) -> Self {
        Self {
            guid: object.guid,
            class_name: object.class_name,
            entity_name: object.entity_name.as_deref(),
            header: object.shop_link_header.as_ref(),
            placement: object.placement_proof.as_ref(),
        }
    }
}

/// Canonical Starmap candidates by case-folded class and selected entity name.
pub(crate) struct Links<'starmap> {
    by_name: HashMap<String, Matches<'starmap>>,
    names_complete: bool,
}

impl<'starmap> Links<'starmap> {
    pub(crate) fn new(
        starmap: impl IntoIterator<Item = ShopLinkSubject<'starmap>>,
        names_complete: bool,
    ) -> Self {
        let mut by_name: HashMap<String, Matches<'_>> = HashMap::new();
        for object in starmap {
            let names = [
                Some((object.class_name, Name::Class)),
                object.entity_name.map(|name| {
                    (
                        name,
                        Name::PlacedEntity {
                            name,
                            placement: object.placement,
                        },
                    )
                }),
            ];
            for (name, field) in names.into_iter().flatten() {
                if name.is_empty() {
                    continue;
                }
                let matches = by_name
                    .entry(name.to_ascii_lowercase())
                    .or_insert_with(|| Matches {
                        winner: Some(object.guid),
                        candidates: Vec::new(),
                    });
                if matches.winner != Some(object.guid) {
                    matches.winner = None;
                }
                matches.candidates.push(Match {
                    header: object.header,
                    field,
                });
            }
        }
        Self {
            by_name,
            names_complete,
        }
    }

    pub(crate) fn resolve(
        &self,
        location: &SourcedValue<String>,
        sources: &SourceUses,
    ) -> Result<SourcedValue<CatalogGuid>, CatalogError> {
        let Some(location_name) = location.value() else {
            return Ok(sources.value_with_origins(SourceValue::Absent, &[location.origin()])?);
        };
        let Some(matches) = self.by_name.get(&location_name.to_ascii_lowercase()) else {
            let state = if self.names_complete {
                SourceValue::Absent
            } else {
                SourceValue::Unreadable
            };
            let incomplete = sources.value(state, &[], false);
            return Ok(sources.add_value_origins(incomplete, &[location.origin()])?);
        };
        let mut origins = vec![location.origin().clone()];
        let mut complete = self.names_complete;
        for candidate in &matches.candidates {
            complete &= candidate.origins(sources, &mut origins)?;
        }
        let outcome = matches
            .winner
            .map_or(SourceValue::Invalid, SourceValue::Present);
        let outcome = sources.value(outcome, &[], complete);
        let originals: Vec<_> = origins.iter().collect();
        Ok(sources.add_value_origins(outcome, &originals)?)
    }
}

struct Matches<'starmap> {
    winner: Option<CatalogGuid>,
    candidates: Vec<Match<'starmap>>,
}

struct Match<'starmap> {
    header: Option<&'starmap ShopLinkHeader>,
    field: Name<'starmap>,
}

enum Name<'starmap> {
    Class,
    PlacedEntity {
        name: &'starmap str,
        placement: Option<&'starmap PlacementEvidence>,
    },
}

impl Match<'_> {
    fn origins(
        &self,
        sources: &SourceUses,
        origins: &mut Vec<FieldOrigin>,
    ) -> Result<bool, CatalogError> {
        let header = self.header;
        if let Some(header) = header {
            origins.push(header.guid.clone());
        }
        let name = match self.field {
            Name::Class => header.map(|header| header.class_name.clone()),
            Name::PlacedEntity { name, placement } => {
                let binding = PlacementBinding::new(sources, placement)?;
                let name = binding.cell(Some(name), PlacementField::Entity)?;
                Some(name.origin().clone())
            }
        };
        let present = name.is_some();
        origins.extend(name);
        Ok(header.is_some() && present)
    }
}

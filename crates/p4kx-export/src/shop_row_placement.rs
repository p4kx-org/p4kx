// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, SourceUses, SourceValue, SourcedValue};

use super::ShopNameCoverage;
use crate::catalog_queries::starmap_name_owners::{AllStarmapNames, NameInput};
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::EntryCapture;
use crate::guid_text::CatalogGuid;
use crate::shop_links::{Links, ShopLinkSubject};
use crate::starmap::ObjectPlacement;
use crate::starmap_containers::Placements;
use crate::{CatalogError, StarmapTraversalBudget};

pub(super) struct Lookup<'query, 'capture, 'data> {
    archive: &'query Archive,
    capture: &'capture EntryCapture<'capture>,
    budget: StarmapTraversalBudget,
    names_input: NameInput<'query, 'data>,
    cancel: Option<&'query CancelToken>,
    survey: Option<NameSurvey>,
}

struct NameSurvey {
    names: AllStarmapNames,
    placements: Placements,
    routes_complete: bool,
}

impl<'query, 'capture, 'data> Lookup<'query, 'capture, 'data> {
    pub(super) const fn new(
        archive: &'query Archive,
        capture: &'capture EntryCapture<'capture>,
        budget: StarmapTraversalBudget,
        names_input: NameInput<'query, 'data>,
        cancel: Option<&'query CancelToken>,
    ) -> Self {
        Self {
            archive,
            capture,
            budget,
            names_input,
            cancel,
            survey: None,
        }
    }

    pub(super) const fn capture(&self) -> &'capture EntryCapture<'capture> {
        self.capture
    }

    pub(super) fn surveys(&self) -> usize {
        usize::from(self.survey.is_some())
    }

    pub(super) fn resolve(
        &mut self,
        location: &SourcedValue<String>,
    ) -> Result<(SourcedValue<CatalogGuid>, Option<ShopNameCoverage>), CatalogError> {
        let sources = self.capture.source_uses();
        let Some(wanted) = location.value() else {
            let absent = sources.value_with_origins(SourceValue::Absent, &[location.origin()])?;
            return Ok((absent, None));
        };
        let survey = if let Some(survey) = &self.survey {
            survey
        } else {
            let names =
                AllStarmapNames::read(self.archive, self.capture, self.names_input, self.cancel)?;
            let placements = Placements::read_for_shop_names(
                self.archive,
                self.cancel,
                self.budget,
                self.capture,
                &names,
            )?;
            let routes_complete = placements.name_coverage.complete_routes(placements.counts);
            self.survey.insert(NameSurvey {
                names,
                placements,
                routes_complete,
            })
        };
        let linked = survey.resolve(wanted, location, sources, self.cancel)?;
        Ok((
            linked,
            Some(ShopNameCoverage {
                records: survey.names.coverage.clone(),
                routes_complete: survey.routes_complete,
            }),
        ))
    }
}

impl NameSurvey {
    fn resolve(
        &self,
        wanted: &str,
        location: &SourcedValue<String>,
        sources: &SourceUses,
        cancel: Option<&CancelToken>,
    ) -> Result<SourcedValue<CatalogGuid>, CatalogError> {
        let mut candidates = Vec::new();
        for owner in &self.names.owners {
            checkpoint(cancel)?;
            if !owner.class_name.eq_ignore_ascii_case(wanted)
                && !self
                    .placements
                    .get(owner.guid.get())
                    .is_some_and(|placement| placement.entity_name.eq_ignore_ascii_case(wanted))
            {
                continue;
            }
            candidates
                .try_reserve(1)
                .map_err(CatalogError::SourceUseAllocation)?;
            candidates.push((owner, ObjectPlacement::select(owner.guid, &self.placements)));
        }
        let links = Links::new(
            candidates.iter().map(|(owner, placement)| ShopLinkSubject {
                guid: owner.guid,
                class_name: &owner.class_name,
                entity_name: placement.entity_name.as_deref(),
                header: Some(&owner.header),
                placement: placement.proof.as_ref(),
            }),
            self.names.complete() && self.routes_complete,
        );
        links.resolve(location, sources)
    }
}

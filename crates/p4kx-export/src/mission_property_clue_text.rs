// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::CancelToken;

use super::{ClueDetails, InvestigationClue};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::mission_property_investigation::Investigation;

impl<'source> Investigation<'source> {
    /// Publishes authored name and clue text from the retained investigation.
    pub(crate) fn publish_texts(
        &mut self,
        projection: &CatalogProjection<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        for suspect in self.suspects.iter_mut().flatten().flatten() {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            if let Some(name) = &mut suspect.name {
                name.name.publish_texts(projection)?;
            }
        }
        for location in self.locations.iter_mut().flatten().flatten() {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            for item in location.clue_items.iter_mut().flatten().flatten() {
                for clue in item.clues.iter_mut().flatten().flatten() {
                    if cancel.is_some_and(CancelToken::is_cancelled) {
                        return Err(CatalogError::Cancelled);
                    }
                    clue.publish_texts(projection)?;
                }
            }
        }
        Ok(())
    }
}

impl<'source> InvestigationClue<'source> {
    fn publish_texts(
        &mut self,
        projection: &CatalogProjection<'_, 'source>,
    ) -> Result<(), CatalogError> {
        self.objective_long_description.publish(projection)?;
        self.objective_short_description.publish(projection)?;
        if let ClueDetails::CorpseScanInfo {
            name,
            cause_of_death,
        } = &mut self.details
        {
            if let Some(name) = name {
                name.name.publish_texts(projection)?;
            }
            cause_of_death.publish(projection)?;
        }
        Ok(())
    }
}

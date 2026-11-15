// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, SourcedValue, StructValue};
use serde::{Serialize, Serializer};

use super::{ClueDetails, InvestigationClue};
use crate::catalog_projection::{CatalogProjection, ProtectedKey};
use crate::catalog_text_contract::GameTextRef;
use crate::mission_property_investigation::Investigation;
use crate::record_read::ReadError;
use crate::source_fields::FieldCapture;
use crate::{CatalogError, ExportAccountingError};

/// One clue text before owner selection or after protected publication.
#[derive(Debug)]
pub(crate) enum ClueText<'source> {
    Pending {
        token: Option<&'source str>,
        key: Option<ProtectedKey<'source>>,
    },
    Published(SourcedValue<GameTextRef>),
}

impl<'source> ClueText<'source> {
    pub(crate) fn read(
        source: &StructValue<'source>,
        field: &str,
        token: Option<&'source str>,
        capture: Option<&dyn FieldCapture<'source>>,
    ) -> Result<Self, ReadError> {
        let key = capture
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?
            .protected_locale(source, field)?;
        Ok(Self::Pending {
            token,
            key: Some(key),
        })
    }

    fn publish(&mut self, projection: &CatalogProjection<'_, 'source>) -> Result<(), CatalogError> {
        let Self::Pending { key, .. } = self else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let key = key
            .take()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        *self = Self::Published(projection.finish_protected_key(key)?);
        Ok(())
    }
}

impl Serialize for ClueText<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Pending { token, .. } => token.serialize(serializer),
            Self::Published(text) => text.serialize(serializer),
        }
    }
}

impl<'source> Investigation<'source> {
    /// Publishes the three authored text occurrences of each retained clue.
    pub(crate) fn publish_clue_texts(
        &mut self,
        projection: &CatalogProjection<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
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
        if let ClueDetails::CorpseScanInfo { cause_of_death, .. } = &mut self.details {
            cause_of_death.publish(projection)?;
        }
        Ok(())
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourcedValue, StructValue};
use serde::{Serialize, Serializer};

use crate::catalog_projection::{CatalogProjection, ProtectedKey};
use crate::catalog_text_contract::GameTextRef;
use crate::mission_layers;
use crate::npc_spawn_model::NpcSpawnName;
use crate::record_read::ReadError;
use crate::source_fields::FieldCapture;
use crate::{CatalogError, ExportAccountingError};

/// A mission text before owner selection or after protected publication.
#[derive(Debug)]
pub(crate) enum MissionPropertyText<'source> {
    Pending {
        token: Option<&'source str>,
        key: Option<ProtectedKey<'source>>,
    },
    Published(SourcedValue<GameTextRef>),
}

impl<'source> MissionPropertyText<'source> {
    pub(crate) const fn pending(token: Option<&'source str>, key: ProtectedKey<'source>) -> Self {
        Self::Pending {
            token,
            key: Some(key),
        }
    }

    pub(crate) fn read_key(
        source: &StructValue<'source>,
        field: &str,
        capture: Option<&dyn FieldCapture<'source>>,
    ) -> Result<Self, ReadError> {
        let key = capture
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?
            .protected_key(source, field)?;
        Ok(Self::pending(mission_layers::key(Some(source), field), key))
    }

    pub(crate) fn read_locale(
        source: &StructValue<'source>,
        field: &str,
        token: Option<&'source str>,
        capture: Option<&dyn FieldCapture<'source>>,
    ) -> Result<Self, ReadError> {
        let key = capture
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?
            .protected_locale(source, field)?;
        Ok(Self::pending(token, key))
    }

    pub(crate) fn publish(
        &mut self,
        projection: &CatalogProjection<'_, 'source>,
    ) -> Result<(), CatalogError> {
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

impl<'source> NpcSpawnName<MissionPropertyText<'source>> {
    pub(crate) fn publish_texts(
        &mut self,
        projection: &CatalogProjection<'_, 'source>,
    ) -> Result<(), CatalogError> {
        self.given_name.publish(projection)?;
        self.given_last_name.publish(projection)?;
        self.given_nick_name.publish(projection)
    }
}

impl Serialize for MissionPropertyText<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Pending { token, .. } => token.serialize(serializer),
            Self::Published(text) => text.serialize(serializer),
        }
    }
}

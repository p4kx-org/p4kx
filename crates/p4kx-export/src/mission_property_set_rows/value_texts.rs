// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::CancelToken;

use crate::catalog_projection::CatalogProjection;
use crate::mission_property_option::OptionMetadata;
use crate::template_property::PropertyValue;
use crate::{CatalogError, ExportAccountingError};

impl<'source> PropertyValue<'source> {
    pub(super) fn publish_texts(
        &mut self,
        projection: &CatalogProjection<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        match self {
            Self::StringHash { options } => publish_options(
                options.as_deref_mut(),
                |option| &mut option.metadata,
                projection,
                cancel,
            ),
            Self::Integer { options } => publish_options(
                options.as_deref_mut(),
                |option| &mut option.metadata,
                projection,
                cancel,
            ),
            Self::Float { options } => publish_options(
                options.as_deref_mut(),
                |option| &mut option.metadata,
                projection,
                cancel,
            ),
            Self::Tags { options, .. } => publish_options(
                options.as_deref_mut(),
                |option| &mut option.metadata,
                projection,
                cancel,
            ),
            Self::Investigation(investigation) => {
                investigation.publish_clue_texts(projection, cancel)
            }
            _ => Ok(()),
        }
    }
}

fn publish_options<'source, T>(
    options: Option<&mut [Option<T>]>,
    mut metadata: impl FnMut(&mut T) -> &mut OptionMetadata<'source>,
    projection: &CatalogProjection<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(), CatalogError> {
    let Some(options) = options else {
        return Ok(());
    };
    for option in options.iter_mut().flatten() {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        let metadata = metadata(option);
        let key = metadata
            .pending_name
            .take()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        metadata.display_name = Some(projection.finish_protected_key(key)?);
    }
    Ok(())
}

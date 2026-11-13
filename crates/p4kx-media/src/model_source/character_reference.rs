// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::character_origin::CharacterElement;
use super::{origins, path_reference};
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter, Reference, SourceContext};

pub(super) struct CharacterReferences<'context, 'input> {
    pub(super) context: &'context SourceContext<'input>,
}

impl CharacterReferences<'_, '_> {
    pub(super) fn field(
        &self,
        element: &CharacterElement<'_, '_>,
        name: &str,
        pointer: &str,
        prepared: &mut PreparedRecordOrigins,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Option<Reference>, MediaError> {
        let (value, authored) = element.value(name, writer)?;
        let Some(path) = value else {
            origins::at(prepared, pointer, authored, writer)?;
            return Ok(None);
        };
        path_reference::field(
            self.context.directory,
            path,
            pointer,
            &authored,
            prepared,
            writer,
        )
        .map(Some)
    }
}

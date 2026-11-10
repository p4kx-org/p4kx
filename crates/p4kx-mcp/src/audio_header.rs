// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, Record};

use crate::audio_shapes::{EntityRef, EntityShape};
use crate::budget::Budget;
use crate::budget_text;
use crate::error::ToolError;

const CLASS_PREFIX: &str = "EntityClassDefinition.";

struct Header<'source> {
    guid: Guid,
    name: &'source str,
    path: &'source str,
    class_name: &'source str,
}

impl<'source> Header<'source> {
    fn read(record: &Record<'source>) -> Result<Self, ToolError> {
        let name = record.name()?;
        Ok(Self {
            guid: record.guid(),
            name,
            path: record.record_path()?,
            class_name: name.strip_prefix(CLASS_PREFIX).unwrap_or(name),
        })
    }
}

pub(super) fn full(record: &Record<'_>, text: &mut Budget) -> Result<EntityShape, ToolError> {
    let header = Header::read(record)?;
    Ok(EntityShape {
        guid: budget_text::display(text, &header.guid)?,
        record_path: text.text(header.path)?,
        name: text.text(header.name)?,
        class_name: text.text(header.class_name)?,
    })
}

pub(super) fn reference(record: &Record<'_>, text: &mut Budget) -> Result<EntityRef, ToolError> {
    let header = Header::read(record)?;
    Ok(EntityRef {
        guid: budget_text::display(text, &header.guid)?,
        record_path: text.text(header.path)?,
        class_name: text.text(header.class_name)?,
    })
}

#[cfg(test)]
#[path = "audio_header_tests.rs"]
mod tests;

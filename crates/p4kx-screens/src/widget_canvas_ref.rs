// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{Guid, StructValue, Value};

#[derive(Debug, Clone, Copy)]
pub(crate) enum WidgetCanvasRef {
    Absent,
    Unreadable,
    Present { guid: Guid, record: u32 },
}

impl WidgetCanvasRef {
    pub(crate) fn read(widget: &StructValue<'_>) -> Self {
        match widget.field("canvas") {
            Some(Value::Reference(reference)) => Self::Present {
                guid: reference.guid(),
                record: reference.original_target().record_index(),
            },
            Some(Value::Error(_)) => Self::Unreadable,
            _ => Self::Absent,
        }
    }

    pub(crate) const fn guid(self) -> Option<Guid> {
        match self {
            Self::Present { guid, .. } => Some(guid),
            _ => None,
        }
    }

    pub(crate) const fn is_unreadable(self) -> bool {
        matches!(self, Self::Unreadable)
    }
}

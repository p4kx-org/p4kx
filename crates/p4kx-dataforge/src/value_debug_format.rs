// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::events::Events;
use super::leaf::Leaf;
use crate::{StructValue, Value};
use std::fmt;

impl fmt::Debug for StructValue<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StructValue")
            .field("type_name", &self.type_name)
            .field("fields", &Events::fields(&self.fields))
            .finish()
    }
}

impl fmt::Debug for Value<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Struct(structure) => formatter.debug_tuple("Struct").field(structure).finish(),
            Self::Array(items) => formatter
                .debug_tuple("Array")
                .field(&Events::items(items))
                .finish(),
            leaf => fmt::Debug::fmt(&Leaf::new(leaf).ok_or(fmt::Error)?, formatter),
        }
    }
}

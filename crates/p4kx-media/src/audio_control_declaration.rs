// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{SourceDeclaration, SourceSpanError};

use crate::audio_control_fields::{AtlFamily, AtlField};
use crate::audio_control_payload::{AtlDeclaration, AtlDeclarationKind, AtlFieldRecord};
use crate::xml_source_projection::declaration_position;

pub(crate) struct ObservedDeclaration<'source> {
    pub(crate) original: AtlDeclaration<'source>,
    pub(crate) typed: Option<TypedDeclaration<'source>>,
}

pub(crate) struct TypedDeclaration<'source> {
    pub(crate) family: AtlFamily,
    pub(crate) record: AtlFieldRecord<'source>,
}

impl<'source> ObservedDeclaration<'source> {
    pub(crate) fn read(
        declaration: &SourceDeclaration<'source>,
        family: Option<AtlFamily>,
    ) -> Result<Self, SourceSpanError> {
        let original = AtlDeclaration {
            qualified_name: declaration.qualified_name(),
            value: declaration.value(),
            lexeme: declaration.lexeme(),
            kind: AtlDeclarationKind::try_from(declaration.kind())?,
            position: declaration_position(declaration.position())?,
        };
        let typed = family.and_then(|family| {
            AtlField::from_attribute(family, original.qualified_name).map(|field| {
                TypedDeclaration {
                    family,
                    record: AtlFieldRecord {
                        field,
                        typed_value: field.value(family, original.value),
                    },
                }
            })
        });
        Ok(Self { original, typed })
    }
}

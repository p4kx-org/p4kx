// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use crate::definition_reference::DefinitionStatus;
use crate::mission_layers;
use crate::mission_property_name::{InvestigationName, InvestigationNameSource};
use crate::npc_spawn_model::AiName;
use crate::record_read::ReadError;
use crate::source_fields::{self, FieldCapture, FieldChecks, ReferenceShape};
use crate::texts::Texts;

pub(crate) fn read<'source>(
    source: Option<&StructValue<'source>>,
    path: &str,
    texts: &Texts<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<Option<InvestigationName<'source>>, ReadError> {
    source
        .map(|source| name(source, path, texts, capture))
        .transpose()
}

/// Checks a nullable investigation-name configuration without locale lookup.
#[cfg(test)]
pub(crate) fn validate(source: Option<&StructValue<'_>>, path: &str) -> Result<(), ReadError> {
    validate_context(source, path, None)
}

pub(crate) fn validate_context<'source>(
    source: Option<&StructValue<'source>>,
    path: &str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<(), ReadError> {
    if let Some(source) = source {
        let view = NameView::read(source, path, capture)?;
        view.given_name()?;
        view.given_last_name()?;
        view.given_nick_name()?;
        view.name_data()?;
        view.random_name()?;
        view.random_last_name()?;
        view.random_nick_name()?;
        view.chance()?;
    }
    Ok(())
}

fn name<'source>(
    source: &StructValue<'source>,
    path: &str,
    texts: &Texts<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<InvestigationName<'source>, ReadError> {
    let view = NameView::read(source, path, capture)?;
    let original = InvestigationNameSource {
        given_name: view.given_name()?,
        given_last_name: view.given_last_name()?,
        given_nick_name: view.given_nick_name()?,
        name_data: view.name_data()?.map(ReferenceShape::into_owned),
    };
    let presentation = AiName {
        random_name: view.random_name()?,
        random_last_name: view.random_last_name()?,
        random_nick_name: view.random_nick_name()?,
        given_name: texts.localised(mission_layers::key(Some(source), "characterGivenName")),
        given_last_name: texts
            .localised(mission_layers::key(Some(source), "characterGivenLastName")),
        given_nick_name: texts
            .localised(mission_layers::key(Some(source), "characterGivenNickName")),
        chance_of_nick_name: view.chance()?,
        name_data_guid: original
            .name_data
            .as_ref()
            .filter(|reference| matches!(reference.status, DefinitionStatus::Present))
            .map(|reference| reference.guid),
    };
    Ok(InvestigationName {
        name: presentation,
        source: original,
    })
}

struct NameView<'capture, 'view, 'source> {
    fields: FieldChecks<'capture, 'view, 'source>,
}

impl<'capture, 'view, 'source> NameView<'capture, 'view, 'source> {
    fn read(
        source: &'view StructValue<'source>,
        path: &'view str,
        capture: Option<&'capture dyn FieldCapture<'source>>,
    ) -> Result<Self, ReadError> {
        if source.type_name() != "MissionPropertyValue_AIName" {
            return Err(source_fields::unknown_struct(source, "name", capture));
        }
        let fields = FieldChecks::for_context(source, path, capture)?;
        fields.check(&[
            "randomName",
            "randomLastName",
            "randomNickName",
            "characterGivenName",
            "characterGivenLastName",
            "characterGivenNickName",
            "characterNameData",
            "chanceOfNickName",
        ])?;
        Ok(Self { fields })
    }

    fn given_name(&self) -> Result<Option<&'source str>, ReadError> {
        self.fields.locale("characterGivenName")
    }
    fn given_last_name(&self) -> Result<Option<&'source str>, ReadError> {
        self.fields.locale("characterGivenLastName")
    }
    fn given_nick_name(&self) -> Result<Option<&'source str>, ReadError> {
        self.fields.locale("characterGivenNickName")
    }
    fn name_data(&self) -> Result<Option<ReferenceShape<'view, 'source>>, ReadError> {
        self.fields.reference_view("characterNameData")
    }
    fn random_name(&self) -> Result<Option<bool>, ReadError> {
        self.fields.boolean("randomName")
    }
    fn random_last_name(&self) -> Result<Option<bool>, ReadError> {
        self.fields.boolean("randomLastName")
    }
    fn random_nick_name(&self) -> Result<Option<bool>, ReadError> {
        self.fields.boolean("randomNickName")
    }
    fn chance(&self) -> Result<Option<f32>, ReadError> {
        self.fields.single("chanceOfNickName")
    }
}

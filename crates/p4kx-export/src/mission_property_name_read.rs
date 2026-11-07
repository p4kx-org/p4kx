// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use crate::definition_reference::DefinitionStatus;
use crate::mission_layers::key;
use crate::mission_property_name::{InvestigationName, InvestigationNameSource};
use crate::mission_property_source;
use crate::npc_spawn_model::AiName;
use crate::record_read::ReadError;
use crate::source_fields::Fields;
use crate::template_issues::TemplateError;
use crate::texts::Texts;

pub(crate) fn read<'f>(
    source: Option<&StructValue<'f>>,
    path: &str,
    texts: &Texts<'f>,
) -> Result<Option<InvestigationName<'f>>, ReadError> {
    source.map(|source| name(source, path, texts)).transpose()
}

fn name<'f>(
    source: &StructValue<'f>,
    path: &str,
    texts: &Texts<'f>,
) -> Result<InvestigationName<'f>, ReadError> {
    if source.type_name() != "MissionPropertyValue_AIName" {
        return Err(TemplateError::unknown("name", source).into());
    }
    mission_property_source::check(
        source,
        path,
        &[
            "randomName",
            "randomLastName",
            "randomNickName",
            "characterGivenName",
            "characterGivenLastName",
            "characterGivenNickName",
            "characterNameData",
            "chanceOfNickName",
        ],
    )?;
    let fields = Fields::new(source, path);
    let original = InvestigationNameSource {
        given_name: fields.locale("characterGivenName")?,
        given_last_name: fields.locale("characterGivenLastName")?,
        given_nick_name: fields.locale("characterGivenNickName")?,
        name_data: fields.reference("characterNameData")?,
    };
    let presentation = AiName {
        random_name: fields.boolean("randomName")?,
        random_last_name: fields.boolean("randomLastName")?,
        random_nick_name: fields.boolean("randomNickName")?,
        given_name: texts.localised(key(Some(source), "characterGivenName")),
        given_last_name: texts.localised(key(Some(source), "characterGivenLastName")),
        given_nick_name: texts.localised(key(Some(source), "characterGivenNickName")),
        chance_of_nick_name: fields.single("chanceOfNickName")?,
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

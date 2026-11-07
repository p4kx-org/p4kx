// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use crate::mission_property_clue::{
    ClueDetails, ClueItemDetails, InvestigationClue, InvestigationClueItem,
};
use crate::mission_property_name_read;
use crate::mission_property_source;
use crate::record_read::ReadError;
use crate::source_fields::Fields;
use crate::spawn_read::Reader;
use crate::template_issues::TemplateError;
use crate::texts::Texts;

pub(crate) fn item<'f>(
    reader: &Reader<'_, 'f>,
    source: &StructValue<'f>,
    path: &str,
    texts: &Texts<'f>,
) -> Result<InvestigationClueItem<'f>, ReadError> {
    let fields = Fields::new(source, path);
    let (details, known) = match source.type_name() {
        "Investigation_ClueItem" => (
            ClueItemDetails::Base,
            &["entityClass", "spawnTags", "clues"][..],
        ),
        "Investigation_ClueItem_Corpse" => (
            ClueItemDetails::Corpse {
                suspect_represented: mission_property_source::weak(
                    reader,
                    source,
                    "suspectRepresented",
                    path,
                )?,
            },
            &["entityClass", "spawnTags", "clues", "suspectRepresented"][..],
        ),
        "Investigation_ClueItem_Datapad" => (
            ClueItemDetails::Datapad {
                canvas: fields.reference("canvas")?,
            },
            &["entityClass", "spawnTags", "clues", "canvas"][..],
        ),
        _ => return Err(TemplateError::unknown("clueItems", source).into()),
    };
    mission_property_source::check(source, path, known)?;
    Ok(InvestigationClueItem {
        entity_class: fields.reference("entityClass")?,
        spawn_tags: mission_property_source::tags(
            fields.structure("spawnTags")?,
            &format!("{path}.spawnTags"),
        )?,
        clues: mission_property_source::list(source, "clues", path, |value, path| {
            clue(value, path, texts)
        })?,
        details,
    })
}

fn clue<'f>(
    source: &StructValue<'f>,
    path: &str,
    texts: &Texts<'f>,
) -> Result<InvestigationClue<'f>, ReadError> {
    let fields = Fields::new(source, path);
    let (details, known) = match source.type_name() {
        "Investigation_Clue" => (
            ClueDetails::Base,
            &["objectiveLongDescription", "objectiveShortDescription"][..],
        ),
        "Investigation_Clue_CorpseScanInfo" => (
            ClueDetails::CorpseScanInfo {
                name: mission_property_name_read::read(
                    fields.structure("name")?,
                    &format!("{path}.name"),
                    texts,
                )?
                .map(Box::new),
                cause_of_death: texts.localised(fields.locale("causeOfDeath")?),
            },
            &[
                "objectiveLongDescription",
                "objectiveShortDescription",
                "name",
                "causeOfDeath",
            ][..],
        ),
        "Investigation_Clue_JournalEntry" => (
            ClueDetails::JournalEntry {
                journal_entry: fields.reference("journalEntry")?,
            },
            &[
                "objectiveLongDescription",
                "objectiveShortDescription",
                "journalEntry",
            ][..],
        ),
        _ => return Err(TemplateError::unknown("clues", source).into()),
    };
    mission_property_source::check(source, path, known)?;
    Ok(InvestigationClue {
        objective_long_description: texts.localised(fields.locale("objectiveLongDescription")?),
        objective_short_description: texts.localised(fields.locale("objectiveShortDescription")?),
        details,
    })
}

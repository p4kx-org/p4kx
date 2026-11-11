// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value};

use crate::mission_property_clue::{
    ClueDetails, ClueItemDetails, InvestigationClue, InvestigationClueItem,
};
use crate::mission_property_name_read;
use crate::mission_property_source;
use crate::owned_reference_read;
use crate::record_read::ReadError;
use crate::source_fields::{self, FieldCapture, FieldChecks, ReferenceShape};
use crate::spawn_read::Reader;
use crate::texts::Texts;

pub(crate) fn item<'source>(
    reader: &Reader<'_, 'source>,
    source: &StructValue<'source>,
    path: &str,
    texts: &Texts<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<InvestigationClueItem<'source>, ReadError> {
    let view = ItemView::read(source, path, capture)?;
    let details = item_details(reader, &view)?;
    view.check()?;
    Ok(InvestigationClueItem {
        entity_class: view.entity()?.map(ReferenceShape::into_owned),
        spawn_tags: mission_property_source::tags_context(
            view.spawn_tags()?,
            &format!("{path}.spawnTags"),
            capture,
        )?,
        clues: mission_property_source::list(source, "clues", path, capture, |value, path| {
            clue(value, path, texts, capture)
        })?,
        details,
    })
}

fn item_details<'source>(
    reader: &Reader<'_, 'source>,
    view: &ItemView<'_, '_, 'source>,
) -> Result<ClueItemDetails, ReadError> {
    Ok(match view.kind {
        ItemKind::Base => ClueItemDetails::Base,
        ItemKind::Corpse => ClueItemDetails::Corpse {
            suspect_represented: view
                .suspect()?
                .map(|value| {
                    owned_reference_read::read(reader, value)
                        .map(|target| target.map(|target| target.reference))
                        .map_err(|()| view.invalid_suspect(value))
                })
                .transpose()?
                .flatten(),
        },
        ItemKind::Datapad => ClueItemDetails::Datapad {
            canvas: view.canvas()?.map(ReferenceShape::into_owned),
        },
    })
}

/// Checks all supported clue-item data without resolving pointers, tags or locales.
#[cfg(test)]
pub(crate) fn validate_item(source: &StructValue<'_>, path: &str) -> Result<(), ReadError> {
    validate_item_context(source, path, None)
}

pub(crate) fn validate_item_context<'source>(
    source: &StructValue<'source>,
    path: &str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<(), ReadError> {
    let view = ItemView::read(source, path, capture)?;
    match view.kind {
        ItemKind::Base => {}
        ItemKind::Corpse => {
            view.suspect()?;
        }
        ItemKind::Datapad => {
            view.canvas()?;
        }
    }
    view.check()?;
    view.entity()?;
    mission_property_source::tag_shape(view.spawn_tags()?, &format!("{path}.spawnTags"), capture)?;
    mission_property_source::visit_context(source, "clues", path, capture, |source, path| {
        if let Some(source) = source {
            validate_clue(source, path, capture)?;
        }
        Ok(())
    })?;
    Ok(())
}

fn clue<'source>(
    source: &StructValue<'source>,
    path: &str,
    texts: &Texts<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<InvestigationClue<'source>, ReadError> {
    let view = ClueView::read(source, path, capture)?;
    let details = match view.kind {
        ClueKind::Base => ClueDetails::Base,
        ClueKind::CorpseScanInfo => ClueDetails::CorpseScanInfo {
            name: mission_property_name_read::read(
                view.name()?,
                &format!("{path}.name"),
                texts,
                capture,
            )?
            .map(Box::new),
            cause_of_death: texts.localised(view.cause()?),
        },
        ClueKind::JournalEntry => ClueDetails::JournalEntry {
            journal_entry: view.journal()?.map(ReferenceShape::into_owned),
        },
    };
    view.check()?;
    Ok(InvestigationClue {
        objective_long_description: texts.localised(view.long_description()?),
        objective_short_description: texts.localised(view.short_description()?),
        details,
    })
}

fn validate_clue<'source>(
    source: &StructValue<'source>,
    path: &str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<(), ReadError> {
    let view = ClueView::read(source, path, capture)?;
    match view.kind {
        ClueKind::Base => {}
        ClueKind::CorpseScanInfo => {
            mission_property_name_read::validate_context(
                view.name()?,
                &format!("{path}.name"),
                capture,
            )?;
            view.cause()?;
        }
        ClueKind::JournalEntry => {
            view.journal()?;
        }
    }
    view.check()?;
    view.long_description()?;
    view.short_description()?;
    Ok(())
}

#[derive(Clone, Copy)]
enum ItemKind {
    Base,
    Corpse,
    Datapad,
}

struct ItemView<'capture, 'view, 'source> {
    source: &'view StructValue<'source>,
    fields: FieldChecks<'capture, 'view, 'source>,
    kind: ItemKind,
}

impl<'capture, 'view, 'source> ItemView<'capture, 'view, 'source> {
    fn read(
        source: &'view StructValue<'source>,
        path: &'view str,
        capture: Option<&'capture dyn FieldCapture<'source>>,
    ) -> Result<Self, ReadError> {
        let kind = match source.type_name() {
            "Investigation_ClueItem" => ItemKind::Base,
            "Investigation_ClueItem_Corpse" => ItemKind::Corpse,
            "Investigation_ClueItem_Datapad" => ItemKind::Datapad,
            _ => return Err(source_fields::unknown_struct(source, "clueItems", capture)),
        };
        Ok(Self {
            source,
            fields: FieldChecks::for_context(source, path, capture)?,
            kind,
        })
    }
    fn check(&self) -> Result<(), ReadError> {
        let known = match self.kind {
            ItemKind::Base => &["entityClass", "spawnTags", "clues"][..],
            ItemKind::Corpse => &["entityClass", "spawnTags", "clues", "suspectRepresented"][..],
            ItemKind::Datapad => &["entityClass", "spawnTags", "clues", "canvas"][..],
        };
        self.fields.check(known)
    }
    fn suspect(&self) -> Result<Option<&'view Value<'source>>, ReadError> {
        if let Some(value) = self.source.field("suspectRepresented") {
            owned_reference_read::check(value).map_err(|()| self.invalid_suspect(value))?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }
    fn invalid_suspect(&self, value: &Value<'source>) -> ReadError {
        self.fields.invalid(
            "suspectRepresented",
            value,
            "a nullable owner-local weak pointer",
        )
    }
    fn canvas(&self) -> Result<Option<ReferenceShape<'view, 'source>>, ReadError> {
        self.fields.reference_view("canvas")
    }
    fn entity(&self) -> Result<Option<ReferenceShape<'view, 'source>>, ReadError> {
        self.fields.reference_view("entityClass")
    }
    fn spawn_tags(&self) -> Result<Option<&'view StructValue<'source>>, ReadError> {
        self.fields.structure("spawnTags")
    }
}

#[derive(Clone, Copy)]
enum ClueKind {
    Base,
    CorpseScanInfo,
    JournalEntry,
}

struct ClueView<'capture, 'view, 'source> {
    fields: FieldChecks<'capture, 'view, 'source>,
    kind: ClueKind,
}

impl<'capture, 'view, 'source> ClueView<'capture, 'view, 'source> {
    fn read(
        source: &'view StructValue<'source>,
        path: &'view str,
        capture: Option<&'capture dyn FieldCapture<'source>>,
    ) -> Result<Self, ReadError> {
        let kind = match source.type_name() {
            "Investigation_Clue" => ClueKind::Base,
            "Investigation_Clue_CorpseScanInfo" => ClueKind::CorpseScanInfo,
            "Investigation_Clue_JournalEntry" => ClueKind::JournalEntry,
            _ => return Err(source_fields::unknown_struct(source, "clues", capture)),
        };
        Ok(Self {
            fields: FieldChecks::for_context(source, path, capture)?,
            kind,
        })
    }
    fn check(&self) -> Result<(), ReadError> {
        let known = match self.kind {
            ClueKind::Base => &["objectiveLongDescription", "objectiveShortDescription"][..],
            ClueKind::CorpseScanInfo => &[
                "objectiveLongDescription",
                "objectiveShortDescription",
                "name",
                "causeOfDeath",
            ][..],
            ClueKind::JournalEntry => &[
                "objectiveLongDescription",
                "objectiveShortDescription",
                "journalEntry",
            ][..],
        };
        self.fields.check(known)
    }
    fn name(&self) -> Result<Option<&'view StructValue<'source>>, ReadError> {
        self.fields.structure("name")
    }
    fn cause(&self) -> Result<Option<&'source str>, ReadError> {
        self.fields.locale("causeOfDeath")
    }
    fn journal(&self) -> Result<Option<ReferenceShape<'view, 'source>>, ReadError> {
        self.fields.reference_view("journalEntry")
    }
    fn long_description(&self) -> Result<Option<&'source str>, ReadError> {
        self.fields.locale("objectiveLongDescription")
    }
    fn short_description(&self) -> Result<Option<&'source str>, ReadError> {
        self.fields.locale("objectiveShortDescription")
    }
}

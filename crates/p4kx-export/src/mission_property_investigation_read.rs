// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use crate::mission_property_clue_read;
use crate::mission_property_investigation::{
    Investigation, InvestigationLocation, InvestigationSuspect,
};
use crate::mission_property_name_read;
use crate::mission_property_source;
use crate::record_read::ReadError;
use crate::source_fields::{FieldCapture, FieldChecks};
use crate::spawn_read::Reader;
use crate::texts::Texts;

fn check<'source>(
    source: &StructValue<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<(), ReadError> {
    FieldChecks::for_context(source, "", capture)?.check(&[
        "startingJournalEntries",
        "suspects",
        "locations",
    ])
}

pub(crate) fn read<'source>(
    reader: &Reader<'_, 'source>,
    source: &StructValue<'source>,
    texts: &Texts<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<Investigation<'source>, ReadError> {
    check(source, capture)?;
    Ok(Investigation {
        starting_journal_entries: FieldChecks::for_context(source, "", capture)?
            .references("startingJournalEntries")?,
        suspects: mission_property_source::list(source, "suspects", "", capture, |value, path| {
            suspect(value, path, texts, capture)
        })?,
        locations: mission_property_source::list(
            source,
            "locations",
            "",
            capture,
            |value, path| location(reader, value, path, texts, capture),
        )?,
    })
}

/// Checks the supported investigation shape without localising or constructing child lists.
#[cfg(test)]
pub(crate) fn validate(source: &StructValue<'_>) -> Result<(), ReadError> {
    validate_context(source, None)
}

/// Checks the same shape with the admitted original guard receiver.
pub(crate) fn validate_observed<'source>(
    source: &StructValue<'source>,
    capture: &dyn FieldCapture<'source>,
) -> Result<(), ReadError> {
    validate_context(source, Some(capture))
}

fn validate_context<'source>(
    source: &StructValue<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<(), ReadError> {
    check(source, capture)?;
    FieldChecks::for_context(source, "", capture)?.reference_shapes("startingJournalEntries")?;
    mission_property_source::visit_context(source, "suspects", "", capture, |source, path| {
        if let Some(source) = source {
            let view = SuspectView::read(source, path, capture)?;
            mission_property_name_read::validate_context(
                view.name()?,
                &format!("{path}.name"),
                capture,
            )?;
            view.initial()?;
            view.guilty()?;
        }
        Ok(())
    })?;
    mission_property_source::visit_context(source, "locations", "", capture, |source, path| {
        if let Some(source) = source {
            check_location(source, path, capture)?;
            mission_property_source::visit_context(
                source,
                "clueItems",
                path,
                capture,
                |source, path| {
                    if let Some(source) = source {
                        mission_property_clue_read::validate_item_context(source, path, capture)?;
                    }
                    Ok(())
                },
            )?;
        }
        Ok(())
    })?;
    Ok(())
}

fn suspect<'source>(
    source: &StructValue<'source>,
    path: &str,
    texts: &Texts<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<InvestigationSuspect<'source>, ReadError> {
    let view = SuspectView::read(source, path, capture)?;
    Ok(InvestigationSuspect {
        name: mission_property_name_read::read(
            view.name()?,
            &format!("{path}.name"),
            texts,
            capture,
        )?,
        initial_suspect: view.initial()?,
        is_guilty: view.guilty()?,
    })
}

fn check_location<'source>(
    source: &StructValue<'source>,
    path: &str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<(), ReadError> {
    FieldChecks::for_context(source, path, capture)?.check(&["clueItems"])
}

fn location<'source>(
    reader: &Reader<'_, 'source>,
    source: &StructValue<'source>,
    path: &str,
    texts: &Texts<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<InvestigationLocation<'source>, ReadError> {
    check_location(source, path, capture)?;
    Ok(InvestigationLocation {
        clue_items: mission_property_source::list(
            source,
            "clueItems",
            path,
            capture,
            |value, path| mission_property_clue_read::item(reader, value, path, texts, capture),
        )?,
    })
}

struct SuspectView<'capture, 'view, 'source> {
    fields: FieldChecks<'capture, 'view, 'source>,
}

impl<'capture, 'view, 'source> SuspectView<'capture, 'view, 'source> {
    fn read(
        source: &'view StructValue<'source>,
        path: &'view str,
        capture: Option<&'capture dyn FieldCapture<'source>>,
    ) -> Result<Self, ReadError> {
        let fields = FieldChecks::for_context(source, path, capture)?;
        fields.check(&["name", "initialSuspect", "isGuilty"])?;
        Ok(Self { fields })
    }
    fn name(&self) -> Result<Option<&'view StructValue<'source>>, ReadError> {
        self.fields.structure("name")
    }
    fn initial(&self) -> Result<Option<bool>, ReadError> {
        self.fields.boolean("initialSuspect")
    }
    fn guilty(&self) -> Result<Option<bool>, ReadError> {
        self.fields.boolean("isGuilty")
    }
}

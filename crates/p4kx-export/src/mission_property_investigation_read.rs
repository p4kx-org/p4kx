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
use crate::source_fields::Fields;
use crate::spawn_read::Reader;
use crate::texts::Texts;

pub(crate) fn read<'f>(
    reader: &Reader<'_, 'f>,
    source: &StructValue<'f>,
    texts: &Texts<'f>,
) -> Result<Investigation<'f>, ReadError> {
    mission_property_source::check(
        source,
        "",
        &["startingJournalEntries", "suspects", "locations"],
    )?;
    Ok(Investigation {
        starting_journal_entries: Fields::new(source, "").references("startingJournalEntries")?,
        suspects: mission_property_source::list(source, "suspects", "", |value, path| {
            suspect(value, path, texts)
        })?,
        locations: mission_property_source::list(source, "locations", "", |value, path| {
            location(reader, value, path, texts)
        })?,
    })
}

fn suspect<'f>(
    source: &StructValue<'f>,
    path: &str,
    texts: &Texts<'f>,
) -> Result<InvestigationSuspect<'f>, ReadError> {
    mission_property_source::check(source, path, &["name", "initialSuspect", "isGuilty"])?;
    let fields = Fields::new(source, path);
    Ok(InvestigationSuspect {
        name: mission_property_name_read::read(
            fields.structure("name")?,
            &format!("{path}.name"),
            texts,
        )?,
        initial_suspect: fields.boolean("initialSuspect")?,
        is_guilty: fields.boolean("isGuilty")?,
    })
}

fn location<'f>(
    reader: &Reader<'_, 'f>,
    source: &StructValue<'f>,
    path: &str,
    texts: &Texts<'f>,
) -> Result<InvestigationLocation<'f>, ReadError> {
    mission_property_source::check(source, path, &["clueItems"])?;
    Ok(InvestigationLocation {
        clue_items: mission_property_source::list(source, "clueItems", path, |value, path| {
            mission_property_clue_read::item(reader, value, path, texts)
        })?,
    })
}

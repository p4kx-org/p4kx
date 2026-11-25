// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::{Result, bail};
use p4kx_extraction::sound::{WemOriginField, WemOriginPlan};
use p4kx_extraction::{
    FieldOrigins, FieldPath, InputScope, Sound, SourceInput, SourceInventory,
    SourceInventorySelection, SourceLocation, SourceUseRole, SourceUses,
};
use serde::Serialize;

use super::InfoJson;

const SUMMARY_FIELDS: [(&str, WemOriginField); 8] = [
    ("/format_tag", WemOriginField::FormatTag),
    ("/channels", WemOriginField::Channels),
    ("/codec", WemOriginField::Codec),
    ("/codec_name", WemOriginField::Codec),
    ("/sample_rate", WemOriginField::SampleRate),
    ("/sample_count", WemOriginField::SampleCount),
    ("/duration_seconds", WemOriginField::Duration),
    ("/loop", WemOriginField::Loop),
];

const LOOP_FIELDS: [(&str, WemOriginField); 3] = [
    ("/loop/start", WemOriginField::LoopStart),
    ("/loop/end", WemOriginField::LoopEnd),
    ("/loop/frames", WemOriginField::LoopFrames),
];

const CONVERSION_FORMAT_FIELDS: [(&str, WemOriginField); 2] = [
    ("/channels", WemOriginField::Channels),
    ("/sample_rate", WemOriginField::SampleRate),
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Report<T, F = FieldOrigins, P = SourceInventory> {
    #[serde(flatten)]
    summary: T,
    field_origins: F,
    provenance: P,
}

pub(super) fn report<'sound>(
    sound: &Sound,
    info: InfoJson<'sound>,
) -> Result<Report<InfoJson<'sound>>> {
    capture(sound, info, datum_fields(sound))
}

pub(super) fn conversion_report<T>(sound: &Sound, summary: T) -> Result<Report<T>> {
    capture(sound, summary, CONVERSION_FORMAT_FIELDS)
}

pub(super) const fn converted_report<'evidence, T>(
    summary: T,
    field_origins: &'evidence FieldOrigins,
    provenance: SourceInventorySelection<'evidence>,
) -> Report<T, &'evidence FieldOrigins, SourceInventorySelection<'evidence>> {
    Report {
        summary,
        field_origins,
        provenance,
    }
}

fn capture<T>(
    sound: &Sound,
    summary: T,
    fields: impl IntoIterator<Item = (&'static str, WemOriginField)>,
) -> Result<Report<T>> {
    let uses = SourceUses::new();
    let field_origins = match sound.source_input() {
        Some(input) => read(sound, &uses, input, fields)?,
        None => unknown(&uses, fields)?,
    };
    Ok(Report {
        summary,
        field_origins,
        provenance: uses.freeze()?,
    })
}

fn datum_fields(sound: &Sound) -> impl Iterator<Item = (&'static str, WemOriginField)> {
    let loops = sound.wem().loop_points().is_some().then_some(LOOP_FIELDS);
    SUMMARY_FIELDS
        .into_iter()
        .chain(loops.into_iter().flatten())
}

fn read(
    sound: &Sound,
    uses: &SourceUses,
    input: SourceInput<'_>,
    datum_fields: impl IntoIterator<Item = (&'static str, WemOriginField)>,
) -> Result<FieldOrigins> {
    let name_scope = match input {
        SourceInput::Archive(_) => InputScope::ArchiveDirectoryMetadata,
        SourceInput::File(_) => InputScope::FileMetadata,
        _ => bail!("Audio summary source is outside its original read boundary"),
    };
    let payload = uses.observe(input, InputScope::Payload)?;
    let name = payload.for_scope(name_scope).at(
        SourceLocation::Metadata {
            field: "path".to_owned(),
        },
        SourceUseRole::Payload,
    )?;
    let mut fields = FieldOrigins::new();
    fields.insert(FieldPath::parse("/sound")?, uses.origin(&[name], true))?;

    let wem = sound.wem();
    let plan = WemOriginPlan::new(wem.info_source(), wem.codec(), wem.sample_count());
    for (path, field) in datum_fields {
        let inputs = plan
            .field(field)?
            .iter()
            .map(|site| site.bind(&payload))
            .collect::<Result<Vec<_>, _>>()?;
        fields.insert(FieldPath::parse(path)?, uses.origin(&inputs, true))?;
    }
    Ok(fields)
}

fn unknown(
    uses: &SourceUses,
    datum_fields: impl IntoIterator<Item = (&'static str, WemOriginField)>,
) -> Result<FieldOrigins> {
    let mut fields = FieldOrigins::new();
    for path in std::iter::once("/sound").chain(datum_fields.into_iter().map(|(path, _)| path)) {
        fields.insert(FieldPath::parse(path)?, uses.origin(&[], false))?;
    }
    Ok(fields)
}

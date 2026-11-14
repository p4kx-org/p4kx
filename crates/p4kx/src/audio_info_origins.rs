// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::{Result, bail};
use p4kx_extraction::sound::{WemOriginField, WemOriginPlan};
use p4kx_extraction::{
    FieldOrigins, FieldPath, InputScope, Sound, SourceInput, SourceInventory, SourceLocation,
    SourceUseRole, SourceUses,
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Report<'sound> {
    #[serde(flatten)]
    info: InfoJson<'sound>,
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

pub(super) fn report<'sound>(sound: &Sound, info: InfoJson<'sound>) -> Result<Report<'sound>> {
    let uses = SourceUses::new();
    let field_origins = match sound.source_input() {
        Some(input) => read(sound, &uses, input)?,
        None => unknown(sound, &uses)?,
    };
    Ok(Report {
        info,
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

fn read(sound: &Sound, uses: &SourceUses, input: SourceInput<'_>) -> Result<FieldOrigins> {
    let name_scope = match input {
        SourceInput::Archive(_) => InputScope::ArchiveDirectoryMetadata,
        SourceInput::File(_) => InputScope::FileMetadata,
        _ => bail!("Audio Info source is outside its original read boundary"),
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
    for (path, field) in datum_fields(sound) {
        let inputs = plan
            .field(field)?
            .iter()
            .map(|site| site.bind(&payload))
            .collect::<Result<Vec<_>, _>>()?;
        fields.insert(FieldPath::parse(path)?, uses.origin(&inputs, true))?;
    }
    Ok(fields)
}

fn unknown(sound: &Sound, uses: &SourceUses) -> Result<FieldOrigins> {
    let mut fields = FieldOrigins::new();
    for path in std::iter::once("/sound").chain(datum_fields(sound).map(|(path, _)| path)) {
        fields.insert(FieldPath::parse(path)?, uses.origin(&[], false))?;
    }
    Ok(fields)
}

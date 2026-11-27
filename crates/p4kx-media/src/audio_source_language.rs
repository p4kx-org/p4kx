// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::sound::wwise_path::Folder;
use p4kx_extraction::{InputScope, SourceUseRole};
use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_record_origins as origins;
use crate::origin_capture::OriginSite;
use crate::origin_contract::OriginProfile;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::{MediaError, MediaWriter, Status, Subresource};

pub(crate) const NAMESPACE: &str = "audio-source-language";
pub(crate) const RECORD_TYPE: &str = "source-language";

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(crate) enum AudioSourceLanguage<'p> {
    SourceLanguage(DirectoryLanguage<'p>),
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct DirectoryLanguage<'p> {
    language_folder: Option<&'p str>,
}

pub(crate) fn project(
    input: &MediaOriginInput,
    folder: Folder<'_>,
    profile: OriginProfile,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = origins::prepare_for(input, profile)?;
    let mut capture = writer.capture_origins();
    capture.add(
        input,
        InputScope::ArchiveDirectoryMetadata,
        OriginSite::new("archive-directory", &[], "path"),
        SourceUseRole::Conversion,
        writer,
    )?;
    origins::field(
        &mut prepared,
        "/data/payload/language_folder",
        capture.finish(),
        writer,
    )?;
    origins::indices(
        &mut prepared,
        OriginContributions::independent(&input.owner),
        writer,
    )?;
    prepared.write(
        writer,
        &Subresource::new(NAMESPACE, Vec::new()),
        Status::Complete,
        &AudioSourceLanguage::SourceLanguage(DirectoryLanguage {
            language_folder: folder.language(),
        }),
        &[],
    )
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::chunks::{ChunkFile, Container};
use p4kx_extraction::{SourceScope, SourceValue, SourcedValue, archive_animation_path};
use p4kx_model::{CafImage, CafImagePath, CafImageRecord};

use crate::player_source::{PlayerSourceError, check};

use super::input::{Inputs, Lookup};

const PATH: &str = "Data/Animations/Animations.img";

pub(super) struct Image<'a> {
    pub(super) source: &'a SourceScope,
    pub(super) chunk: usize,
    pub(super) record: usize,
    pub(super) clip: CafImageRecord<'a>,
    pub(super) path: &'a str,
    pub(super) canonical: String,
}

pub(super) fn read(
    inputs: &Inputs<'_, '_>,
    mut project: impl FnMut(Image<'_>) -> Result<(), PlayerSourceError>,
) -> Result<(SourcedValue<Lookup>, Option<SourcedValue<()>>), PlayerSourceError> {
    let entries = inputs.entries(PATH)?;
    let target = inputs.lookup(&entries)?;
    let [entry] = entries.as_slice() else {
        return Ok((target, None));
    };
    let scope = inputs.scope(entry)?;
    let member = inputs.member(entry)?;
    let Ok(bytes) = member.payload() else {
        return Ok((
            target,
            Some(inputs.unreadable(&scope, "cafImageInventory")?),
        ));
    };
    let Ok(file) = ChunkFile::parse(bytes) else {
        let state = inputs.bytes(
            &scope,
            0..bytes.len() as u64,
            "cafImageInventory",
            SourceValue::Invalid,
            false,
        )?;
        return Ok((target, Some(state)));
    };
    let complete =
        file.container() == Container::Ivo && project_file(inputs, &scope, &file, &mut project)?;
    let state = inputs.bytes(
        &scope,
        file.table_range(),
        "cafImageInventory",
        if complete {
            SourceValue::Present(())
        } else {
            SourceValue::Invalid
        },
        complete,
    )?;
    Ok((target, Some(state)))
}

fn project_file(
    inputs: &Inputs<'_, '_>,
    scope: &SourceScope,
    file: &ChunkFile<&[u8]>,
    project: &mut impl FnMut(Image<'_>) -> Result<(), PlayerSourceError>,
) -> Result<bool, PlayerSourceError> {
    let mut found = false;
    let mut complete = true;
    for (chunk, descriptor) in file.chunks().iter().enumerate() {
        check(inputs.cancel)?;
        if descriptor.chunk_type().id() != CafImage::CHUNK_TYPE {
            continue;
        }
        found = true;
        let image = file
            .chunk_data(chunk)
            .filter(|_| descriptor.version() == CafImage::CHUNK_VERSION)
            .and_then(|bytes| CafImage::read(bytes, descriptor.offset()).ok());
        let Some(image) = image else {
            complete = false;
            continue;
        };
        for (record, clip) in image.records().enumerate() {
            check(inputs.cancel)?;
            let Some((path, canonical)) = key(clip) else {
                complete = false;
                continue;
            };
            project(Image {
                source: scope,
                chunk,
                record,
                clip,
                path,
                canonical,
            })?;
        }
    }
    Ok(found && complete)
}

fn key(clip: CafImageRecord<'_>) -> Option<(&str, String)> {
    let CafImagePath::Text(path) = clip.path() else {
        return None;
    };
    let (_, extension) = path.rsplit_once('.')?;
    if !extension.eq_ignore_ascii_case("caf") {
        return None;
    }
    Some((path, archive_animation_path(path)?.to_ascii_lowercase()))
}

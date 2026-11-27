// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_assembly::{CharacterAnimationInput, CharacterAnimationRole, CharacterAnimations};
use p4kx_extraction::{
    ArchiveAnimationRead, EntryOrigin, FieldOrigin, FieldOrigins, FieldPath, InputScope,
    SourceCoordinateOmission, SourceInput, SourceUse, SourceUseRole, SourceUses,
};
use p4kx_gltf::{AnimationSummary, GlbSummary};

use crate::package_scene_provenance::SceneFields;

pub(crate) fn capture(
    animations: &CharacterAnimations<'_>,
    written: &GlbSummary,
) -> io::Result<Option<SceneFields>> {
    let Some(source) = animations.archive_animation_read() else {
        return Ok(None);
    };
    let sources = SourceUses::with_compact_use_refs();
    let mut fields = FieldOrigins::new();
    let mut inputs = animations.inputs();
    let mut root = FieldPath::parse("/meshes/0/rig/animations").map_err(io::Error::other)?;
    let mut output_index = 0;
    for clip in &written.animations {
        let input = inputs.next().ok_or_else(invalid_binding)?;
        if clip.channels == 0 {
            continue;
        }
        let output = clip_fields(source, input, clip, &sources)?;
        root.push_index(output_index).map_err(io::Error::other)?;
        for (field, origin) in output {
            root.push_field(field).map_err(io::Error::other)?;
            fields
                .insert(root.clone(), origin)
                .map_err(io::Error::other)?;
            root.pop();
        }
        root.pop();
        output_index += 1;
    }
    if inputs.next().is_some() {
        return Err(invalid_binding());
    }
    Ok(Some(SceneFields { fields, sources }))
}

fn clip_fields(
    source: &ArchiveAnimationRead,
    input: CharacterAnimationInput<'_>,
    written: &AnimationSummary,
    sources: &SourceUses,
) -> io::Result<[(&'static str, FieldOrigin); 5]> {
    let decoded = source
        .set()
        .animations
        .get(input.index())
        .ok_or_else(invalid_binding)?;
    if decoded.name != written.name {
        return Err(invalid_binding());
    }
    let clip_use = source
        .archive_source(input.index())
        .map(|entry| conversion_input(sources, entry))
        .transpose()?;
    let (name, additive, duration) = match input.role() {
        CharacterAnimationRole::Keys => {
            let original = source
                .clip_field_origins(input.index(), sources)
                .map_err(io::Error::other)?
                .ok_or_else(invalid_binding)?;
            (original.name, original.additive, original.duration_seconds)
        }
        CharacterAnimationRole::Sampled => {
            let name = source
                .clip_name_origin(input.index(), sources)
                .map_err(io::Error::other)?
                .ok_or_else(invalid_binding)?;
            (
                name,
                sources.origin(&[], true),
                sources.origin(clip_use.as_slice(), false),
            )
        }
        _ => return Err(invalid_binding()),
    };
    let duration = if written.duration_seconds.is_finite() {
        duration
    } else {
        sources.origin(&[], false)
    };
    let derived = transformed_counts(clip_use, input, sources)?;
    Ok([
        ("name", name),
        ("additive", additive),
        ("duration_seconds", duration),
        ("channels", derived.clone()),
        ("keys", derived),
    ])
}

fn transformed_counts(
    clip_use: Option<SourceUse>,
    input: CharacterAnimationInput<'_>,
    sources: &SourceUses,
) -> io::Result<FieldOrigin> {
    let rig_use = input
        .rig_read()
        .and_then(|rig| rig.entry.as_ref())
        .map(|entry| conversion_input(sources, entry))
        .transpose()?;
    let inputs: Vec<_> = clip_use.into_iter().chain(rig_use).collect();
    Ok(sources.origin(&inputs, false))
}

fn conversion_input(sources: &SourceUses, entry: &EntryOrigin) -> io::Result<SourceUse> {
    sources
        .observe(SourceInput::Archive(entry), InputScope::Payload)
        .map_err(io::Error::other)?
        .at_opaque(
            SourceUseRole::Conversion,
            SourceCoordinateOmission::Unavailable,
        )
        .map_err(io::Error::other)
}

fn invalid_binding() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "written Character animations differ from their prepared inputs",
    )
}

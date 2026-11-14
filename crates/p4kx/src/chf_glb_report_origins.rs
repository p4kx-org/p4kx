// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_extraction::{FieldOrigins, FieldPath, SourceUses};

use super::parts::Parts;
use super::{Contributors, field};

pub(super) fn fields(
    uses: &SourceUses,
    character: &Contributors,
    parts: &Parts,
    images: &Contributors,
    animations: &[Contributors],
    payload: &Contributors,
) -> Result<FieldOrigins> {
    let mut fields = FieldOrigins::new();
    let options = Contributors::default();
    let mut nodes = Contributors::default();
    nodes.extend(&parts.nodes);
    nodes.extend(&parts.bones);
    for (names, contribution) in [
        (&["/archive", "/chf", "/out", "/lod"][..], &options),
        (&["/body", "/rig", "/skeleton"][..], character),
        (&["/nodes"][..], &nodes),
        (&["/bones"][..], &parts.bones),
        (
            &[
                "/meshes",
                "/skinned_meshes",
                "/primitives",
                "/vertices",
                "/triangles",
            ][..],
            &parts.geometry,
        ),
        (
            &["/materials", "/default_material_ids"][..],
            &parts.materials,
        ),
        (&["/morph_targets"][..], &parts.morphs),
        (
            &["/textures", "/textured_materials", "/texture_pixels"][..],
            images,
        ),
        (&["/bytes"][..], payload),
    ] {
        let origin = contribution.origin(uses);
        for name in names {
            field(&mut fields, name, origin.clone())?;
        }
    }
    part_fields(&mut fields, uses, character, parts)?;
    for (index, clip) in animations.iter().enumerate() {
        field(
            &mut fields,
            &format!("/animations/{index}"),
            clip.origin(uses),
        )?;
    }
    let unknown = uses.origin(&[], false);
    for name in ["/skipped", "/texture_fallbacks", "/issues"] {
        field(&mut fields, name, unknown.clone())?;
    }
    Ok(fields)
}

pub(super) fn fallback(
    fields: &mut FieldOrigins,
    uses: &SourceUses,
    read: &p4kx_extraction::ArchiveDefaultMaterial,
) -> Result<()> {
    use p4kx_extraction::{
        ArchiveDefaultMaterialState, InputScope, SourceInput, SourceLocation, SourceUseRole,
    };

    fn metadata(uses: &SourceUses, source: &p4kx_extraction::EntryOrigin) -> Result<Contributors> {
        let scope = uses.observe(
            SourceInput::Archive(source),
            InputScope::ArchiveDirectoryMetadata,
        )?;
        let evidence = scope.at(
            SourceLocation::Metadata {
                field: "entry".into(),
            },
            SourceUseRole::Selector,
        )?;
        Ok(Contributors {
            values: vec![evidence],
            complete: true,
        })
    }

    let mut state = Contributors::default();
    if let Some(source) = read.original() {
        let origin = metadata(uses, source)?;
        field(fields, "/default_material/source", origin.origin(uses))?;
        state.extend(&origin);
        if read.decoded_sha256().is_some() {
            let mut payload = Contributors::default();
            payload.entry(uses, Some(source), SourceUseRole::Conversion)?;
            field(
                fields,
                "/default_material/decoded_sha256",
                payload.origin(uses),
            )?;
            state.extend(&payload);
        } else {
            state.incomplete();
        }
    }
    if let ArchiveDefaultMaterialState::Ambiguous { witnesses } = read.state() {
        for (index, witness) in witnesses.iter().enumerate() {
            let origin = metadata(uses, witness)?;
            field(
                fields,
                &format!("/default_material/ambiguous_sources/{index}"),
                origin.origin(uses),
            )?;
            state.extend(&origin);
        }
    }
    if read.original().is_none()
        && !matches!(read.state(), ArchiveDefaultMaterialState::Ambiguous { .. })
    {
        state.incomplete();
    }
    field(fields, "/default_material/state", state.origin(uses))?;

    Ok(())
}

fn part_fields(
    fields: &mut FieldOrigins,
    uses: &SourceUses,
    character: &Contributors,
    parts: &Parts,
) -> Result<()> {
    let options = Contributors::default();
    for (index, part) in parts.individual.iter().enumerate() {
        let mut selection = Contributors::default();
        selection.extend(character);
        selection.extend(&part.path);
        field(
            fields,
            &format!("/parts/{index}/name"),
            character.origin(uses),
        )?;
        field(
            fields,
            &format!("/parts/{index}/origin"),
            options.origin(uses),
        )?;
        field(
            fields,
            &format!("/parts/{index}/file"),
            selection.origin(uses),
        )?;
        field(
            fields,
            &format!("/parts/{index}/morph_targets"),
            part.morphs.origin(uses),
        )?;
    }
    Ok(())
}

pub(super) fn cover(
    fields: &mut FieldOrigins,
    value: &serde_json::Value,
    path: &mut FieldPath,
    inherited: &p4kx_extraction::FieldOrigin,
) -> Result<()> {
    use serde_json::Value;
    let origin = fields.get(path).unwrap_or(inherited).clone();
    match value {
        Value::Object(members) if !members.is_empty() => {
            for (name, member) in members {
                path.push_field(name)?;
                cover(fields, member, path, &origin)?;
                path.pop();
            }
        }
        Value::Array(items) if !items.is_empty() => {
            for (index, item) in items.iter().enumerate() {
                path.push_index(index)?;
                cover(fields, item, path, &origin)?;
                path.pop();
            }
        }
        _ if fields.get(path).is_none() => fields.insert(path.clone(), origin)?,
        _ => {}
    }
    Ok(())
}

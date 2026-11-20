// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;

use anyhow::Result;
use p4kx_assembly::{
    ArchiveModels, Armour, CharacterAnimations, MountModelSource, PortPlacementField,
    WornMountSource,
};
use p4kx_extraction::{
    ArchiveWriteProtection, CancelToken, ConversionError, DataForgeOrigin, FieldOrigin, FieldPath,
    GlbOptions, GlbSummary, GltfError, InputScope, MaterialMode, OriginOutputAddress,
    RiggedGlbEmission, RiggedJsonAddress, SourceInput, SourceInventory, SourceScope, SourceUse,
    SourceUseRole, SourceUses, TextureLoader, write_origin_companion, write_replacing_protected,
};

use crate::cli_armour::ArmourGlbArgs;
use crate::cli_chf::ChfMaterialsArg;
use crate::file_receipt::FileReceipt;

pub(super) fn write(
    armour: &Armour,
    animated: &CharacterAnimations<'_>,
    args: &ArmourGlbArgs,
    (models, forge): (&ArchiveModels<'_>, &DataForgeOrigin),
    (protection, cancel): (&ArchiveWriteProtection, &CancelToken),
) -> Result<GlbSummary> {
    let mode = match args.materials {
        ChfMaterialsArg::Colours => MaterialMode::Colours,
        ChfMaterialsArg::Textures => MaterialMode::Textures,
    };
    let mut options = GlbOptions::default()
        .with_lod(args.lod)
        .with_materials(mode)
        .with_texture_mip(args.texture_mip);
    if args.no_vertex_colours {
        options = options.without_vertex_colours();
    }
    let out: &Path = &args.out;
    let name = armour.variant().code().to_owned();
    let textures = models.textures();
    if armour
        .parts()
        .iter()
        .any(|part| part.mount_source().is_some())
    {
        return publish(
            out,
            protection,
            cancel,
            &Export {
                armour,
                animated,
                options: &options,
                textures: &textures,
                forge,
            },
        );
    }
    Ok(write_replacing_protected(
        out,
        Some(protection),
        Some(cancel),
        |writer| {
            armour
                .write_glb_animated_with(animated, &options, &textures, writer)
                .map_err(|error| match error {
                    GltfError::Write(source) => ConversionError::Write {
                        path: out.to_path_buf(),
                        source,
                    },
                    source => ConversionError::Glb {
                        file: name.clone(),
                        source,
                    },
                })
        },
    )?)
}

struct Export<'a> {
    armour: &'a Armour,
    animated: &'a CharacterAnimations<'a>,
    options: &'a GlbOptions,
    textures: &'a dyn TextureLoader,
    forge: &'a DataForgeOrigin,
}

fn publish(
    out: &Path,
    protection: &ArchiveWriteProtection,
    cancel: &CancelToken,
    export: &Export<'_>,
) -> Result<GlbSummary> {
    let receipt =
        FileReceipt::new_with_cancel(out, Some(protection), Some(protection), cancel.clone())?;
    let (summary, _) = receipt
        .publish(
            |mut writer| {
                let (summary, emission) = export
                    .armour
                    .write_glb_animated_observed(
                        export.animated,
                        export.options,
                        export.textures,
                        &mut writer,
                    )
                    .map_err(|error| match error {
                        GltfError::Write(source) => source,
                        source => io::Error::other(source),
                    })?;
                let origins = project(export, &emission).map_err(io::Error::other)?;
                Ok((summary, origins))
            },
            |writer, binding, (_, origins)| {
                write_origin_companion(
                    writer,
                    binding,
                    &origins.inventory,
                    origins
                        .fields
                        .iter()
                        .map(|(address, origin)| (address, origin)),
                )
            },
        )
        .map_err(|error| match error.downcast::<GltfError>() {
            Ok(source) => ConversionError::Glb {
                file: export.armour.variant().code().to_owned(),
                source,
            },
            Err(source) => ConversionError::Write {
                path: out.to_path_buf(),
                source,
            },
        })?;
    Ok(summary)
}

type Fields = Vec<(OriginOutputAddress, FieldOrigin)>;

struct Projection {
    inventory: SourceInventory,
    fields: Fields,
}

fn project(export: &Export<'_>, emission: &RiggedGlbEmission) -> Result<Projection> {
    let uses = SourceUses::new();
    let forge = uses.observe(forge_input(export.forge), InputScope::Payload)?;
    let mut fields = Vec::new();
    for &address in emission.json_addresses() {
        let RiggedJsonAddress::MountedNode {
            index,
            part,
            bone,
            child,
            trs,
        } = address
        else {
            continue;
        };
        let Some(source) = export
            .armour
            .parts()
            .get(part)
            .and_then(|part| part.mount_source())
        else {
            continue;
        };
        let origins = mount_origins(&uses, &forge, source)?;
        node_origin(&mut fields, bone, "children", Some(child), origins.parent)?;
        for (field, written) in ["translation", "rotation", "scale"].into_iter().zip(trs) {
            if written {
                node_origin(&mut fields, index, field, None, origins.pose.clone())?;
            }
        }
    }
    Ok(Projection {
        inventory: uses.freeze()?,
        fields,
    })
}

struct MountOrigins {
    parent: FieldOrigin,
    pose: FieldOrigin,
}

fn mount_origins(
    uses: &SourceUses,
    forge: &SourceScope,
    source: &WornMountSource,
) -> Result<MountOrigins> {
    let mut parent = Vec::new();
    let mut pose = Vec::new();
    for (field, location) in source.port().locations() {
        let (target, role) = match field {
            PortPlacementField::Helper => (&mut parent, SourceUseRole::Selector),
            PortPlacementField::ItemOffsetHelper => (&mut pose, SourceUseRole::Selector),
            _ => (&mut pose, SourceUseRole::Conversion),
        };
        target.push(forge.at(location, role)?);
    }
    for model in [source.holder_bone(), source.skeleton_bone()] {
        if let Some(model) = model_input(uses, model)? {
            parent.extend(model.selector);
            pose.push(model.transform);
        }
    }
    if let Some(locator) = model_input(uses, source.item_locator())? {
        pose.extend(locator.selector);
        pose.push(locator.transform);
    }
    let parent_origin = uses.origin(&parent, false);
    pose.extend(parent);
    Ok(MountOrigins {
        parent: parent_origin,
        pose: uses.origin(&pose, false),
    })
}

const fn forge_input(forge: &DataForgeOrigin) -> SourceInput<'_> {
    match forge {
        DataForgeOrigin::Archive(entry) => SourceInput::Archive(entry),
        DataForgeOrigin::File(file) => SourceInput::File(file),
        _ => SourceInput::Unknown,
    }
}

fn node_origin(
    fields: &mut Fields,
    node: usize,
    property: &str,
    child: Option<usize>,
    origin: FieldOrigin,
) -> Result<()> {
    let mut path = FieldPath::root();
    path.push_field("nodes")?;
    path.push_index(node)?;
    path.push_field(property)?;
    if let Some(child) = child {
        path.push_index(child)?;
    }
    fields.push((OriginOutputAddress::GlbJsonField { path }, origin));
    Ok(())
}

struct ModelUses {
    selector: Option<SourceUse>,
    transform: SourceUse,
}

fn model_input(uses: &SourceUses, model: Option<&MountModelSource>) -> Result<Option<ModelUses>> {
    let Some(model) = model else { return Ok(None) };
    let scope = uses.observe(SourceInput::Archive(model.origin()), InputScope::Payload)?;
    let name = model
        .name()
        .map(|location| scope.at(location, SourceUseRole::Selector))
        .transpose()?;
    let transform = scope.at(model.transform(), SourceUseRole::Conversion)?;
    Ok(Some(ModelUses {
        selector: name,
        transform,
    }))
}

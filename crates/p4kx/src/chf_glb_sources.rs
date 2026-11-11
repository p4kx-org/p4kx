// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::Path;

use anyhow::{Context as _, Result, ensure};
use p4kx_assembly::{Armour, CharacterAnimations};
use p4kx_extraction::RiggedGlbEmission;
use p4kx_extraction::{
    ArchiveAnimationRead, ArchiveTextureReads, ArchiveWriteProtection, CancelToken,
    ConversionError, DataForgeOrigin, EntryOrigin, FieldOrigin, FieldOrigins, FieldPath,
    GlbOptions, GlbSummary, GltfError, InputScope, ObservedFileRead, OriginArtifactBinding,
    OriginOutputAddress, SourceCoordinateOmission, SourceInput, SourceInventory, SourceLocation,
    SourceUse, SourceUseRole, SourceUses, write_origin_companion,
};
use serde::Serialize;

use crate::cli::ChfGlbArgs;
use crate::file_receipt::FileReceipt;

#[path = "chf_glb_part_origins.rs"]
mod parts;

#[path = "chf_glb_report_origins.rs"]
mod report;

use parts::{Parts, image_material, rig};

pub(super) struct Projection {
    payload: FieldOrigin,
    fields: FieldOrigins,
    images: Vec<ImageOrigin>,
    inventory: SourceInventory,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GlbPublication {
    origin_receipt: String,
    field_origins: FieldOrigins,
    texture_image_origins: Vec<ImageOrigin>,
    provenance: SourceInventory,
}

#[derive(Serialize)]
struct ImageOrigin {
    index: usize,
    #[serde(flatten)]
    origin: FieldOrigin,
}

impl Projection {
    pub(super) fn write(
        &self,
        writer: impl Write,
        binding: &OriginArtifactBinding,
    ) -> io::Result<()> {
        write_origin_companion(
            writer,
            binding,
            &self.inventory,
            [(&OriginOutputAddress::OpaquePayload, &self.payload)],
        )
    }

    pub(super) fn summary(self, origin_receipt: String) -> GlbPublication {
        GlbPublication {
            origin_receipt,
            field_origins: self.fields,
            texture_image_origins: self.images,
            provenance: self.inventory,
        }
    }
}

pub(super) struct Contributors {
    values: Vec<SourceUse>,
    complete: bool,
}

impl Default for Contributors {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            complete: true,
        }
    }
}

impl Contributors {
    fn extend(&mut self, other: &Self) {
        self.values.extend_from_slice(&other.values);
        self.complete &= other.complete;
    }

    const fn incomplete(&mut self) {
        self.complete = false;
    }

    fn entry(
        &mut self,
        uses: &SourceUses,
        entry: Option<&EntryOrigin>,
        role: SourceUseRole,
    ) -> Result<()> {
        let scope = uses.observe(
            entry.map_or(SourceInput::Unknown, SourceInput::Archive),
            InputScope::Payload,
        )?;
        self.complete &= entry.is_some();
        let location = if entry.is_some() {
            SourceLocation::Subresource {
                namespace: "archiveEntry".into(),
                indices: Vec::new(),
                field: "payload".into(),
            }
        } else {
            SourceLocation::Opaque {
                reason: SourceCoordinateOmission::Unavailable,
            }
        };
        self.values.push(scope.at(location, role)?);
        Ok(())
    }

    fn origin(&self, uses: &SourceUses) -> FieldOrigin {
        uses.origin(&self.values, self.complete)
    }
}

pub(super) fn project(
    input: &ObservedFileRead,
    forge: &DataForgeOrigin,
    armour: &Armour,
    (prepared, found): (&CharacterAnimations<'_>, &ArchiveAnimationRead),
    textures: &ArchiveTextureReads<'_, '_>,
    (summary, emission): (&GlbSummary, &RiggedGlbEmission),
) -> Result<Projection> {
    ensure!(
        emission.parts().len() == armour.parts().len(),
        "GLB part evidence count differs"
    );
    ensure!(
        prepared.len() == summary.animations.len(),
        "GLB animation evidence count differs"
    );
    ensure!(
        emission.image_sources().len() == summary.textures,
        "GLB texture evidence count differs"
    );
    let uses = SourceUses::new();
    let character = character(&uses, input)?;
    let parts = Parts::read(&uses, armour, emission, summary.bones > 0, forge)?;
    let images = images(&uses, textures, emission, armour, forge)?;
    let clips = animations(&uses, prepared, found, summary, &parts.bones)?;
    let mut payload = Contributors::default();
    for group in [
        &character,
        &parts.nodes,
        &parts.bones,
        &parts.geometry,
        &parts.morphs,
        &parts.materials,
        &images.all,
        &clips.written,
    ] {
        payload.extend(group);
    }
    let fields = report::fields(&uses, &character, &parts, &images.all, &clips.all, &payload)?;
    Ok(Projection {
        payload: payload.origin(&uses),
        fields,
        images: images.fields,
        inventory: uses.freeze()?,
    })
}

fn character(uses: &SourceUses, input: &ObservedFileRead) -> Result<Contributors> {
    let scope = uses.observe(
        input
            .origin()
            .map_or(SourceInput::Unknown, SourceInput::File),
        InputScope::Payload,
    )?;
    Ok(Contributors {
        values: vec![scope.at(
            SourceLocation::Subresource {
                namespace: "chf".into(),
                indices: Vec::new(),
                field: "character".into(),
            },
            SourceUseRole::Conversion,
        )?],
        complete: input.origin().is_some(),
    })
}

struct Images {
    all: Contributors,
    fields: Vec<ImageOrigin>,
}

fn images(
    uses: &SourceUses,
    textures: &ArchiveTextureReads<'_, '_>,
    emission: &RiggedGlbEmission,
    armour: &Armour,
    forge: &DataForgeOrigin,
) -> Result<Images> {
    let mut all = Contributors::default();
    let mut fields = Vec::new();
    for (index, image) in emission.image_sources().enumerate() {
        let mut found = Contributors::default();
        image_material(
            uses,
            &mut found,
            armour,
            forge,
            emission.image_material(index),
        )?;
        for &read in image {
            if read.fallback {
                found.incomplete();
            }
            match textures.source_uses(read, uses)? {
                Some(values) => found.values.extend(values),
                None => found.incomplete(),
            }
        }
        if image.is_empty() {
            found.incomplete();
        }
        fields.push(ImageOrigin {
            index,
            origin: found.origin(uses),
        });
        all.extend(&found);
    }
    Ok(Images { all, fields })
}

struct Clips {
    all: Vec<Contributors>,
    written: Contributors,
}

fn animations(
    uses: &SourceUses,
    prepared: &CharacterAnimations<'_>,
    found: &ArchiveAnimationRead,
    summary: &GlbSummary,
    bones: &Contributors,
) -> Result<Clips> {
    let mut all = Vec::new();
    let mut written = Contributors::default();
    for (index, (source, result)) in prepared
        .input_indices()
        .zip(&summary.animations)
        .enumerate()
    {
        let mut clip = Contributors::default();
        clip.entry(
            uses,
            found.archive_source(source),
            SourceUseRole::Conversion,
        )?;
        if let Some(source) = prepared.rig_read_for(index) {
            rig(uses, &mut clip, source)?;
        }
        if result.channels > 0 {
            clip.extend(bones);
            written.extend(&clip);
        }
        all.push(clip);
    }
    Ok(Clips { all, written })
}

fn field(fields: &mut FieldOrigins, path: &str, origin: FieldOrigin) -> Result<()> {
    fields.insert(FieldPath::parse(path)?, origin)?;
    Ok(())
}

pub(super) struct Export<'a, 'archive> {
    pub(super) name: &'a str,
    pub(super) armour: &'a Armour,
    pub(super) animated: &'a CharacterAnimations<'a>,
    pub(super) found: &'a ArchiveAnimationRead,
    pub(super) options: &'a GlbOptions,
    pub(super) textures: &'a ArchiveTextureReads<'a, 'archive>,
    pub(super) input: &'a ObservedFileRead,
    pub(super) forge: &'a DataForgeOrigin,
}

pub(super) fn publish(
    args: &ChfGlbArgs,
    archive: &Path,
    write_guard: &ArchiveWriteProtection,
    cancel: &CancelToken,
    export: &Export<'_, '_>,
) -> Result<(GlbSummary, Projection, String)> {
    let receipt_protection =
        ArchiveWriteProtection::for_sources(&[&args.file, archive], cancel.flag())?;
    let receipt = FileReceipt::new_with_cancel(
        &args.out,
        Some(write_guard),
        Some(&receipt_protection),
        cancel.clone(),
    )?;
    let (summary, projection) = receipt
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
                let projection = project(
                    export.input,
                    export.forge,
                    export.armour,
                    (export.animated, export.found),
                    export.textures,
                    (&summary, &emission),
                )
                .map_err(io::Error::other)?;
                Ok((summary, projection))
            },
            |writer, binding, (_, projection)| projection.write(writer, binding),
        )
        .map_err(|error| match error.downcast::<GltfError>() {
            Ok(source) => ConversionError::Glb {
                file: export.name.to_owned(),
                source,
            },
            Err(source) => ConversionError::Write {
                path: args.out.clone(),
                source,
            },
        })?;
    let name = receipt
        .path()
        .file_name()
        .context("missing GLB origin companion basename")?;
    Ok((summary, projection, crate::json::path(Path::new(name))))
}

impl GlbPublication {
    pub(super) fn cover(&mut self, report: &impl Serialize) -> Result<()> {
        let report = serde_json::to_value(report)?;
        let unknown = SourceUses::new().origin(&[], false);
        report::cover(
            &mut self.field_origins,
            &report,
            &mut FieldPath::root(),
            &unknown,
        )
    }
}

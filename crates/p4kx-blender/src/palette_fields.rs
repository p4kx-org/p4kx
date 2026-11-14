// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    FieldOrigins, FieldPath, SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses,
};
use p4kx_model::TintPalette;

use crate::PackageResult;
use crate::package_palette_provenance::error;

const SURFACES: [(&str, &str); 3] = [
    ("primary", "root.entryA"),
    ("secondary", "root.entryB"),
    ("tertiary", "root.entryC"),
];
const DECALS: [&str; 3] = ["root.decalColorR", "root.decalColorG", "root.decalColorB"];
const CHANNELS: [&str; 3] = ["r", "g", "b"];

pub(crate) fn capture(
    uses: &SourceUses,
    scope: Option<&SourceScope>,
    record: Option<u32>,
    palette: &TintPalette,
) -> PackageResult<FieldOrigins> {
    let mut writer = Fields {
        uses,
        observed: scope.zip(record),
        values: FieldOrigins::new(),
    };
    let name = if palette.name.is_empty() {
        None
    } else {
        writer.observe("root.name".to_owned(), SourceUseRole::Payload)?
    };
    writer.insert("/name", name.as_slice())?;
    for (exported, original) in SURFACES {
        writer.surface(exported, original)?;
    }
    writer.colour("/glass", "root.glassColor")?;
    if palette.decal.is_some() {
        let mut inputs = Vec::new();
        for (position, original) in DECALS.into_iter().enumerate() {
            inputs.extend(writer.colour(&format!("/decal/{position}"), original)?);
        }
        writer.insert("/decal", &inputs)?;
    }
    Ok(writer.values)
}

struct Fields<'a> {
    uses: &'a SourceUses,
    observed: Option<(&'a SourceScope, u32)>,
    values: FieldOrigins,
}

impl Fields<'_> {
    fn surface(&mut self, target: &str, original: &str) -> PackageResult<()> {
        let mut inputs =
            self.colour(&format!("/{target}/tint"), &format!("{original}.tintColor"))?;
        inputs.extend(self.colour(
            &format!("/{target}/specular"),
            &format!("{original}.specColor"),
        )?);
        let gloss = self.observe(format!("{original}.glossiness"), SourceUseRole::Conversion)?;
        self.insert(&format!("/{target}/glossiness"), gloss.as_slice())?;
        inputs.extend(gloss);
        self.insert(&format!("/{target}"), &inputs)
    }

    fn colour(&mut self, target: &str, original: &str) -> PackageResult<Vec<SourceUse>> {
        let mut inputs = Vec::new();
        for (channel, field) in CHANNELS.into_iter().enumerate() {
            let input = self.observe(format!("{original}.{field}"), SourceUseRole::Conversion)?;
            self.insert(&format!("{target}/{channel}"), input.as_slice())?;
            inputs.extend(input);
        }
        self.insert(target, &inputs)?;
        Ok(inputs)
    }

    fn observe(&self, field: String, role: SourceUseRole) -> PackageResult<Option<SourceUse>> {
        self.observed
            .map(|(scope, record_index)| {
                scope.at(
                    SourceLocation::DataForge {
                        record_index,
                        field,
                    },
                    role,
                )
            })
            .transpose()
            .map_err(error)
    }

    fn insert(&mut self, path: &str, uses: &[SourceUse]) -> PackageResult<()> {
        let path = FieldPath::parse(path).map_err(error)?;
        self.values
            .insert(path, self.uses.origin(uses, !uses.is_empty()))
            .map_err(error)
    }
}

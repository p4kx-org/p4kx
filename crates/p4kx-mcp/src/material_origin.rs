// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::material_source_info::MaterialSourceSite;
use p4kx_extraction::model::{MaterialReadError, MaterialReadField, MaterialReadObservation};

use p4kx_extraction::{
    Archive, Entry, FieldOrigin, FieldOrigins, FieldOriginsError, FieldPath, FieldPathError,
    SourceEvidenceError, SourceInventory, SourceUse,
};
use thiserror::Error;

use crate::content_origin::EntryPageSources;
use crate::material_shapes::{MaterialHeader, MaterialShape};
use crate::page::Page;

pub(crate) type MaterialObservationFailure = MaterialReadError<EvidenceError>;

#[derive(Debug, Error)]
pub(crate) enum EvidenceError {
    #[error(transparent)]
    Source(#[from] SourceEvidenceError),
    #[error(transparent)]
    Address(#[from] FieldPathError),
    #[error(transparent)]
    DuplicateField(#[from] FieldOriginsError),
    #[error("material source observation omitted {field:?}")]
    Missing { field: MaterialReadField },
    #[error("unknown material source observation role")]
    UnknownRole,
}

type Sites = BTreeMap<MaterialReadField, Vec<MaterialSourceSite>>;

#[derive(Default)]
pub(crate) struct Observations {
    library: Sites,
    rows: BTreeMap<usize, Sites>,
}

impl Observations {
    pub(crate) fn retain(
        &mut self,
        page: Page,
        observation: MaterialReadObservation<'_>,
    ) -> Result<(), EvidenceError> {
        let field = observation.field;
        if matches!(field, MaterialReadField::LayerAttribute { .. }) {
            return Ok(());
        }
        let library = matches!(
            field,
            MaterialReadField::LibrarySingle
                | MaterialReadField::LibraryTotal
                | MaterialReadField::LibraryUnknownCount
                | MaterialReadField::LibraryDefaultPalette
        );
        let sites = if library {
            &mut self.library
        } else if page.contains(observation.material) {
            self.rows.entry(observation.material).or_default()
        } else {
            return Ok(());
        };
        let site =
            MaterialSourceSite::from_observation(observation).ok_or(EvidenceError::UnknownRole)?;
        sites.entry(field).or_default().push(site);
        Ok(())
    }

    pub(crate) fn project(
        &self,
        archive: &Archive,
        entry: &Entry,
        selected: &[MaterialShape],
        header: &MaterialHeader,
        next_cursor: Option<&str>,
    ) -> Result<(FieldOrigins, SourceInventory), EvidenceError> {
        let sources = EntryPageSources::observe_with_compact_use_refs(archive, entry)?;
        let mut writer = Writer {
            sources: &sources,
            fields: FieldOrigins::new(),
        };
        let root = "/structuredContent";
        writer.put(root, "archive", sources.local())?;
        writer.put(root, "path", sources.path.clone())?;
        for (name, field) in [
            ("single", MaterialReadField::LibrarySingle),
            ("total", MaterialReadField::LibraryTotal),
            ("unknown_entries", MaterialReadField::LibraryUnknownCount),
            ("materials", MaterialReadField::LibraryTotal),
        ] {
            writer.put(root, name, writer.read(&self.library, field)?)?;
        }
        if header.default_palette.is_some() {
            writer.put(
                root,
                "default_palette",
                writer.read(&self.library, MaterialReadField::LibraryDefaultPalette)?,
            )?;
        }
        if next_cursor.is_some() {
            writer.put(
                root,
                "next_cursor",
                writer.read(&self.library, MaterialReadField::LibraryTotal)?,
            )?;
        }
        for (position, material) in selected.iter().enumerate() {
            let sites = self.rows.get(&material.id).ok_or(EvidenceError::Missing {
                field: MaterialReadField::Row,
            })?;
            writer.material(&format!("{root}/materials/{position}"), sites, material)?;
        }
        Ok((writer.fields, sources.uses.freeze()?))
    }
}

struct Writer<'a> {
    sources: &'a EntryPageSources,
    fields: FieldOrigins,
}

impl Writer<'_> {
    fn read(&self, sites: &Sites, field: MaterialReadField) -> Result<FieldOrigin, EvidenceError> {
        self.read_fields(sites, [field])
    }

    fn read_fields(
        &self,
        sites: &Sites,
        fields: impl IntoIterator<Item = MaterialReadField>,
    ) -> Result<FieldOrigin, EvidenceError> {
        let mut uses: Vec<SourceUse> = Vec::new();
        for field in fields {
            let observed = sites.get(&field).ok_or(EvidenceError::Missing { field })?;
            for site in observed {
                uses.push(
                    self.sources
                        .subresource(site.location().clone(), site.role())?,
                );
            }
        }
        Ok(self.sources.origin(&uses))
    }

    fn put(&mut self, parent: &str, field: &str, origin: FieldOrigin) -> Result<(), EvidenceError> {
        let mut address = FieldPath::parse(parent)?;
        address.push_field(field)?;
        Ok(self.fields.insert(address, origin)?)
    }

    fn material(
        &mut self,
        parent: &str,
        sites: &Sites,
        shape: &MaterialShape,
    ) -> Result<(), EvidenceError> {
        use MaterialReadField::{
            AlphaTest, Diffuse, Emissive, Flags, Glow, Layers, Name, Opacity, Palette,
            PublicParams, Row, Shader, Shininess, Specular, StringGenMask, SurfaceType, Textures,
        };
        for (field, names) in [
            (Row, &["id"][..]),
            (Name, &["name"][..]),
            (Flags, &["flags", "two_sided", "no_draw"][..]),
            (Shader, &["shader"][..]),
            (SurfaceType, &["surface_type"][..]),
            (StringGenMask, &["string_gen_mask"][..]),
            (Shininess, &["shininess"][..]),
            (Opacity, &["opacity"][..]),
            (AlphaTest, &["alpha_test"][..]),
            (Glow, &["glow"][..]),
            (Textures, &["textures", "texture_count"][..]),
            (PublicParams, &["public_params", "public_param_count"][..]),
            (Layers, &["layers", "layer_count"][..]),
        ] {
            let origin = self.read(sites, field)?;
            for name in names {
                self.put(parent, name, origin.clone())?;
            }
        }
        for (field, name, color) in [
            (Diffuse, "diffuse", &shape.diffuse),
            (Specular, "specular", &shape.specular),
            (Emissive, "emissive", &shape.emissive),
        ] {
            let origin = self.read(sites, field)?;
            self.put(parent, name, origin.clone())?;
            let path = format!("{parent}/{name}");
            for (index, _) in color.iter().enumerate() {
                self.put(&path, &index.to_string(), origin.clone())?;
            }
        }
        if shape.palette.is_some() {
            self.put(parent, "palette", self.read(sites, Palette)?)?;
        }
        self.textures(parent, sites, shape.textures.len())?;
        self.parameters(parent, sites, shape.public_params.len())?;
        self.layers(parent, sites, shape)
    }

    fn textures(&mut self, parent: &str, sites: &Sites, count: usize) -> Result<(), EvidenceError> {
        let parent = format!("{parent}/textures");
        for index in 0..count {
            let row = format!("{parent}/{index}");
            self.put(
                &row,
                "map",
                self.read_fields(
                    sites,
                    [
                        MaterialReadField::Row,
                        MaterialReadField::Texture { index },
                        MaterialReadField::TextureMap { index },
                    ],
                )?,
            )?;
            self.put(
                &row,
                "file",
                self.read_fields(
                    sites,
                    [
                        MaterialReadField::Row,
                        MaterialReadField::Texture { index },
                        MaterialReadField::TextureFile { index },
                    ],
                )?,
            )?;
        }
        Ok(())
    }

    fn parameters(
        &mut self,
        parent: &str,
        sites: &Sites,
        count: usize,
    ) -> Result<(), EvidenceError> {
        let parent = format!("{parent}/public_params");
        for index in 0..count {
            let source = self.read_fields(
                sites,
                [
                    MaterialReadField::Row,
                    MaterialReadField::PublicParam { index },
                ],
            )?;
            let row = format!("{parent}/{index}");
            self.put(&row, "name", source.clone())?;
            self.put(&row, "value", source)?;
        }
        Ok(())
    }

    fn layers(
        &mut self,
        parent: &str,
        sites: &Sites,
        shape: &MaterialShape,
    ) -> Result<(), EvidenceError> {
        let parent = format!("{parent}/layers");
        for (index, layer) in shape.layers.iter().enumerate() {
            let row = format!("{parent}/{index}");
            for (value, name, field) in [
                (&layer.name, "name", MaterialReadField::LayerName { index }),
                (&layer.path, "path", MaterialReadField::LayerPath { index }),
                (
                    &layer.palette_tint,
                    "palette_tint",
                    MaterialReadField::LayerPaletteTint { index },
                ),
            ] {
                if value.is_some() {
                    self.put(
                        &row,
                        name,
                        self.read_fields(
                            sites,
                            [
                                MaterialReadField::Row,
                                MaterialReadField::Layer { index },
                                field,
                            ],
                        )?,
                    )?;
                }
            }
        }
        Ok(())
    }
}

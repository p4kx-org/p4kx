// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod definitions;
mod timelines;

use anyhow::{Result, ensure};
use p4kx_extraction::gfx::{MovieSpan, SpanRuns, TagSite};
use p4kx_extraction::{
    FieldOrigins, FieldPath, InputScope, SourceInput, SourceInventory, SourceLocation, SourceScope,
    SourceUse, SourceUseRole, SourceUses, UiMovie,
};
use serde::Serialize;

use crate::ui_json::{AssetsJson, describe};

use SourceUseRole::{Conversion, Payload, Selector};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report<'a> {
    #[serde(flatten)]
    assets: AssetsJson<'a>,
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

pub(super) fn print(movie: &UiMovie<'_>) -> Result<bool> {
    let observed = movie.movie().assets_observed();
    let mut projection = Projection::new(movie)?;
    projection.header(movie)?;
    projection.timelines(&observed)?;
    projection.definitions(&observed)?;
    crate::json::print(&Report {
        assets: describe(movie, &observed.assets),
        field_origins: projection.fields,
        provenance: projection.uses.freeze()?,
    })?;
    Ok(observed.assets.failures.is_empty())
}

struct Projection {
    uses: SourceUses,
    source: SourceScope,
    bound: bool,
    name_scope: InputScope,
    decoded_from: Vec<SourceUse>,
    fields: FieldOrigins,
}

#[derive(Clone, Copy)]
enum Site<'a> {
    Bytes(MovieSpan),
    Header(TagSite),
    Body(TagSite),
    Item(TagSite, usize),
    Survey(MovieSpan),
    Runs(u16, &'a SpanRuns),
}

impl Site<'_> {
    fn coordinates(self, compressed: bool) -> (&'static str, Vec<u64>) {
        let domain = |direct, decoded| if compressed { decoded } else { direct };
        match self {
            Self::Bytes(span) => (
                domain("gfxBytes", "gfxDecodedBytes"),
                span_indices(span).to_vec(),
            ),
            Self::Header(tag) => (
                domain("gfxTagHeader", "gfxDecodedTagHeader"),
                vec![
                    u64::from(tag.code),
                    tag.header.offset as u64,
                    tag.header.length as u64,
                ],
            ),
            Self::Body(tag) => (domain("gfxTag", "gfxDecodedTag"), tag_indices(tag).to_vec()),
            Self::Item(tag, ordinal) => {
                let mut indices = tag_indices(tag).to_vec();
                indices.push(ordinal as u64);
                (domain("gfxTagItem", "gfxDecodedTagItem"), indices)
            }
            Self::Survey(span) => (
                domain("gfxSurvey", "gfxDecodedSurvey"),
                span_indices(span).to_vec(),
            ),
            Self::Runs(code, runs) => {
                let mut indices = vec![u64::from(code)];
                for run in runs.runs() {
                    indices.extend([
                        run.first.offset as u64,
                        run.first.length as u64,
                        run.stride as u64,
                        run.count as u64,
                    ]);
                }
                (domain("gfxTagRuns", "gfxDecodedTagRuns"), indices)
            }
        }
    }
}

fn tag_indices(tag: TagSite) -> [u64; 5] {
    [
        u64::from(tag.code),
        tag.header.offset as u64,
        tag.header.length as u64,
        tag.body.offset as u64,
        tag.body.length as u64,
    ]
}

const fn span_indices(span: MovieSpan) -> [u64; 2] {
    [span.offset as u64, span.length as u64]
}

impl Projection {
    fn new(movie: &UiMovie<'_>) -> Result<Self> {
        let uses = SourceUses::new();
        let input = movie.source_input();
        let source = uses.observe(input.unwrap_or(SourceInput::Unknown), InputScope::Payload)?;
        let mut result = Self {
            uses,
            source,
            bound: input.is_some(),
            name_scope: match input {
                Some(SourceInput::Archive(_) | SourceInput::ArchiveView(_)) => {
                    InputScope::ArchiveDirectoryMetadata
                }
                _ => InputScope::FileMetadata,
            },
            decoded_from: Vec::new(),
            fields: FieldOrigins::new(),
        };
        let read = movie.movie().read_source();
        if let Some(compressed) = read.compressed {
            result.decoded_from = vec![
                result.original(read.signature, "compressionSignature", Selector)?,
                result.original(read.declared_length, "decodedLengthLimit", Selector)?,
                result.original(compressed, "compressedPayload", Conversion)?,
            ];
        }
        Ok(result)
    }

    fn header(&mut self, movie: &UiMovie<'_>) -> Result<()> {
        let read = movie.movie().read_source();
        let name = self.source.for_scope(self.name_scope).at(
            SourceLocation::Metadata {
                field: "path".into(),
            },
            Payload,
        )?;
        self.bind("/movie", &[name], true)?;
        for (path, field, span, role) in [
            ("/signature", "signature", read.signature, Payload),
            ("/version", "version", read.version, Payload),
            ("/compression", "signature", read.signature, Conversion),
        ] {
            let original = self.original(span, field, role)?;
            self.bind(path, &[original], true)?;
        }
        let mut length = self.decoded_from.clone();
        if length.is_empty() {
            length.push(self.original(read.signature, "compressionSignature", Selector)?);
            length.push(self.original(read.declared_length, "declaredLength", Selector)?);
            length.push(self.use_at(
                "gfxMovieInput",
                "measuredByteExtent",
                vec![0, read.input_length as u64],
                Conversion,
            )?);
        }
        self.bind("/length", &length, true)?;
        self.rect("/stage", Site::Bytes(read.stage), "stage")?;
        self.logical(
            "/frame_rate",
            Site::Bytes(read.frame_rate),
            "frameRate",
            Conversion,
            true,
        )?;
        self.logical(
            "/frame_count",
            Site::Bytes(read.frame_count),
            "declaredFrameCount",
            Payload,
            true,
        )
    }

    fn bind(&mut self, path: &str, actual: &[SourceUse], complete: bool) -> Result<()> {
        self.fields.insert(
            FieldPath::parse(path)?,
            self.uses.origin(actual, complete && self.bound),
        )?;
        Ok(())
    }

    fn original(&self, span: MovieSpan, field: &str, role: SourceUseRole) -> Result<SourceUse> {
        self.use_at("gfxMovieInput", field, span_indices(span).to_vec(), role)
    }

    fn use_at(
        &self,
        namespace: &str,
        field: &str,
        indices: Vec<u64>,
        role: SourceUseRole,
    ) -> Result<SourceUse> {
        Ok(self.source.at(
            SourceLocation::Subresource {
                namespace: namespace.into(),
                indices,
                field: field.into(),
            },
            role,
        )?)
    }

    fn decoded(&self, site: Site<'_>, field: &str, role: SourceUseRole) -> Result<SourceUse> {
        let (namespace, indices) = site.coordinates(!self.decoded_from.is_empty());
        self.use_at(namespace, field, indices, role)
    }

    fn bind_decoded(
        &mut self,
        path: &str,
        mut actual: Vec<SourceUse>,
        complete: bool,
    ) -> Result<()> {
        actual.extend_from_slice(&self.decoded_from);
        self.bind(path, &actual, complete)
    }

    fn logical(
        &mut self,
        path: &str,
        site: Site<'_>,
        field: &str,
        role: SourceUseRole,
        complete: bool,
    ) -> Result<()> {
        let contributor = self.decoded(site, field, role)?;
        self.bind_decoded(path, vec![contributor], complete)
    }

    fn rect(&mut self, path: &str, site: Site<'_>, field: &str) -> Result<()> {
        self.logical(path, site, field, Payload, true)?;
        for coordinate in ["x_min", "x_max", "y_min", "y_max"] {
            self.logical(
                &format!("{path}/{coordinate}"),
                site,
                &format!("{field}.{coordinate}"),
                Payload,
                true,
            )?;
        }
        Ok(())
    }

    fn datum(
        &mut self,
        path: &str,
        site: Site<'_>,
        fields: &[(&str, SourceUseRole)],
    ) -> Result<()> {
        for &(field, role) in fields {
            self.logical(&format!("{path}/{field}"), site, field, role, true)?;
        }
        Ok(())
    }
}

fn aligned<'a, A, B>(
    values: &'a [A],
    receipts: &'a [B],
) -> Result<impl Iterator<Item = (&'a A, &'a B)>> {
    ensure!(
        values.len() == receipts.len(),
        "GFx asset rows differ from their original receipts"
    );
    Ok(values.iter().zip(receipts))
}

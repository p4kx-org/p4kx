// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_archive::ObservedEntry;
use p4kx_gfx::{AssetOrigins, CanvasSize, FailureOrigin};

use super::ui_render_frame_evidence::FrameContributions;
use crate::ui_render::ComposedFrame;
use crate::{
    FieldOrigin, InputScope, SourceInput, SourceLocation, SourceUseRole, SourceUses,
    UiMovieEvidence, UiSourceSite as Site,
};

use SourceUseRole::{Conversion, Payload};

/// The direct source facts of a rendered movie summary.
#[derive(Debug)]
#[non_exhaustive]
pub struct RenderSummaryOrigins {
    /// The stored movie Entry name or observed standalone filename.
    pub movie: FieldOrigin,
    /// The stage or explicit canvas width.
    pub width: FieldOrigin,
    /// The stage or explicit canvas height.
    pub height: FieldOrigin,
    /// An authored command option or tool-generated constant.
    pub tool_authored: FieldOrigin,
    /// An output field whose complete contributing inputs have not been bound.
    pub unknown: FieldOrigin,
    /// Directory names of the actual read import movies, in report order.
    pub import_movies: Vec<FieldOrigin>,
    /// Authored import URL and name receipts in unresolved report order.
    pub unresolved_imports: Vec<UnresolvedImportOrigins>,
    /// Tag code and logical offset receipts in failure order.
    pub tag_failures: Vec<FieldOrigin>,
}

impl RenderSummaryOrigins {
    pub(crate) fn register(
        contributions: &mut FrameContributions,
        movie: &UiMovieEvidence,
        origins: &AssetOrigins,
        frame: &ComposedFrame<'_>,
        size: CanvasSize,
        title: bool,
    ) -> io::Result<Self> {
        let name = movie.name_use().map_err(io::Error::other)?;
        let mut stage = vec![
            movie
                .decoded(Site::Bytes(movie.stage()), "stageRectangle", Conversion)
                .map_err(io::Error::other)?,
        ];
        movie.append_decoding(&mut stage);
        let registry = contributions.registry();
        let tool_authored = registry.origin(&[], true);
        let unknown = registry.origin(&[], false);
        let dimension = registry.origin(&stage, movie.is_bound());
        let movie_origin = if title {
            contributions.add(vec![name], movie.is_bound())
        } else {
            registry.origin(&[name], movie.is_bound())
        };
        let (width, height) = match size {
            CanvasSize::Stage => (dimension.clone(), dimension),
            CanvasSize::Width(_) => (tool_authored.clone(), dimension),
            CanvasSize::Fit { .. } => (tool_authored.clone(), tool_authored.clone()),
        };
        let registry = contributions.registry();
        let import_movies = frame
            .import_sources
            .movies
            .iter()
            .map(|entry| import_movie(registry, *entry))
            .collect::<io::Result<_>>()?;
        let unresolved_imports = frame
            .import_sources
            .unresolved
            .iter()
            .map(|&(tag, item)| unresolved(registry, movie, origins, tag, item))
            .collect::<io::Result<_>>()?;
        let tag_failures = origins
            .failures
            .iter()
            .map(|failure| failure_site(registry, movie, failure))
            .collect::<io::Result<_>>()?;
        Ok(Self {
            movie: movie_origin,
            width,
            height,
            tool_authored,
            unknown,
            import_movies,
            unresolved_imports,
            tag_failures,
        })
    }
}

/// The authored fields of one unresolved import.
#[derive(Debug)]
#[non_exhaustive]
pub struct UnresolvedImportOrigins {
    /// The requested import URL.
    pub url: FieldOrigin,
    /// The authored export name.
    pub name: FieldOrigin,
}

fn import_movie(uses: &SourceUses, entry: Option<ObservedEntry<'_>>) -> io::Result<FieldOrigin> {
    let input = entry.map_or(SourceInput::Unknown, SourceInput::ArchiveView);
    let name = uses
        .observe(input, InputScope::ArchiveDirectoryMetadata)
        .and_then(|scope| {
            scope.at(
                SourceLocation::Metadata {
                    field: "path".into(),
                },
                Payload,
            )
        })
        .map_err(io::Error::other)?;
    Ok(uses.origin(&[name], entry.is_some()))
}

fn unresolved(
    uses: &SourceUses,
    movie: &UiMovieEvidence,
    origins: &AssetOrigins,
    ordinal: usize,
    item: usize,
) -> io::Result<UnresolvedImportOrigins> {
    let tag = *origins.imports.get(ordinal).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "unresolved import has no observed declaration",
        )
    })?;
    Ok(UnresolvedImportOrigins {
        url: movie_field(uses, movie, Site::Body(tag), "importUrl")?,
        name: movie_field(uses, movie, Site::Item(tag, item), "importName")?,
    })
}

fn failure_site(
    uses: &SourceUses,
    movie: &UiMovieEvidence,
    failure: &FailureOrigin,
) -> io::Result<FieldOrigin> {
    let site = match failure {
        FailureOrigin::Tag(tag) => Site::Header(*tag),
        FailureOrigin::Stream { header, available } => {
            if header.length == 0 {
                Site::Survey(*available)
            } else {
                Site::Bytes(*header)
            }
        }
        _ => return Ok(uses.origin(&[], false)),
    };
    movie_field(uses, movie, site, "tagFailureLocation")
}

fn movie_field(
    uses: &SourceUses,
    movie: &UiMovieEvidence,
    site: Site<'_>,
    field: &str,
) -> io::Result<FieldOrigin> {
    let mut actual = vec![
        movie
            .decoded(site, field, Payload)
            .map_err(io::Error::other)?,
    ];
    movie.append_decoding(&mut actual);
    Ok(uses.origin(&actual, movie.is_bound()))
}

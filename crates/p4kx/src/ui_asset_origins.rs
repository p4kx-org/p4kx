// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod definitions;
mod timelines;

use anyhow::{Result, ensure};
use p4kx_extraction::{
    FieldOrigins, FieldPath, SourceInventory, SourceUse, SourceUseRole, SourceUses, UiMovie,
    UiMovieEvidence, UiSourceSite as Site,
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
    source: UiMovieEvidence,
    fields: FieldOrigins,
}

impl Projection {
    fn new(movie: &UiMovie<'_>) -> Result<Self> {
        let uses = SourceUses::new();
        let source = UiMovieEvidence::observe(movie, &uses)?;
        Ok(Self {
            uses,
            source,
            fields: FieldOrigins::new(),
        })
    }

    fn header(&mut self, movie: &UiMovie<'_>) -> Result<()> {
        let read = movie.movie().read_source();
        let name = self.source.name_use()?;
        self.bind("/movie", &[name], true)?;
        for (path, field, span, role) in [
            ("/signature", "signature", read.signature, Payload),
            ("/version", "version", read.version, Payload),
            ("/compression", "signature", read.signature, Conversion),
        ] {
            let original = self.source.original(span, field, role)?;
            self.bind(path, &[original], true)?;
        }
        let length = self.source.length_uses()?;
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
            self.uses.origin(actual, complete && self.source.is_bound()),
        )?;
        Ok(())
    }

    fn decoded(&self, site: Site<'_>, field: &str, role: SourceUseRole) -> Result<SourceUse> {
        Ok(self.source.decoded(site, field, role)?)
    }

    fn bind_decoded(
        &mut self,
        path: &str,
        mut actual: Vec<SourceUse>,
        complete: bool,
    ) -> Result<()> {
        self.source.append_decoding(&mut actual);
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

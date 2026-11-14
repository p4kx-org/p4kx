// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::model::ModelFile;
use p4kx_extraction::{
    FieldOrigin, FieldOrigins, FieldOriginsError, FieldPath, FieldPathError, ModelArchiveSource,
    ModelSource, SourceEvidenceError,
};
use serde_json::Value;
use thiserror::Error;

use crate::archives::Opened;
use crate::budget;
use crate::error::ToolError;
use crate::model_params::Section;
use crate::model_shapes::{ModelItemOrigins, ModelItemsResult};
use crate::page::Page;

#[derive(Debug, Error)]
pub(crate) enum EvidenceError {
    #[error(transparent)]
    Source(#[from] SourceEvidenceError),
    #[error(transparent)]
    Path(#[from] FieldPathError),
    #[error(transparent)]
    Duplicate(#[from] FieldOriginsError),
    #[error("model rows are unavailable at their requested positions")]
    MissingRows,
}

pub(crate) struct ModelItemPage<'a> {
    pub(crate) opened: &'a Opened,
    pub(crate) source: &'a ModelSource,
    pub(crate) section: Section,
    pub(crate) page: Page,
    pub(crate) reserved: usize,
}

impl ModelItemPage<'_> {
    pub(crate) fn primary(&self) -> Option<&ModelArchiveSource> {
        self.source
            .archive_sources()
            .iter()
            .find(|read| read.file() == ModelFile::Model)
    }

    pub(crate) fn publish(
        &self,
        total: usize,
        values: impl IntoIterator<Item = Result<Value, ToolError>>,
        origins: impl Fn(Range<usize>, Option<&str>) -> Result<ModelItemOrigins, EvidenceError>,
    ) -> Result<ModelItemsResult, ToolError> {
        self.page.check(total)?;
        let section = self.section.name();
        let (mut items, _) = budget::fill_within(self.page, total, self.reserved, values, |_| {
            format!("item of {section}")
        })?;
        let archive = self.opened.archive().path().display().to_string();
        let (keep, result) = budget::fit_prefix(
            items.len(),
            |len| {
                let next_cursor = self.page.next_cursor_after(len, total)?;
                let start = self.page.offset();
                let origins = origins(start..start + len, next_cursor.as_deref())?;
                Ok(ModelItemsResult {
                    archive: archive.clone(),
                    model: self.source.name().to_owned(),
                    section,
                    total,
                    items: items.split_at(len).0,
                    next_cursor,
                    origins: Some(origins),
                })
            },
            || format!("item {} of {section}", self.page.offset()),
        )?;
        let ModelItemsResult {
            archive,
            model,
            section,
            total,
            next_cursor,
            origins,
            ..
        } = result;
        items.truncate(keep);
        Ok(ModelItemsResult {
            archive,
            model,
            section,
            total,
            items,
            next_cursor,
            origins,
        })
    }
}

pub(crate) fn put(
    fields: &mut FieldOrigins,
    parent: &FieldPath,
    name: &str,
    origin: FieldOrigin,
) -> Result<(), EvidenceError> {
    let mut address = parent.clone();
    address.push_field(name)?;
    Ok(fields.insert(address, origin)?)
}

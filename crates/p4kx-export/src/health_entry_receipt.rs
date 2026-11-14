// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{LoadoutFileError, LoadoutFileRead};
use p4kx_extraction::{Archive, Entry, SourceUseRole, SourceValue};

use super::health_entry_frame::{EntryCapture, EntryFrame};
use super::health_entry_metadata::{self, EntryMetadata};
use super::{AuthoredEntryPath, LoadoutBodyReceipt, OriginalDatum};
use crate::build::original_input::OriginalCatalogInput;
use crate::{CatalogError, ExportAccountingError, HealthText};

#[path = "health_entry_reference.rs"]
mod reference;

#[derive(Debug, Clone)]
enum PathRecipe {
    Directory,
    Authored(AuthoredEntryPath),
    Constant,
    Loadout(LoadoutBodyReceipt),
}

/// Original Entry metadata and request recipes paired before the source operation.
#[derive(Debug, Clone)]
pub(crate) struct EntryReceipt {
    frame: Arc<EntryFrame>,
    outer: Option<EntryMetadata>,
    inner: Option<EntryMetadata>,
    path: HealthText,
    recipes: Vec<PathRecipe>,
}

/// Exact directory identity and path facts for one observed physical Archive Entry.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::schema_fields::every_field_required)]
pub(crate) struct PhysicalEntryIdentity {
    pub(crate) input_ref: p4kx_extraction::PhysicalSourceRef,
    pub(crate) entry_index: u32,
    pub(crate) source_path: p4kx_extraction::SourcedValue<String>,
    pub(crate) normalized_path: p4kx_extraction::SourcedValue<String>,
}

/// Structural navigation to one actual physical Archive Entry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::schema_fields::every_field_required)]
pub(crate) struct PhysicalEntryRef {
    pub(crate) input_ref: p4kx_extraction::PhysicalSourceRef,
    pub(crate) entry_index: u32,
}

impl EntryReceipt {
    pub(crate) fn outer(
        capture: &EntryCapture<'_>,
        archive: &Archive,
        entry: &Entry,
    ) -> Result<Self, CatalogError> {
        let origin = EntryMetadata::observe(capture, archive, entry)?;
        let path = origin.path().clone();
        Ok(Self {
            frame: capture.frame(),
            outer: Some(origin),
            inner: None,
            path,
            recipes: vec![PathRecipe::Directory],
        })
    }

    pub(crate) fn inner(
        capture: &EntryCapture<'_>,
        archive: &Archive,
        outer: &Entry,
        container: &Archive,
        inner: &Entry,
    ) -> Result<Self, CatalogError> {
        let frame = capture.frame();
        let parent = health_entry_metadata::checked_view(capture, archive, outer)?;
        let child = health_entry_metadata::checked_view(capture, container, inner)?;
        let parent_binding = parent.binding();
        let binding = child.binding();
        let position = binding
            .ancestor_count()
            .checked_sub(1)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        if !binding
            .ancestor(position)
            .is_some_and(|ancestor| ancestor.is_same_entry(&parent_binding))
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let parent = EntryMetadata::from_view(capture, parent)?;
        let child = EntryMetadata::from_view(capture, child)?;
        let path = capture.text(&format!(
            "{}!{}",
            parent.path().as_str(),
            child.path().as_str()
        ))?;
        Ok(Self {
            frame,
            outer: Some(parent),
            inner: Some(child),
            path,
            recipes: vec![PathRecipe::Directory],
        })
    }

    pub(crate) fn loadout(
        capture: &EntryCapture<'_>,
        read: &LoadoutFileRead,
    ) -> Result<Self, CatalogError> {
        let source = read.source();
        let archive = source
            .archive_input_origin()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        capture.check_archive(archive)?;
        let origin = source
            .entry_origin()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        match read.outcome() {
            Ok(_)
            | Err(LoadoutFileError::TooLarge { .. } | LoadoutFileError::Unreadable { .. }) => {}
            Err(error) => return Err(CatalogError::LoadoutSourceEvidence(error.clone())),
        }
        let origin = EntryMetadata::from_origin(capture, origin)?;
        Ok(Self {
            frame: capture.frame(),
            path: origin.path().clone(),
            outer: Some(origin),
            inner: None,
            recipes: vec![PathRecipe::Directory],
        })
    }

    pub(crate) fn of_operation(
        capture: &EntryCapture<'_>,
        body: LoadoutBodyReceipt,
    ) -> Result<Self, CatalogError> {
        body.check_frame(&capture.frame())?;
        let operation = body.original()?;
        let source = operation.source();
        if let Some(archive) = source.archive_input_origin() {
            capture.check_archive(archive)?;
        }
        let outer = source
            .entry_origin()
            .map(|origin| EntryMetadata::from_origin(capture, origin))
            .transpose()?;
        Ok(Self {
            frame: capture.frame(),
            outer,
            inner: None,
            path: body.path().clone(),
            recipes: vec![PathRecipe::Loadout(body)],
        })
    }

    pub(crate) fn fixed(
        capture: &EntryCapture<'_>,
        path: &'static str,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            frame: capture.frame(),
            outer: None,
            inner: None,
            path: capture.text(path)?,
            recipes: vec![PathRecipe::Constant],
        })
    }

    pub(crate) fn selected(path: AuthoredEntryPath) -> Self {
        Self {
            frame: path.frame(),
            outer: None,
            inner: None,
            path: path.text().clone(),
            recipes: vec![PathRecipe::Authored(path)],
        }
    }

    pub(crate) fn path_storage(&self) -> HealthText {
        self.path.clone()
    }
    pub(crate) fn entry_index(&self) -> Option<usize> {
        self.outer.as_ref().map(|origin| origin.index() as usize)
    }
    pub(crate) fn container_entry_index(&self) -> Option<usize> {
        self.inner.as_ref().map(|origin| origin.index() as usize)
    }

    pub(super) fn check_frame(&self, frame: &EntryFrame) -> Result<(), ExportAccountingError> {
        self.frame.check_frame(frame)
    }

    pub(crate) fn check_input(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<(), ExportAccountingError> {
        self.frame.check_input(input)?;
        for recipe in &self.recipes {
            match recipe {
                PathRecipe::Authored(path) => path.check_input(input)?,
                PathRecipe::Loadout(body) => body.check_input(input)?,
                PathRecipe::Directory | PathRecipe::Constant => {}
            }
        }
        Ok(())
    }

    pub(super) fn path_fact(&self) -> Result<OriginalDatum<HealthText>, CatalogError> {
        let mut inputs = Vec::new();
        let mut complete = true;
        for recipe in &self.recipes {
            let fact = match recipe {
                PathRecipe::Authored(path) => path.retain()?,
                PathRecipe::Loadout(body) => body.path_fact()?,
                PathRecipe::Directory => self.directory_path()?,
                PathRecipe::Constant => OriginalDatum::observed(
                    SourceValue::Present(self.path.clone()),
                    Vec::new(),
                    true,
                ),
            };
            let (uses, known) = fact.contributions();
            inputs.extend_from_slice(uses);
            complete &= known;
        }
        Ok(OriginalDatum::observed(
            SourceValue::Present(self.path.clone()),
            inputs,
            complete,
        ))
    }

    fn directory_path(&self) -> Result<OriginalDatum<HealthText>, CatalogError> {
        let role = if self.inner.is_some() {
            SourceUseRole::Conversion
        } else {
            SourceUseRole::Payload
        };
        let inputs = self
            .outer
            .iter()
            .chain(self.inner.iter())
            .map(|origin| origin.metadata("path", role))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(OriginalDatum::observed(
            SourceValue::Present(self.path.clone()),
            inputs,
            true,
        ))
    }

    pub(super) fn index_fact(&self, inner: bool) -> Result<OriginalDatum<usize>, CatalogError> {
        let origin = if inner {
            self.inner.as_ref()
        } else {
            self.outer.as_ref()
        };
        let Some(origin) = origin else {
            return Ok(OriginalDatum::observed(
                SourceValue::Absent,
                Vec::new(),
                true,
            ));
        };
        Ok(OriginalDatum::observed(
            SourceValue::Present(origin.index() as usize),
            vec![origin.metadata("entryIndex", SourceUseRole::Selector)?],
            true,
        ))
    }

    pub(crate) fn physical_file(
        &self,
        normalize: impl FnOnce(&str) -> String,
    ) -> Result<PhysicalEntryIdentity, CatalogError> {
        let origin = self.outer_directory()?;
        let sources = self.source_uses();
        let path = origin.path().as_str();
        let raw = origin.metadata("path", SourceUseRole::Payload)?;
        let normalized = origin.metadata("path", SourceUseRole::Conversion)?;
        Ok(PhysicalEntryIdentity {
            input_ref: origin.source_ref().clone(),
            entry_index: origin.index(),
            source_path: sources.value(SourceValue::Present(path.to_owned()), &[raw], true),
            normalized_path: sources.value(
                SourceValue::Present(normalize(path)),
                &[normalized],
                true,
            ),
        })
    }

    /// Returns the original containing Archive entry of a nested Entry read.
    pub(super) fn containing_entry(&self) -> Result<Self, ExportAccountingError> {
        if !matches!(self.recipes.as_slice(), [PathRecipe::Directory]) || self.inner.is_none() {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        let outer = self
            .outer
            .as_ref()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        Ok(Self {
            frame: Arc::clone(&self.frame),
            outer: Some(outer.clone()),
            inner: None,
            path: outer.path().clone(),
            recipes: vec![PathRecipe::Directory],
        })
    }

    pub(crate) fn physical_path(&self) -> Result<OriginalDatum<HealthText>, CatalogError> {
        self.outer_directory()?;
        self.directory_path()
    }

    fn outer_directory(&self) -> Result<&EntryMetadata, ExportAccountingError> {
        if !matches!(self.recipes.as_slice(), [PathRecipe::Directory]) || self.inner.is_some() {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        self.outer
            .as_ref()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)
    }

    pub(crate) fn source_uses(&self) -> &p4kx_extraction::SourceUses {
        self.frame.sources()
    }

    pub(crate) fn check_capture(
        &self,
        capture: &EntryCapture<'_>,
    ) -> Result<(), ExportAccountingError> {
        self.check_frame(&capture.frame())
    }

    pub(super) fn check_entry(&self, archive: &Archive, entry: &Entry) -> Result<(), CatalogError> {
        self.frame.check_archive(archive.input_origin())?;
        self.inner
            .as_ref()
            .or(self.outer.as_ref())
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?
            .check_entry(archive, entry)
    }

    pub(super) fn payload_entry(&self) -> Result<&EntryMetadata, ExportAccountingError> {
        self.inner
            .as_ref()
            .or(self.outer.as_ref())
            .ok_or(ExportAccountingError::ConflictingSourceDatum)
    }

    pub(crate) fn attempted_payload(&self) -> Result<(), CatalogError> {
        self.inner
            .as_ref()
            .or(self.outer.as_ref())
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?
            .attempted_payload()
    }

    pub(crate) fn merge(&mut self, other: &Self) -> Result<(), ExportAccountingError> {
        self.frame.check_frame(&other.frame)?;
        if self.path != other.path
            || !same_origin(self.outer.as_ref(), other.outer.as_ref())
            || !same_origin(self.inner.as_ref(), other.inner.as_ref())
        {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        self.recipes.extend(other.recipes.iter().cloned());
        Ok(())
    }
}

fn same_origin(left: Option<&EntryMetadata>, right: Option<&EntryMetadata>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.binding().is_same_entry(right.binding()),
        (None, None) => true,
        _ => false,
    }
}

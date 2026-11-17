// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[cfg(test)]
use p4kx_extraction::DataForge;
use p4kx_extraction::{CancelToken, Record, TreeBuilder};
use rayon::prelude::*;

use super::{MissionIndex, Tags, read};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_v3::tag_catalog::CapturedTags;
use crate::{CatalogError, record_read};
impl<'f> MissionIndex<'f> {
    pub(super) fn read_dependencies(
        &mut self,
        mut records: impl ExactSizeIterator<Item = Record<'f>>,
        input: Option<&OriginalCatalogInput<'_, 'f>>,
        captured: Option<&CapturedTags<'_, 'f>>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        let window_size = record_read::window_size(records.len());
        let mut window = Vec::with_capacity(window_size);
        let mut checked = Vec::new();
        while records.len() != 0 {
            window.clear();
            window.extend(records.by_ref().take(window_size));
            let tags = self.tags.tree();
            window
                .par_iter()
                .map_init(TreeBuilder::new, |trees, record| {
                    read::read(trees, record, tags, input, captured, cancel)
                })
                .collect_into_vec(&mut checked);
            #[expect(
                clippy::iter_with_drain,
                reason = "reuse the indexed Rayon result buffer across Record windows"
            )]
            for result in checked.drain(..) {
                if let Some(read) = result? {
                    self.ledger.merge(read.ledger);
                    if let Some(piece) = read.piece {
                        self.insert(piece);
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn read_cached_records<'tree>(
        input: &OriginalCatalogInput<'_, 'f>,
        captured: &CapturedTags<'_, 'f>,
        dependencies: crate::catalog_v3::tag_catalog::TagDependencies<'f>,
        records: impl Iterator<Item = (Record<'f>, read::CachedTree<'tree, 'f>)>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError>
    where
        'f: 'tree,
    {
        if !std::ptr::eq(dependencies.tree.tree(), captured.tree()) {
            return Err(crate::ExportAccountingError::ConflictingTagTrace.into());
        }
        let mut index = Self {
            tags: Tags::Captured(dependencies.tree),
            failures: dependencies.failures,
            ledger: dependencies.ledger,
            template_source: Some(super::TemplateReadSource::new(input.origins())),
            ..Self::default()
        };
        for (record, tree) in records {
            index.extend_cached_record(input, captured, record, tree, cancel)?;
        }
        Ok(index)
    }

    pub(crate) fn extend_cached_record(
        &mut self,
        input: &OriginalCatalogInput<'_, 'f>,
        captured: &CapturedTags<'_, 'f>,
        record: Record<'f>,
        tree: read::CachedTree<'_, 'f>,
        cancel: Option<&CancelToken>,
    ) -> Result<bool, CatalogError> {
        let found = read::read_cached(
            &mut TreeBuilder::new(),
            &record,
            self.tags.tree(),
            Some(input),
            Some(captured),
            cancel,
            Some(tree),
        )?;
        let Some(found) = found else {
            return Ok(false);
        };
        let failed = matches!(
            &found.piece,
            Some(
                super::Piece::Failed(_)
                    | super::Piece::FailedTemplate(_, _)
                    | super::Piece::UnusableRewardCandidate(_, _)
            )
        );
        self.ledger.merge(found.ledger);
        if let Some(piece) = found.piece {
            self.insert(piece);
        }
        Ok(failed)
    }

    #[cfg(test)]
    pub(crate) fn read_candidate_records(
        forge: &'f DataForge<'f>,
        records: impl ExactSizeIterator<Item = Record<'f>>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let (tags, failures, ledger) = read::read_tags(forge, None, cancel)?;
        Self::from_tags(
            records,
            None,
            cancel,
            Tags::Owned(tags),
            failures,
            ledger,
            None,
        )
    }
}

#[cfg(test)]
#[path = "mission_dependency_records_tests.rs"]
mod tests;

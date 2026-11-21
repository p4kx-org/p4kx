// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::sync::Arc;

use p4kx_channels::PublicPatches;
use thiserror::Error;

use crate::compare::{JoinResult, check_levels, join};
use crate::comparison::RecordSources;
use crate::record_compare::{compare_record, compare_records};
use crate::{
    Archive, CancelToken, CompareOptions, Comparison, ComparisonError, ComparisonProgress,
    DataForgeEntry, DataForgeError, DataForges, EntryChange, Filter, IncomparableEntry, Levels,
    RecordChange, RecordKey, SchemaDiff, Side, compare, compare_schema,
    compare_with_dataforge_entries,
};

/// The DataForge Entries of the old and the new Archive, owned.
#[derive(Debug, Clone)]
pub struct OwnedDataForges {
    /// The DataForge Entry of the old Archive.
    pub old: Arc<DataForgeEntry>,
    /// The DataForge Entry of the new Archive.
    pub new: Arc<DataForgeEntry>,
}

/// Both DataForges of an [`OwnedComparison`], opened for comparing single Records again.
#[derive(Debug, Clone, Copy)]
pub struct RecordLookup<'a> {
    forges: DataForges<'a>,
}

impl<'a> RecordLookup<'a> {
    /// Compares the Records at `key` again, as the Record level did; `None` when they do not
    /// differ, `key` matches no Record or its two Records have different GUIDs.
    #[must_use]
    pub fn change(&self, key: RecordKey) -> Option<RecordChange<'a>> {
        compare_record(self.forges, key)
    }
}

/// Why the full Comparison of an [`OwnedComparison`] could not be made.
#[derive(Debug, Error)]
pub enum RecompareError {
    /// The Comparison itself failed.
    #[error(transparent)]
    Comparison(#[from] ComparisonError),
    /// A DataForge Entry could not be opened.
    #[error(transparent)]
    DataForge(#[from] DataForgeError),
}

/// A Comparison that owns both Archives and their DataForge Entries, so it can be kept, shared
/// and sent across threads.
///
/// It holds the Entry changes as positions into both Archives and lends them out as
/// [`EntryChange`]s; the Record level is compared again on demand by [`Self::recompare`].
#[derive(Debug, Clone)]
pub struct OwnedComparison {
    old: Arc<Archive>,
    new: Arc<Archive>,
    dataforges: Option<OwnedDataForges>,
    levels: Levels,
    filter: Option<String>,
    joined: JoinResult,
    public_patches: Option<PublicPatches>,
}

impl OwnedComparison {
    /// Compares the Entries of the `old` and the `new` Archive like [`compare`] and keeps the
    /// `dataforges` for the Record level.
    ///
    /// # Errors
    ///
    /// Returns [`ComparisonError::NoLevel`] when no level is selected,
    /// [`ComparisonError::NoDataForge`] when the Record level is selected without `dataforges`
    /// and [`ComparisonError::Cancelled`] when the Comparison was cancelled.
    pub fn compare(
        old: Arc<Archive>,
        new: Arc<Archive>,
        dataforges: Option<OwnedDataForges>,
        options: &CompareOptions<'_>,
    ) -> Result<Self, ComparisonError> {
        check_levels(options, dataforges.is_some())?;
        let joined = join(&old, &new, options)?;
        Ok(Self {
            old,
            new,
            dataforges: dataforges.filter(|_| options.levels.records),
            levels: options.levels,
            filter: options.filter.map(|filter| filter.as_str().to_owned()),
            joined,
            public_patches: options.public_patches.cloned(),
        })
    }

    /// Returns the old Archive.
    #[must_use]
    pub const fn old_archive(&self) -> &Arc<Archive> {
        &self.old
    }

    /// Returns the new Archive.
    #[must_use]
    pub const fn new_archive(&self) -> &Arc<Archive> {
        &self.new
    }

    /// Returns the old side.
    #[must_use]
    pub fn old_side(&self) -> Side<'_> {
        Side::of(&self.old).with_public_patches(self.public_patches.as_ref())
    }

    /// Returns the new side.
    #[must_use]
    pub fn new_side(&self) -> Side<'_> {
        Side::of(&self.new).with_public_patches(self.public_patches.as_ref())
    }

    /// Returns the levels that were compared.
    #[must_use]
    pub const fn levels(&self) -> Levels {
        self.levels
    }

    /// Returns the number of added, removed and changed Entries.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.joined.changes.len()
    }

    /// Returns whether no Entry was added, removed or changed.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.joined.changes.is_empty()
    }

    /// Returns the Entry change at `index` in normalised path order, `None` past the end.
    #[must_use]
    pub fn entry_change(&self, index: usize) -> Option<EntryChange<'_>> {
        self.joined
            .changes
            .get(index)?
            .resolve(&self.old, &self.new)
    }

    /// Returns matched Entries with different checksum algorithms, in normalised path order.
    pub fn incomparable_entries(&self) -> impl Iterator<Item = IncomparableEntry<'_>> {
        self.joined
            .incomparables
            .iter()
            .filter_map(|joined| joined.resolve_incomparable(&self.old, &self.new))
    }

    /// Returns the Entry level as a Comparison, for its counts; it holds no Record changes.
    #[must_use]
    pub fn comparison(&self) -> Comparison<'_> {
        Comparison {
            entries: self
                .joined
                .changes
                .iter()
                .filter_map(|joined| joined.resolve(&self.old, &self.new))
                .collect(),
            incomparables: self.incomparable_entries().collect(),
            ..self.without_changes()
        }
    }

    /// Returns the Entry level of only the changes whose position is set in `selected`, for the
    /// counts of a selection already made; positions past its end are left out.
    #[must_use]
    pub fn comparison_of(&self, selected: &[bool]) -> Comparison<'_> {
        Comparison {
            entries: self
                .joined
                .changes
                .iter()
                .zip(selected)
                .filter(|(_, keep)| **keep)
                .filter_map(|(joined, _)| joined.resolve(&self.old, &self.new))
                .collect(),
            ..self.without_changes()
        }
    }

    /// Compares both Archives again on the levels of this Comparison, narrowed to `filter`, and
    /// hands the whole Comparison, the Record level included, to `use_it`; it is what
    /// [`compare_with_dataforge_entries`] gives with these settings.
    ///
    /// # Errors
    ///
    /// Returns [`RecompareError::DataForge`] when a DataForge cannot be opened and
    /// [`RecompareError::Comparison`] when the Comparison fails or is cancelled.
    pub fn recompare<R>(
        &self,
        filter: Option<&Filter>,
        cancel: Option<&CancelToken>,
        use_it: impl FnOnce(&Comparison<'_>) -> R,
    ) -> Result<R, RecompareError> {
        let forges = match &self.dataforges {
            Some(entries) => Some((entries.old.open()?, entries.new.open()?)),
            None => None,
        };
        let options = CompareOptions {
            levels: self.levels,
            filter,
            cancel,
            progress: None,
            public_patches: self.public_patches.as_ref(),
        };
        let comparison = match (&self.dataforges, &forges) {
            (Some(entries), Some((old, new))) => compare_with_dataforge_entries(
                &self.old,
                &self.new,
                DataForges { old, new },
                [&entries.old, &entries.new],
                &options,
            ),
            _ => compare(&self.old, &self.new, None, &options),
        }?;
        Ok(use_it(&comparison))
    }

    /// Compares the Records of both DataForges, narrowed to `filter`, and hands the changes to
    /// `use_it`; they are those [`compare`] gives on the Record level, in the same order.
    ///
    /// # Errors
    ///
    /// Returns [`RecompareError::Comparison`] with [`ComparisonError::NoDataForge`] when the
    /// Record level was not compared and with [`ComparisonError::Cancelled`] once `cancel` is
    /// set, and [`RecompareError::DataForge`] when a DataForge cannot be opened.
    pub fn record_changes<R>(
        &self,
        filter: Option<&Filter>,
        cancel: Option<&CancelToken>,
        progress: Option<&(dyn Fn(ComparisonProgress) + Sync)>,
        use_it: impl FnOnce(&[RecordChange<'_>]) -> R,
    ) -> Result<R, RecompareError> {
        let entries = self
            .dataforges
            .as_ref()
            .ok_or(ComparisonError::NoDataForge)?;
        let (old, new) = (entries.old.open()?, entries.new.open()?);
        let options = CompareOptions {
            levels: Levels::RECORDS,
            filter,
            cancel,
            progress,
            public_patches: None,
        };
        let forges = DataForges {
            old: &old,
            new: &new,
        };
        let changes = compare_records(
            forges,
            &options,
            ComparisonProgress::default(),
            &RecordSources::default(),
        )?;
        Ok(use_it(&changes))
    }

    /// Opens both DataForges once and hands them to `use_it` for comparing single Records again
    /// by their [`RecordKey`].
    ///
    /// # Errors
    ///
    /// Returns [`RecompareError::Comparison`] with [`ComparisonError::NoDataForge`] when the
    /// Record level was not compared and [`RecompareError::DataForge`] when a DataForge cannot be
    /// opened.
    pub fn with_records<R>(
        &self,
        use_it: impl FnOnce(&RecordLookup<'_>) -> R,
    ) -> Result<R, RecompareError> {
        let entries = self
            .dataforges
            .as_ref()
            .ok_or(ComparisonError::NoDataForge)?;
        let (old, new) = (entries.old.open()?, entries.new.open()?);
        let lookup = RecordLookup {
            forges: DataForges {
                old: &old,
                new: &new,
            },
        };
        Ok(use_it(&lookup))
    }

    /// Compares the schema of both DataForges, like [`compare_schema`].
    ///
    /// # Errors
    ///
    /// Returns [`RecompareError::Comparison`] with [`ComparisonError::NoDataForge`] when the
    /// Record level was not compared and [`RecompareError::DataForge`] when a DataForge cannot be
    /// opened.
    pub fn schema_diff(&self) -> Result<SchemaDiff, RecompareError> {
        let entries = self
            .dataforges
            .as_ref()
            .ok_or(ComparisonError::NoDataForge)?;
        let (old, new) = (entries.old.open()?, entries.new.open()?);
        Ok(compare_schema(&old, &new))
    }

    fn without_changes(&self) -> Comparison<'_> {
        Comparison {
            old: self.old_side(),
            new: self.new_side(),
            levels: self.levels,
            filter: self.filter.clone(),
            entries: Vec::new(),
            incomparables: Vec::new(),
            entry_collisions: Cow::Borrowed(&self.joined.collisions),
            records: Vec::new(),
            record_sources: RecordSources::default(),
        }
    }
}

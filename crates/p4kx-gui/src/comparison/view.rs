// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use p4kx_comparison::{CancelToken, EntryChange, Filter, OwnedComparison};

use super::ComparisonSide;
use super::diff::EntryDiff;
use super::filter_job::{self, FilterJob, Matched, Polled, Started};
use super::orders::{ChangeColumn, ChangeOrders, ChangeSort};
use super::record_list::RecordChangeList;
use super::record_view::{ComparisonList, RecordChangeView};
use super::report::{self, ReportJob, ReportStatus};
use super::schema_view::SchemaView;
use super::selection::{ChangeSelection, selected_len};
use super::summary::Summary;
use crate::SortDirection;

static GENERATIONS: AtomicU64 = AtomicU64::new(0);

/// A finished Comparison as the tab shows it: filtered, sorted and with the report it saves.
#[derive(Debug)]
pub struct ComparisonView {
    generation: u64,
    revision: u64,
    comparison: Arc<OwnedComparison>,
    orders: ChangeOrders,
    totals: Summary,
    filter_text: String,
    matched: Option<Matched>,
    pending: Option<FilterJob>,
    sort: ChangeSort,
    shown: Vec<u32>,
    selected: Option<u32>,
    diff: Option<EntryDiff>,
    reveal: bool,
    report: Option<ReportJob>,
    records: Option<RecordChangeView>,
    schema: Option<Result<SchemaView, String>>,
    list: ComparisonList,
}

impl ComparisonView {
    pub(crate) fn output_owner(&self) -> crate::output_target::Witness {
        crate::output_target::Witness::of(&self.comparison).at(self.generation, self.revision)
    }

    /// Prepares `comparison` and its Record changes `records` for display, or returns `None`
    /// once `cancel` is set; this sorts every change several times and belongs on a background
    /// thread.
    pub(crate) fn new(
        comparison: Arc<OwnedComparison>,
        (records, schema): (Option<RecordChangeList>, Option<Result<SchemaView, String>>),
        cancel: &CancelToken,
    ) -> Option<Self> {
        let orders = ChangeOrders::build(&comparison);
        if cancel.is_cancelled() {
            return None;
        }
        let totals = Summary::of(&comparison.comparison());
        let list = if comparison.levels().entries || records.is_none() {
            ComparisonList::Entries
        } else {
            ComparisonList::Records
        };
        let mut view = Self {
            records: records.map(RecordChangeView::new),
            schema,
            list,
            generation: GENERATIONS.fetch_add(1, Ordering::Relaxed),
            revision: 0,
            comparison,
            orders,
            totals,
            filter_text: String::new(),
            matched: None,
            pending: None,
            sort: ChangeSort::default(),
            shown: Vec::new(),
            selected: None,
            diff: None,
            reveal: false,
            report: None,
        };
        view.refresh_shown();
        Some(view)
    }

    /// Returns a number no other view of this process carries.
    pub(crate) const fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns a number that grows whenever the summary changes.
    pub(crate) const fn revision(&self) -> u64 {
        self.revision
    }

    /// Returns the Comparison.
    #[must_use]
    pub fn comparison(&self) -> &OwnedComparison {
        &self.comparison
    }

    /// Returns the change with id `id`, its position in path order.
    #[must_use]
    pub fn change(&self, id: u32) -> Option<EntryChange<'_>> {
        self.comparison.entry_change(id as usize)
    }

    /// Returns the counts of the changes the Filter selects, of every change without one.
    #[must_use]
    pub fn summary(&self) -> &Summary {
        self.matched
            .as_ref()
            .map_or(&self.totals, |matched| &matched.summary)
    }

    /// Returns the text of the Filter.
    #[must_use]
    pub fn filter_text(&self) -> &str {
        &self.filter_text
    }

    /// Returns whether a Filter is applied in the background.
    #[must_use]
    pub const fn is_filtering(&self) -> bool {
        self.pending.is_some()
    }

    /// Filters the list by `text` with the rules of the search on a background thread, calling
    /// `wake` from it once the result is ready; empty text shows every change at once.
    pub fn set_filter(&mut self, text: &str, wake: impl Fn() + Clone + Send + 'static) {
        text.clone_into(&mut self.filter_text);
        self.pending = None;
        if text.is_empty() {
            if let Some(records) = &mut self.records {
                records.set_filter(None, wake);
            }
            self.matched = None;
            self.revision = self.revision.wrapping_add(1);
            self.refresh_shown();
            return;
        }
        let Ok(filter) = Filter::parse(text) else {
            if let Some(records) = &mut self.records {
                records.show_none();
            }
            self.show_matched(filter_job::none(&self.comparison));
            return;
        };
        if let Some(records) = &mut self.records {
            records.set_filter(Some(filter.clone()), wake.clone());
        }
        match filter_job::start(&self.comparison, &filter, wake) {
            Started::Running(job) => self.pending = Some(job),
            Started::Finished(matched) => self.show_matched(matched),
        }
    }

    /// Returns the number of added and changed Entries of `side` the Filter selects, the ones
    /// extracting the changes of that side writes; none while a Filter is being applied.
    #[must_use]
    pub fn change_count(&self, side: ComparisonSide) -> usize {
        if self.is_filtering() {
            return 0;
        }
        selected_len(side, self.summary().counts)
    }

    /// Returns the added and changed Entries of `side` the Filter selects, or `None` while a
    /// Filter is being applied.
    pub(crate) fn selection(&self, side: ComparisonSide) -> Option<ChangeSelection> {
        (!self.is_filtering()).then(|| {
            ChangeSelection::new(
                Arc::clone(&self.comparison),
                self.matched
                    .as_ref()
                    .map(|matched| Arc::clone(&matched.mask)),
                side,
                self.summary().counts,
            )
        })
    }

    /// Returns how the list is sorted.
    #[must_use]
    pub const fn sort(&self) -> ChangeSort {
        self.sort
    }

    /// Sorts by `column`, reversing the direction when it is already the sort column.
    pub fn sort_by(&mut self, column: ChangeColumn) {
        self.sort = self.sort.clicked(column);
        self.refresh_shown();
    }

    /// Returns the ids of the shown changes in list order.
    #[must_use]
    pub fn shown(&self) -> &[u32] {
        &self.shown
    }

    /// Returns the selected change, or `None` when none is.
    #[must_use]
    pub const fn selected(&self) -> Option<u32> {
        self.selected
    }

    /// Selects the change `id`, dropping the line diff of another change.
    pub fn select(&mut self, id: u32) {
        self.reveal |= self.selected != Some(id);
        self.selected = Some(id);
        if self.diff.as_ref().is_some_and(|diff| diff.change() != id) {
            self.diff = None;
        }
    }

    /// Starts the line diff of the selected change on a background thread unless it is made
    /// already, calling `wake` from it once it ended; a diff still being made for another change
    /// is cancelled.
    pub fn load_diff(&mut self, wake: impl Fn() + Send + 'static) {
        let Some(id) = self.selected else {
            self.diff = None;
            return;
        };
        if self.diff.as_ref().is_some_and(|diff| diff.change() == id) {
            return;
        }
        self.diff = None;
        self.diff = self
            .change(id)
            .map(|change| change.path().to_owned())
            .map(|path| EntryDiff::start(&self.comparison, (id, &path), wake));
    }

    /// Returns the position of the selected change in the list once after each new selection,
    /// for the list to scroll it into view.
    pub fn take_reveal(&mut self) -> Option<usize> {
        let id = self.selected.filter(|_| std::mem::take(&mut self.reveal))?;
        self.shown.iter().position(|&shown| shown == id)
    }

    /// Returns the line diff of the selected change, `None` before [`Self::load_diff`].
    #[must_use]
    pub const fn diff(&self) -> Option<&EntryDiff> {
        self.diff.as_ref()
    }

    /// Returns the line diff of the selected change for navigation.
    pub const fn diff_mut(&mut self) -> Option<&mut EntryDiff> {
        self.diff.as_mut()
    }

    /// Writes the JSON report of the shown changes to `path` on a background thread, the same
    /// report `p4kx compare --report` writes with the current Filter; `wake` is called from that
    /// thread once it ended. Nothing starts while a report is being saved or while the Filter text
    /// is refused.
    pub fn save_report(&mut self, path: PathBuf, wake: impl Fn() + Send + 'static) {
        if self.is_saving() {
            return;
        }
        let filter = if self.filter_text.is_empty() {
            None
        } else if let Ok(filter) = Filter::parse(&self.filter_text) {
            Some(filter)
        } else {
            return;
        };
        self.report = Some(report::start(&self.comparison, filter, path, wake));
    }

    /// Returns where saving the report stands, or `None` when none was saved.
    #[must_use]
    pub fn report_status(&self) -> Option<&ReportStatus> {
        self.report.as_ref().map(ReportJob::status)
    }

    /// Returns whether a report is being saved.
    #[must_use]
    pub fn is_saving(&self) -> bool {
        matches!(self.report_status(), Some(ReportStatus::Saving(_)))
    }

    /// Takes the results of a finished filter and report; returns whether one finished.
    pub(crate) fn poll(&mut self) -> bool {
        let saved = self.report.as_mut().is_some_and(ReportJob::poll)
            | self.diff.as_mut().is_some_and(EntryDiff::poll)
            | self.records.as_mut().is_some_and(RecordChangeView::poll);
        let Some(job) = &self.pending else {
            return saved;
        };
        match job.poll() {
            Polled::Running => saved,
            Polled::Finished(matched) => {
                self.pending = None;
                self.show_matched(matched);
                true
            }
            Polled::Lost => {
                self.pending = None;
                self.filter_text.clear();
                self.matched = None;
                self.revision = self.revision.wrapping_add(1);
                self.refresh_shown();
                true
            }
        }
    }

    /// Returns the Record changes, `None` when the Record level was not compared.
    #[must_use]
    pub const fn records(&self) -> Option<&RecordChangeView> {
        self.records.as_ref()
    }

    /// Returns the Record changes to change them, `None` when the Record level was not compared.
    pub const fn records_mut(&mut self) -> Option<&mut RecordChangeView> {
        self.records.as_mut()
    }

    /// Returns the list shown.
    #[must_use]
    pub const fn list(&self) -> ComparisonList {
        self.list
    }

    /// Shows `list`; the Record and the schema list only when the Record level was compared.
    pub const fn show_list(&mut self, list: ComparisonList) {
        let available = match list {
            ComparisonList::Entries => true,
            ComparisonList::Records => self.records.is_some(),
            ComparisonList::Schema => self.schema.is_some(),
        };
        if available {
            self.list = list;
        }
    }

    /// Returns the schema drift between both DataForges, the reason it could not be compared, or
    /// `None` when the Record level was not compared.
    #[must_use]
    pub fn schema(&self) -> Option<Result<&SchemaView, &str>> {
        self.schema
            .as_ref()
            .map(|schema| schema.as_ref().map_err(String::as_str))
    }

    /// Returns the number of rows the shown list holds.
    #[must_use]
    pub fn shown_len(&self) -> usize {
        match (self.list, &self.records, &self.schema) {
            (ComparisonList::Records, Some(records), _) => records.shown().len(),
            (ComparisonList::Schema, _, Some(Ok(schema))) => schema.rows().len(),
            _ => self.shown.len(),
        }
    }

    /// Starts comparing the fields of the selected Record on a background thread unless that is
    /// done already, calling `wake` from it once it ended.
    pub fn load_record_detail(&mut self, wake: impl Fn() + Send + 'static) {
        if let Some(records) = &mut self.records {
            records.load_detail(&self.comparison, wake);
        }
    }

    fn show_matched(&mut self, matched: Matched) {
        self.matched = Some(matched);
        self.revision = self.revision.wrapping_add(1);
        self.refresh_shown();
    }

    fn refresh_shown(&mut self) {
        let ascending = self.orders.ascending(self.sort.column).iter().copied();
        let mask = self.matched.as_ref().map(|matched| &*matched.mask);
        let keep =
            |id: &u32| mask.is_none_or(|mask| mask.get(*id as usize).copied().unwrap_or(false));
        self.shown = match self.sort.direction {
            SortDirection::Ascending => ascending.filter(keep).collect(),
            SortDirection::Descending => ascending.rev().filter(keep).collect(),
        };
    }
}

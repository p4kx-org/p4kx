// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::vec;

use p4kx_extraction::Exclusion;
use p4kx_testkit::TestEntry;

use crate::required_input_fault_support::{self as fixture, Inventory};
use crate::{
    MediaError, MediaKind, MediaWriter, OriginalIdentifier, ReferenceCandidate, ReferenceTarget,
    SourceContext, Subresource,
};

pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub(super) fn with_writer(
    run: impl FnOnce(&SourceContext<'_>, &mut MediaWriter<'_>) -> TestResult,
) -> TestResult {
    fixture::with_fixture(
        [TestEntry::stored("owner.dds", b"synthetic owner")],
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, _, _| run(context, writer),
    )
}

pub(super) fn original() -> Result<OriginalIdentifier, MediaError> {
    OriginalIdentifier::try_new_with_text("synthetic.reference-admission", "original")
}

pub(super) fn candidate(context: &SourceContext<'_>, ordinal: u64) -> ReferenceCandidate {
    ReferenceCandidate::new(
        ReferenceTarget::new_with_subresource(
            context.source,
            MediaKind::Texture,
            Subresource::new("synthetic.reference-admission", vec![ordinal]),
        ),
        crate::CandidateState::Selected,
    )
}

pub(super) fn bytes<T: serde::Serialize>(value: &T) -> TestResult<u64> {
    let length = serde_json::to_vec(value)?.len();
    u64::try_from(length).map_err(Into::into)
}

#[derive(Default)]
pub(super) struct Counts {
    pub(super) initialized: Cell<usize>,
    pub(super) advanced: Cell<usize>,
}

pub(super) struct Probe<'counts, T> {
    items: Vec<T>,
    counts: &'counts Counts,
}

impl<'counts, T> Probe<'counts, T> {
    pub(super) const fn new(items: Vec<T>, counts: &'counts Counts) -> Self {
        Self { items, counts }
    }
}

impl<'counts, T> IntoIterator for Probe<'counts, T> {
    type Item = T;
    type IntoIter = ProbeIter<'counts, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.counts
            .initialized
            .set(self.counts.initialized.get() + 1);
        ProbeIter {
            items: self.items.into_iter(),
            counts: self.counts,
        }
    }
}

pub(super) struct ProbeIter<'counts, T> {
    items: vec::IntoIter<T>,
    counts: &'counts Counts,
}

impl<T> Iterator for ProbeIter<'_, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.counts.advanced.set(self.counts.advanced.get() + 1);
        self.items.next()
    }
}

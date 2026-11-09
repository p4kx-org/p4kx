// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Entry};

use crate::directory_lookup::DirectoryLookup;
use crate::input_budget::InputBudget;
use crate::required_input;
use crate::source_selection::EntrySelection;
use crate::traversal_visit::TraversalVisit;
use crate::{MediaError, MediaWriter, Provider, SourceContext};

pub(crate) struct ProjectionVisit<'run> {
    pub(crate) providers: &'run [&'run dyn Provider],
    pub(crate) budget: &'run InputBudget,
}

impl TraversalVisit for ProjectionVisit<'_> {
    fn counts_sources(&self) -> bool {
        true
    }

    fn directory(
        &mut self,
        archive: &Archive,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        crate::directory_run::run(archive, lookup, self.providers, self.budget, writer)
    }

    fn entry(
        &mut self,
        entry: &Entry,
        selection: EntrySelection,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        if selection != EntrySelection::Included {
            return Ok(());
        }
        let (source, payload) = required_input::retain(lookup, entry, writer)?;
        for (index, provider) in self.providers.iter().enumerate() {
            let input = writer.origins.input(lookup.index.archive, entry)?;
            writer.begin_with_input(index, source.clone(), input)?;
            let result = provider
                .source(
                    &SourceContext {
                        source: &source,
                        entry,
                        payload: &payload,
                        directory: lookup,
                    },
                    writer,
                )
                .map_err(|cause| crate::error::at(&source, cause))?;
            writer
                .end_source(result)
                .map_err(|cause| crate::error::at(&source, cause))?;
        }
        Ok(())
    }

    fn excluded_container(&mut self) {}
}

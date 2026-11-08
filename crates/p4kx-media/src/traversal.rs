// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;

use p4kx_extraction::{Archive, Entry, Exclusion, Filter};

use crate::directory_index::DirectoryIndex;
use crate::directory_inputs::DirectoryInputs;
use crate::directory_lookup::DirectoryLookup;
use crate::input_budget::InputBudget;
use crate::source_selection::{EntrySelection, SourceSelection};
use crate::{ContainerIdentity, Digest, MediaError, MediaWriter, Provider, SourceContext, Tally};
use crate::{required_input, source};

pub(crate) struct Traversal<'run> {
    pub(crate) root: &'run Digest,
    pub(crate) providers: &'run [&'run dyn Provider],
    pub(crate) include: &'run Filter,
    pub(crate) exclude: &'run Exclusion,
    pub(crate) entries: Tally,
    pub(crate) budget: &'run InputBudget,
    pub(crate) index_keys: &'run Cell<u64>,
}

impl Traversal<'_> {
    pub(crate) fn walk(
        &mut self,
        archive: &Archive,
        chain: &mut Vec<ContainerIdentity>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let index = DirectoryIndex::new(archive, self.index_keys);
        let inputs = DirectoryInputs::try_new(archive.entries().len(), self.budget, writer)?;
        let selection = SourceSelection {
            include: self.include,
            exclude: self.exclude,
        };
        {
            let lookup =
                DirectoryLookup::new_with_inputs(&index, self.root, chain, selection, &inputs);
            crate::directory_run::run(archive, &lookup, self.providers, self.budget, writer)?;
        }
        for (position, entry) in archive.entries().iter().enumerate() {
            inputs.admit(position, writer)?;
            self.entries.enumerated = self.budget.sources.get();
            let selected = selection.classify(chain, entry.path());
            if selected == EntrySelection::Included {
                let lookup =
                    DirectoryLookup::new_with_inputs(&index, self.root, chain, selection, &inputs);
                self.project(entry, &lookup, writer)?;
            } else {
                self.entries.skipped += 1;
            }
            if selected != EntrySelection::Excluded && is_container(entry.path()) {
                self.container(archive, entry, chain, writer)?;
            }
        }
        self.entries.enumerated = self.budget.sources.get();
        Ok(())
    }

    fn project(
        &mut self,
        entry: &Entry,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let (source, payload) = required_input::retain(lookup, entry, writer)?;
        for (index, provider) in self.providers.iter().enumerate() {
            writer.begin(index, source.clone())?;
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
        self.entries.written += 1;
        Ok(())
    }

    fn container(
        &mut self,
        archive: &Archive,
        entry: &Entry,
        chain: &mut Vec<ContainerIdentity>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        writer.checkpoint()?;
        if chain.len() as u64 >= u64::from(writer.limits.depth) {
            return Err(MediaError::Limit {
                budget: "depth",
                limit: u64::from(writer.limits.depth),
            });
        }
        if entry.uncompressed_size() > writer.limits.entry_bytes {
            return Err(MediaError::Limit {
                budget: "entry_bytes",
                limit: writer.limits.entry_bytes,
            });
        }
        self.budget
            .bytes(entry.uncompressed_size(), writer.limits())?;
        let container = archive.open_container(entry).map_err(|cause| {
            let failure = MediaError::Archive(Box::new(cause));
            source::entry(self.root, archive, entry, chain)
                .map_or_else(|error| error, |entry| crate::error::at(&entry, failure))
        })?;
        writer.checkpoint()?;
        chain.push(source::identity(archive, entry));
        let result = self.walk(&container, chain, writer);
        let _container = chain.pop();
        result
    }
}

fn is_container(path: &str) -> bool {
    path.rsplit('.').next().is_some_and(|extension| {
        extension.eq_ignore_ascii_case("pak") || extension.eq_ignore_ascii_case("socpak")
    })
}

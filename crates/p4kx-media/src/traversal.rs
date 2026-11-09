// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;

use p4kx_extraction::{Archive, Exclusion, Filter};

use crate::directory_index::DirectoryIndex;
use crate::directory_inputs::DirectoryInputs;
use crate::directory_lookup::DirectoryLookup;
use crate::input_budget::InputBudget;
use crate::shader_lookup::ShaderLookup;
use crate::source_selection::{EntrySelection, SourceSelection};
use crate::traversal_projection::ProjectionVisit;
use crate::traversal_visit::TraversalVisit;
use crate::{ContainerIdentity, Digest, MediaError, MediaWriter, Provider, Tally};
use crate::{source, traversal_container};

pub(crate) struct Traversal<'run> {
    pub(crate) root: &'run Digest,
    pub(crate) providers: &'run [&'run dyn Provider],
    pub(crate) include: &'run Filter,
    pub(crate) exclude: &'run Exclusion,
    pub(crate) entries: Tally,
    pub(crate) budget: &'run InputBudget,
    pub(crate) index_keys: &'run Cell<u64>,
    pub(crate) shaders: Option<&'run ShaderLookup<'run>>,
}

impl Traversal<'_> {
    pub(crate) fn walk(
        &mut self,
        archive: &Archive,
        chain: &mut Vec<ContainerIdentity>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut visit = ProjectionVisit {
            providers: self.providers,
            budget: self.budget,
        };
        self.visit(archive, chain, writer, &mut visit)
    }

    pub(crate) fn visit(
        &mut self,
        archive: &Archive,
        chain: &mut Vec<ContainerIdentity>,
        writer: &mut MediaWriter<'_>,
        visit: &mut impl TraversalVisit,
    ) -> Result<(), MediaError> {
        let index = DirectoryIndex::new(archive, self.index_keys);
        let inputs = DirectoryInputs::try_new(archive.entries().len(), self.budget, writer)?;
        let selection = SourceSelection {
            include: self.include,
            exclude: self.exclude,
        };
        {
            let lookup =
                DirectoryLookup::new_with_inputs(&index, self.root, chain, selection, &inputs)
                    .with_shaders(self.shaders);
            visit.directory(archive, &lookup, writer)?;
        }
        for (position, entry) in archive.entries().iter().enumerate() {
            self.admit(position, &inputs, visit.counts_sources(), writer)?;
            let selected = selection.classify(chain, entry.path());
            {
                let lookup =
                    DirectoryLookup::new_with_inputs(&index, self.root, chain, selection, &inputs)
                        .with_shaders(self.shaders);
                visit.entry(entry, selected, &lookup, writer)?;
            }
            self.count_selection(selected);
            if !traversal_container::is_container(entry.path()) {
                continue;
            }
            if selected == EntrySelection::Excluded {
                visit.excluded_container();
            } else {
                let container = traversal_container::open(
                    archive,
                    entry,
                    chain,
                    self.root,
                    self.budget,
                    writer,
                )?;
                let original = writer.origins.input(archive, entry)?;
                writer.origins.push_directory(original)?;
                chain.push(source::identity(archive, entry));
                let result = self.visit(&container, chain, writer, visit);
                let _parent = chain.pop();
                let restored = writer.origins.pop_directory();
                crate::error::finish(result, restored)?;
            }
        }
        Ok(())
    }

    fn admit(
        &mut self,
        position: usize,
        inputs: &DirectoryInputs<'_>,
        counts_sources: bool,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        if counts_sources {
            inputs.admit(position, writer)?;
            self.entries.enumerated = self.budget.sources.get();
        } else {
            if self.entries.enumerated >= writer.limits().sources {
                return Err(MediaError::Limit {
                    budget: "sources",
                    limit: writer.limits().sources,
                });
            }
            self.budget
                .metadata(0, 1, writer.limits(), writer.checkpoint)?;
            self.entries.enumerated += 1;
        }
        Ok(())
    }

    fn count_selection(&mut self, selection: EntrySelection) {
        if selection == EntrySelection::Included {
            self.entries.written += 1;
        } else {
            self.entries.skipped += 1;
        }
    }
}

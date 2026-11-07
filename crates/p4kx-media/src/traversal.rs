// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;

use p4kx_extraction::{Archive, Entry, Exclusion, Filter};

use crate::directory_index::DirectoryIndex;
use crate::directory_lookup::DirectoryLookup;
use crate::source_selection::{EntrySelection, SourceSelection};

use crate::contract::Tally;
use crate::retained_source;
use crate::source;
use crate::{ContainerIdentity, Digest, MediaError, MediaWriter, Provider, SourceContext};

pub(crate) struct Traversal<'a> {
    pub(crate) root: &'a Digest,
    pub(crate) providers: &'a [&'a dyn Provider],
    pub(crate) include: &'a Filter,
    pub(crate) exclude: &'a Exclusion,
    pub(crate) entries: Tally,
    pub(crate) bytes: u64,
    pub(crate) metadata_work: &'a Cell<u64>,
    pub(crate) index_keys: &'a Cell<u64>,
}

impl Traversal<'_> {
    pub(crate) fn walk(
        &mut self,
        archive: &Archive,
        chain: &mut Vec<ContainerIdentity>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let index = DirectoryIndex::new(archive, self.index_keys);
        let lookup = DirectoryLookup::new(
            &index,
            self.root,
            chain,
            SourceSelection {
                include: self.include,
                exclude: self.exclude,
            },
            self.metadata_work,
            writer
                .limits
                .sources
                .saturating_sub(self.entries.enumerated),
        );
        crate::directory_run::run(
            archive,
            &lookup,
            self.providers,
            &mut self.bytes,
            self.metadata_work,
            writer,
        )?;
        for (position, entry) in archive.entries().iter().enumerate() {
            writer.checkpoint()?;
            self.entries.enumerated += 1;
            if self.entries.enumerated > writer.limits.sources {
                return Err(MediaError::Limit {
                    budget: "sources",
                    limit: writer.limits.sources,
                });
            }
            let selection = SourceSelection {
                include: self.include,
                exclude: self.exclude,
            }
            .classify(chain, entry.path());
            if selection == EntrySelection::Included {
                let allowance = writer.limits.sources - self.entries.enumerated
                    + u64::try_from(position)
                        .map_err(|_| MediaError::Contract("source position exceeds u64".into()))?
                    + 1;
                self.project(archive, entry, chain, &index, allowance, writer)?;
            } else {
                self.entries.skipped += 1;
            }
            if selection != EntrySelection::Excluded && is_container(entry.path()) {
                self.container(archive, entry, chain, writer)?;
            }
        }
        Ok(())
    }

    fn project(
        &mut self,
        archive: &Archive,
        entry: &Entry,
        chain: &[ContainerIdentity],
        index: &DirectoryIndex<'_>,
        source_allowance: u64,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut source = source::entry(self.root, archive, entry, chain)?;
        let size = entry.uncompressed_size();
        if size > writer.limits.source_bytes.saturating_sub(self.bytes) {
            return Err(MediaError::Limit {
                budget: "source_bytes",
                limit: writer.limits.source_bytes,
            });
        }
        self.bytes += size;
        let (raw, payload) = retained_source::retain(archive, entry, writer)
            .map_err(|cause| crate::error::at(&source, cause))?;
        source::retained(&mut source, entry, raw);
        writer
            .retained(&source)
            .map_err(|cause| crate::error::at(&source, cause))?;
        let lookup = DirectoryLookup::new(
            index,
            self.root,
            chain,
            SourceSelection {
                include: self.include,
                exclude: self.exclude,
            },
            self.metadata_work,
            source_allowance,
        );
        for (index, provider) in self.providers.iter().enumerate() {
            writer.begin(index, source.clone())?;
            let result = provider
                .source(
                    &SourceContext {
                        source: &source,
                        entry,
                        payload: &payload,
                        directory: &lookup,
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
        if entry.uncompressed_size() > writer.limits.source_bytes.saturating_sub(self.bytes) {
            return Err(MediaError::Limit {
                budget: "source_bytes",
                limit: writer.limits.source_bytes,
            });
        }
        self.bytes += entry.uncompressed_size();
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

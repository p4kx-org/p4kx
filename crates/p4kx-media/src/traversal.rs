// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Entry, Exclusion, Filter};

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
}

impl Traversal<'_> {
    pub(crate) fn walk(
        &mut self,
        archive: &Archive,
        chain: &mut Vec<ContainerIdentity>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        for entry in archive.entries() {
            writer.checkpoint()?;
            self.entries.enumerated += 1;
            if self.entries.enumerated > writer.limits.sources {
                return Err(MediaError::Limit {
                    budget: "sources",
                    limit: writer.limits.sources,
                });
            }
            let qualified = qualified_path(chain, entry.path());
            let excluded = self.excluded(entry.path()) || self.exclude.matches(&qualified);
            let selected = !excluded
                && (self.include.matches(entry.path()) || self.include.matches(&qualified));
            if selected {
                self.project(archive, entry, chain, writer)?;
            } else {
                self.entries.skipped += 1;
            }
            if !excluded && is_container(entry.path()) {
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
        for (index, provider) in self.providers.iter().enumerate() {
            writer.begin(index, source.clone())?;
            let result = provider
                .source(
                    &SourceContext {
                        source: &source,
                        entry,
                        payload: &payload,
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

    fn excluded(&self, path: &str) -> bool {
        if self.exclude.matches(path) {
            return true;
        }
        let lower = path.to_ascii_lowercase();
        if let Some(end) = lower.find(".dds.") {
            return self.exclude.matches(path.get(..end + 4).unwrap_or(path));
        }
        false
    }
}

fn is_container(path: &str) -> bool {
    path.rsplit('.').next().is_some_and(|extension| {
        extension.eq_ignore_ascii_case("pak") || extension.eq_ignore_ascii_case("socpak")
    })
}

fn qualified_path(chain: &[ContainerIdentity], path: &str) -> String {
    let mut result = String::new();
    for container in chain {
        result.push_str(&container.path);
        result.push('!');
    }
    result.push_str(path);
    result
}

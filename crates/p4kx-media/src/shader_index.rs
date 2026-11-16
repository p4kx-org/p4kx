// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::cell::{Cell, OnceCell};
use std::collections::HashMap;
use std::io;

use p4kx_extraction::{Archive, Entry, EntryOrigin, EntryOriginStep};

use crate::directory_lookup::DirectoryLookup;
use crate::shader_flags::ShaderRead;
use crate::shader_lookup::ShaderLookup;
use crate::source_selection::EntrySelection;
use crate::traversal::Traversal;
use crate::traversal_visit::TraversalVisit;
use crate::{
    CandidateInventory, CandidateState, MediaError, MediaKind, MediaWriter, NotSelectedReason,
    ReferenceCandidate, ReferenceTarget, ReferenceUnavailable, Source, Tally,
};
use crate::{shader_source, source};

pub(crate) struct ShaderTarget {
    pub(crate) source: Source,
    pub(crate) origin: EntryOrigin,
    pub(crate) state: CandidateState,
    pub(crate) read: OnceCell<ShaderRead>,
}

pub(crate) struct ShaderIndex {
    pub(crate) targets: Vec<ShaderTarget>,
    names: HashMap<String, Vec<usize>>,
    pub(crate) inventory: CandidateInventory,
    pub(crate) inventory_origins: crate::origin_contributions::OriginContributions,
}

impl ShaderIndex {
    pub(crate) fn build(
        lookup: &ShaderLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        let mut collector = Collector {
            index: Self {
                targets: Vec::new(),
                names: HashMap::new(),
                inventory: CandidateInventory::Complete,
                inventory_origins: crate::origin_contributions::OriginContributions::independent(
                    &writer.origins.owner,
                ),
            },
            requested: writer.has_kind(MediaKind::Shader),
            inventory_capture: writer.capture_origins(),
        };
        let keys = Cell::new(0);
        let mut traversal = Traversal {
            root: lookup.root,
            providers: &[],
            include: lookup.selection.include,
            exclude: lookup.selection.exclude,
            entries: Tally::default(),
            budget: lookup.budget,
            index_keys: &keys,
            shaders: None,
        };
        traversal.visit(lookup.archive, &mut Vec::new(), writer, &mut collector)?;
        collector.index.inventory_origins = collector.inventory_capture.finish().share_uses();
        Ok(collector.index)
    }

    pub(crate) fn candidates(
        &self,
        positions: &[usize],
        writer: &MediaWriter<'_>,
    ) -> Result<Vec<crate::shader_origins::ShaderCandidate>, MediaError> {
        let observer = writer.origins.observer()?;
        let mut candidates = Vec::new();
        candidates
            .try_reserve_exact(positions.len())
            .map_err(|cause| allocation(writer, cause))?;
        for position in positions {
            let target = self.targets.get(*position).ok_or_else(|| {
                MediaError::Contract("shader candidate is outside its physical index".into())
            })?;
            candidates.push(crate::shader_origins::ShaderCandidate {
                value: target.candidate(),
                evidence: crate::shader_origins::ShaderCandidateInput {
                    input: observer.original(&target.origin)?,
                    property: None,
                    site: None,
                },
            });
        }
        Ok(candidates)
    }
    pub(crate) fn matching(
        &self,
        name: &str,
        lookup: &ShaderLookup<'_>,
        writer: &MediaWriter<'_>,
    ) -> Result<&[usize], MediaError> {
        lookup.work(name.len().saturating_add(1), writer)?;
        let name = if name.as_bytes().iter().any(u8::is_ascii_uppercase) {
            Cow::Owned(fold_name(name, writer)?)
        } else {
            Cow::Borrowed(name)
        };
        let Some(positions) = self.names.get(name.as_ref()) else {
            return Ok(&[]);
        };
        // The resolver uses the first position after its limit as an overage witness.
        let admitted = usize::try_from(writer.limits().reference_candidates)
            .unwrap_or(usize::MAX)
            .saturating_add(1);
        let count = positions.len().min(admitted);
        lookup.work(count, writer)?;
        positions.get(..count).ok_or_else(|| {
            MediaError::Contract("shader candidate prefix exceeds its name bucket".into())
        })
    }

    fn index_name(
        &mut self,
        name: &str,
        lookup: &DirectoryLookup<'_>,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let name = fold_name(name, writer)?;
        if !self.names.contains_key(&name) {
            lookup.input_state()?.budget.metadata(
                0,
                crate::shader_values::count(size_of::<(String, Vec<usize>)>())?,
                writer.limits(),
                writer.checkpoint,
            )?;
            self.names
                .try_reserve(1)
                .map_err(|cause| allocation(writer, cause))?;
        }
        let positions = self.names.entry(name).or_default();
        positions
            .try_reserve(1)
            .map_err(|cause| allocation(writer, cause))?;
        positions.push(self.targets.len());
        Ok(())
    }
}

impl ShaderTarget {
    pub(crate) fn candidate(&self) -> ReferenceCandidate {
        let mut target = ReferenceTarget::new_with_source(&self.source);
        target.kind = Some(MediaKind::Shader);
        ReferenceCandidate::new(target, self.state)
    }
}

struct Collector {
    index: ShaderIndex,
    requested: bool,
    inventory_capture: crate::origin_capture::OriginCapture,
}

impl TraversalVisit for Collector {
    fn counts_sources(&self) -> bool {
        false
    }

    fn directory(
        &mut self,
        archive: &Archive,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let input = if lookup.chain.is_empty() {
            writer.origins.root.as_ref()
        } else {
            writer.origins.directory()
        };
        self.inventory_capture
            .include(&crate::origin_lookup::directory(input, writer)?, writer)?;
        for (_, issue) in archive.issues() {
            lookup
                .input_state()?
                .budget
                .metadata(0, 1, writer.limits(), writer.checkpoint)?;
            if issue.is_archive_wide() {
                self.index.inventory =
                    CandidateInventory::Unavailable(ReferenceUnavailable::IncompleteInventory);
            }
        }
        Ok(())
    }

    fn entry(
        &mut self,
        entry: &Entry,
        selection: EntrySelection,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<bool, MediaError> {
        let selected = selection == EntrySelection::Included;
        if lookup.index.archive.raw_path(entry.index()).is_some() {
            self.index.inventory =
                CandidateInventory::Unavailable(ReferenceUnavailable::IncompleteInventory);
        }
        let file = shader_source::file_name(entry.path());
        let Some((name, extension)) = file.rsplit_once('.') else {
            return Ok(selected);
        };
        if !extension.eq_ignore_ascii_case("ext") {
            return Ok(selected);
        }
        lookup.input_state()?.budget.metadata(
            0,
            identity_work(name, entry, lookup)?,
            writer.limits(),
            writer.checkpoint,
        )?;
        self.index
            .targets
            .try_reserve(1)
            .map_err(|cause| allocation(writer, cause))?;
        let origin = lookup.index.archive.entry_origin(entry).ok_or_else(|| {
            MediaError::Contract("shader inventory Entry has no owned origin".into())
        })?;
        let state = if selection != EntrySelection::Included {
            CandidateState::NotSelected {
                reason: NotSelectedReason::Filter,
            }
        } else if !self.requested {
            CandidateState::NotSelected {
                reason: NotSelectedReason::KindNotRequested,
            }
        } else {
            CandidateState::Selected
        };
        let source = source::entry(lookup.root, lookup.index.archive, entry, lookup.chain)?;
        self.index.index_name(name, lookup, writer)?;
        self.index.targets.push(ShaderTarget {
            source,
            origin,
            state,
            read: OnceCell::new(),
        });
        Ok(selected)
    }

    fn excluded_container(&mut self) {
        self.index.inventory =
            CandidateInventory::Unavailable(ReferenceUnavailable::IncompleteInventory);
    }
}

fn identity_work(
    name: &str,
    entry: &Entry,
    lookup: &DirectoryLookup<'_>,
) -> Result<u64, MediaError> {
    let mut work = crate::shader_values::count(size_of::<ShaderTarget>())?;
    let mut add = |length: usize, copies: u64| -> Result<(), MediaError> {
        let bytes = crate::shader_values::count(length)?
            .checked_mul(copies)
            .ok_or_else(|| MediaError::Contract("shader identity storage size overflow".into()))?;
        work = work
            .checked_add(bytes)
            .ok_or_else(|| MediaError::Contract("shader identity storage size overflow".into()))?;
        Ok(())
    };
    add(size_of::<EntryOriginStep>(), 1)?;
    add(
        size_of_val(entry.metadata().encryption_declarations().iter().as_slice()),
        1,
    )?;
    add(name.len(), 1)?;
    add(size_of::<usize>(), 1)?;
    add(entry.path().len(), 2)?;
    add(lookup.root.hex.len(), 1)?;
    if let Some(raw) = lookup.index.archive.raw_path(entry.index()) {
        add(raw.len(), 2)?;
    }
    for ancestor in lookup.chain {
        add(ancestor.path.len(), 1)?;
        if let Some(raw) = &ancestor.raw_path_hex {
            add(raw.len(), 1)?;
        }
    }
    Ok(work)
}
fn fold_name(name: &str, writer: &MediaWriter<'_>) -> Result<String, MediaError> {
    let mut key = String::new();
    key.try_reserve_exact(name.len())
        .map_err(|cause| allocation(writer, cause))?;
    key.push_str(name);
    key.make_ascii_lowercase();
    Ok(key)
}

fn allocation(writer: &MediaWriter<'_>, cause: std::collections::TryReserveError) -> MediaError {
    MediaError::Io {
        path: writer.root.clone(),
        source: io::Error::new(io::ErrorKind::OutOfMemory, cause),
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::Path;

    use p4kx_extraction::{
        Archive, ArchiveWatch, ArchiveWriteProtection, CancelToken, Exclusion, Filter,
    };
    use p4kx_testkit::{ArchiveBuilder, TestEntry};

    use super::{ShaderIndex, ShaderLookup};
    use crate::artifact::StreamObserver;
    use crate::checkpoint::Checkpoint;
    use crate::input_budget::InputBudget;
    use crate::origin_test_support::TestResult;
    use crate::source_selection::SourceSelection;
    use crate::{ByteScope, Limits, MediaWriter, hashing};

    const UNRELATED: u32 = 128;

    #[test]
    fn shader_lookup_work_ignores_unrelated_names_and_unused_duplicates() -> TestResult {
        let folder = tempfile::tempdir()?;
        let archive = fixture(folder.path())?;
        let cancel = CancelToken::new();
        let watch = ArchiveWatch::new(&archive);
        let protection = ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
        let checkpoint = Checkpoint {
            cancel: &cancel,
            watch: &watch,
            protection: &protection,
        };
        let limits = Limits {
            reference_candidates: 1,
            ..Limits::default()
        };
        let (root, size) = hashing::file(
            archive.path(),
            ByteScope::RootArchiveFile,
            limits.root_bytes,
            &checkpoint,
        )?;
        let budget = InputBudget::default();
        let include = Filter::parse("*")?;
        let exclude = Exclusion::default();
        let lookup = ShaderLookup::new(
            &archive,
            &root,
            SourceSelection {
                include: &include,
                exclude: &exclude,
            },
            &budget,
        );
        let mut writer = MediaWriter::observed(
            folder.path(),
            Vec::new(),
            limits,
            checkpoint,
            false,
            &Discard,
        )?;
        writer.origins.observe_root(&archive, size)?;
        let index = ShaderIndex::build(&lookup, &mut writer)?;
        check_work(&index, &lookup, &writer)
    }

    fn check_work(
        index: &ShaderIndex,
        lookup: &ShaderLookup<'_>,
        writer: &MediaWriter<'_>,
    ) -> TestResult {
        let budget = lookup.budget;
        let before = budget.work_count()?;
        let query = "EXPECTED";
        let matches = index.matching(query, lookup, writer)?;
        assert_eq!(
            matches,
            [0, usize::try_from(UNRELATED)? + 1],
            "the bounded physical prefix"
        );
        assert_eq!(
            budget.work_count()? - before,
            u64::try_from(query.len() + 1 + matches.len())?,
            "charge only one key and the admitted prefix"
        );

        let before = budget.work_count()?;
        let query = "missing";
        assert!(
            index.matching(query, lookup, writer)?.is_empty(),
            "missing remains an empty candidate list"
        );
        assert_eq!(
            budget.work_count()? - before,
            u64::try_from(query.len() + 1)?,
            "a miss only charges its own key"
        );
        Ok(())
    }

    fn fixture(root: &Path) -> Result<Archive, Box<dyn std::error::Error>> {
        let path = root.join("Data.p4k");
        let mut archive = ArchiveBuilder::new().entry(TestEntry::stored("Data/expected.ext", b""));
        for index in 0..UNRELATED {
            archive = archive.entry(TestEntry::stored(format!("Data/extra-{index}.ext"), b""));
        }
        archive
            .entry(TestEntry::stored("Data/Expected.ext", b""))
            .entry(TestEntry::stored("Data/ExPeCtEd.ext", b""))
            .write_to(&path)?;
        Ok(Archive::open(&path)?)
    }

    struct Discard;

    impl StreamObserver for Discard {
        fn write(&self, _relative: &str, _bytes: &[u8]) -> io::Result<()> {
            Ok(())
        }
    }
}

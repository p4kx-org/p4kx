// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Bounded discovery of a complete nested shader family by physical source identity.

use p4kx_extraction::{Archive, ArchiveWatch, CancelToken, Entry, Exclusion, Filter};
use p4kx_media::ContainerIdentity;

#[path = "shader_inventory_read.rs"]
mod read;

use super::TestResult;
use super::shader_expected::ExpectedSource;

const ENTRY_BYTES: u64 = 1 << 20;
pub(super) const SOURCE_BYTES: u64 = 8 << 20;
pub(super) const ENTRY_COUNT: u64 = 10_000_000;
pub(super) const CONTAINER_BYTES: u64 = 64 << 20;
const DISCOVERY_BYTES: u64 = 256 << 20;
pub(super) const EXPORT_BYTES: u64 = DISCOVERY_BYTES + SOURCE_BYTES;
const ROOT_CONTAINERS: usize = 64;
const CONTAINERS: usize = 128;
const DEFINITION_SOURCES: usize = 256;

pub(super) struct Selection {
    pub(super) include: Filter,
    pub(super) exclude: Exclusion,
    pub(super) sources: Vec<ExpectedSource>,
    pub(super) enumerated: u64,
    pub(super) compiled: usize,
}

struct Budget<'a> {
    watch: &'a ArchiveWatch,
    cancel: &'a CancelToken,
    opened: usize,
    entries: u64,
    bytes: u64,
}

struct Family<'a> {
    include: &'a Filter,
    exclude: &'a Exclusion,
    sources: Vec<ExpectedSource>,
    enumerated: u64,
    source_bytes: u64,
    compiled: usize,
    expected_records: u64,
}

pub(super) fn discover(
    archive: &Archive,
    watch: &ArchiveWatch,
    cancel: &CancelToken,
) -> TestResult<Selection> {
    let root_count = u64::try_from(archive.entries().len())?;
    if root_count > ENTRY_COUNT {
        return Err("root directory exceeds shader verification Entry budget".into());
    }
    let containers = root_containers(archive)?;
    let mut budget = Budget {
        watch,
        cancel,
        opened: 0,
        entries: root_count,
        bytes: 0,
    };
    let mut oversized = 0;
    for (position, candidate) in containers.iter().enumerate() {
        budget.checkpoint()?;
        if containers
            .get(..position)
            .ok_or("root container prefix is unavailable")?
            .iter()
            .any(|prior| same_path(prior.path(), candidate.path()))
        {
            continue;
        }
        if candidate.uncompressed_size() > CONTAINER_BYTES {
            oversized += 1;
            continue;
        }
        let include = Filter::parse(&format!("{}!*.ext", candidate.path()))?;
        let exclude = family_exclusion(&containers, candidate.path())?;
        let Family {
            sources,
            enumerated,
            compiled,
            ..
        } = inspect_group(
            archive,
            &containers,
            candidate.path(),
            &include,
            &exclude,
            &mut budget,
        )?;
        if !sources.is_empty() {
            return Ok(Selection {
                include,
                exclude,
                sources,
                enumerated: root_count
                    .checked_add(enumerated)
                    .ok_or("Entry count overflow")?,
                compiled,
            });
        }
    }
    unavailable(containers.len(), oversized);
    Err("no bounded nested shader family was available for export verification".into())
}

fn family_exclusion(containers: &[&Entry], path: &str) -> TestResult<Exclusion> {
    let exclude = Exclusion::parse(
        std::iter::once("*.socpak").chain(
            containers
                .iter()
                .filter(|other| !same_path(other.path(), path))
                .map(|other| other.path()),
        ),
    )?;
    if exclude.matches(path) {
        return Err("shader family cannot be isolated by the supported Filters".into());
    }
    Ok(exclude)
}

fn root_containers(archive: &Archive) -> TestResult<Vec<&Entry>> {
    let mut containers = Vec::new();
    for entry in archive.entries() {
        if !extension(entry.path()).is_some_and(|ext| ext.eq_ignore_ascii_case("pak")) {
            continue;
        }
        if containers.len() == ROOT_CONTAINERS || entry.path().contains(['*', '?']) {
            return Err("root container inventory exceeds supported verification selection".into());
        }
        containers.push(entry);
    }
    Ok(containers)
}

fn inspect_group<'a>(
    archive: &Archive,
    containers: &[&Entry],
    path: &str,
    include: &'a Filter,
    exclude: &'a Exclusion,
    budget: &mut Budget<'_>,
) -> TestResult<Family<'a>> {
    let mut family = Family {
        include,
        exclude,
        sources: Vec::new(),
        enumerated: 0,
        source_bytes: 0,
        compiled: 0,
        expected_records: 0,
    };
    for entry in containers
        .iter()
        .filter(|entry| same_path(entry.path(), path))
    {
        let mut chain = vec![ContainerIdentity::try_new_with_entry(archive, entry)?];
        let child = budget.open(archive, entry)?;
        family.walk(&child, &mut chain, budget)?;
    }
    Ok(family)
}

impl Family<'_> {
    fn walk(
        &mut self,
        archive: &Archive,
        chain: &mut Vec<ContainerIdentity>,
        budget: &mut Budget<'_>,
    ) -> TestResult {
        budget.entries = budget
            .entries
            .checked_add(u64::try_from(archive.entries().len())?)
            .filter(|count| *count <= ENTRY_COUNT)
            .ok_or("discovery Entry budget exceeded")?;
        self.enumerated = self
            .enumerated
            .checked_add(u64::try_from(archive.entries().len())?)
            .filter(|count| *count <= ENTRY_COUNT)
            .ok_or("nested Entry budget exceeded")?;
        for entry in archive.entries() {
            budget.checkpoint()?;
            let qualified = qualified(chain, entry.path());
            if self.exclude.leaves_out(entry.path()) || self.exclude.leaves_out(&qualified) {
                continue;
            }
            if is_container(entry.path()) {
                if chain.len() >= p4kx_extraction::MAX_CONTAINER_DEPTH {
                    return Err("shader verification container depth exceeded".into());
                }
                let child = budget.open(archive, entry)?;
                chain.push(ContainerIdentity::try_new_with_entry(archive, entry)?);
                self.walk(&child, chain, budget)?;
                let _container = chain.pop();
            } else if extension(entry.path()).is_some_and(|ext| ext.eq_ignore_ascii_case("ext")) {
                if !self.include.matches(entry.path()) && !self.include.matches(&qualified) {
                    return Err(
                        "discovered shader source is outside the exact export Filter".into(),
                    );
                }
                self.read_source(archive, entry, chain, budget)?;
            } else if extension(entry.path()).is_some_and(|ext| {
                ["cfib", "cfxb", "shcb"]
                    .iter()
                    .any(|known| ext.eq_ignore_ascii_case(known))
            }) {
                self.compiled = self.compiled.checked_add(1).ok_or("cache count overflow")?;
            }
        }
        Ok(())
    }
}

fn same_path(left: &str, right: &str) -> bool {
    left.bytes()
        .map(normalized)
        .eq(right.bytes().map(normalized))
}

const fn normalized(byte: u8) -> u8 {
    if byte == b'\\' {
        b'/'
    } else {
        byte.to_ascii_lowercase()
    }
}

fn qualified(chain: &[ContainerIdentity], leaf: &str) -> String {
    let mut path = String::new();
    for entry in chain {
        path.push_str(&entry.path);
        path.push('!');
    }
    path.push_str(leaf);
    path
}

fn extension(path: &str) -> Option<&str> {
    path.rsplit_once('.').map(|(_, extension)| extension)
}

fn is_container(path: &str) -> bool {
    extension(path)
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pak") || ext.eq_ignore_ascii_case("socpak"))
}

#[expect(
    clippy::print_stderr,
    reason = "the opt-in test reports only bounded capability counts"
)]
fn unavailable(containers: usize, oversized: usize) {
    eprintln!(
        "nested .pak shader family unavailable; .socpak explicitly excluded: root_containers={containers}, containers_over_64_mib={oversized}; no complete source-export claim"
    );
}

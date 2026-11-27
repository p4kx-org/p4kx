// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::Archive;
use p4kx_dataforge::DataForge;

use crate::DataForgeEntry;
use crate::coverage::{CoverageReport, ReferenceList};
use crate::coverage_entries;
use crate::coverage_forge;
use crate::coverage_origins::{CoverageOriginError, CoverageSites, ObservedCoverage};

impl CoverageReport {
    /// Checks what `forge` and `archive` hold of `list`: for every Record type whether it is
    /// defined, how many Records it has, whether they read without error and whether a sample
    /// exports; for every component how many `EntityClassDefinition` Records and instances have
    /// it, derived structs included, and whether a Component selector Query reaches it; for every
    /// Entry folder whether its Entries convert from CryXml. `forge` is the DataForge of
    /// `archive`.
    #[must_use]
    pub fn of(archive: &Archive, forge: &DataForge<'_>, list: &ReferenceList<'_>) -> Self {
        scan(archive, forge, list, None)
    }

    /// Checks coverage and retains original sources for the sample and failure paths.
    ///
    /// # Errors
    /// Returns an error if a field path or its original source cannot be registered.
    pub fn observed(
        archive: &Archive,
        forge: &DataForge<'_>,
        input: &DataForgeEntry,
        list: &ReferenceList<'_>,
    ) -> Result<ObservedCoverage, CoverageOriginError> {
        let mut sites = CoverageSites::default();
        let report = scan(archive, forge, list, Some(&mut sites));
        ObservedCoverage::new(report, archive, forge, input, sites)
    }
}

fn scan<'archive, 'forge>(
    archive: &'archive Archive,
    forge: &'forge DataForge<'_>,
    list: &ReferenceList<'_>,
    mut sites: Option<&mut CoverageSites<'archive, 'forge>>,
) -> CoverageReport {
    let found = coverage_forge::check(
        forge,
        list.items(),
        sites.as_mut().map(|sites| &mut sites.samples),
    );
    let entries = coverage_entries::check(
        archive,
        list.items(),
        sites.map(|sites| &mut sites.failures),
    );
    CoverageReport {
        types: found.types,
        components: found.components,
        objects: found.objects,
        entries,
        unreadable_entities: found.unreadable_entities,
    }
}

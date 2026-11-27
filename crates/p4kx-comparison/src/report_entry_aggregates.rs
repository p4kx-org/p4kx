// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    EntryDirectoryFact, EntryDirectoryOrigins, FieldOrigin, FieldOrigins, SourceEvidenceError,
    SourceUseRole, SourceUses,
};

use crate::path_key::{cmp_normalised, top_level};
use crate::report_origins::{ReportOriginsError, put};
use crate::{ChangeKind, DirectoryCounts, EntryChange};

pub(crate) struct AggregateOrigins<'a> {
    directories: &'a [DirectoryCounts<'a>],
    totals: CountOrigins,
    selects_paths: bool,
    groups: Vec<GroupOrigins>,
}

impl<'a> AggregateOrigins<'a> {
    pub(crate) fn new(directories: &'a [DirectoryCounts<'a>], selects_paths: bool) -> Self {
        Self {
            directories,
            totals: CountOrigins::default(),
            selects_paths,
            groups: directories
                .iter()
                .map(|_| GroupOrigins::default())
                .collect(),
        }
    }

    pub(crate) fn observe(
        &mut self,
        change: &EntryChange<'_>,
        selected: &EntryDirectoryOrigins,
        decision: &FieldOrigin,
        uses: &SourceUses,
    ) -> Result<(), ReportOriginsError> {
        let directory = top_level(change.path());
        let index = self
            .directories
            .binary_search_by(|known| cmp_normalised(known.directory, directory))
            .map_err(|_| SourceEvidenceError::ForeignEvidence)?;
        let group = self
            .groups
            .get_mut(index)
            .ok_or(SourceEvidenceError::ForeignEvidence)?;
        let selection = selected.use_fact(EntryDirectoryFact::Path, SourceUseRole::Selector)?;
        let membership = uses.origin(&[selection], selected.complete());
        if group.first.is_none() {
            let conversion =
                selected.use_fact(EntryDirectoryFact::Path, SourceUseRole::Conversion)?;
            group.first = Some(GroupName {
                display: uses.origin(&[conversion], selected.complete()),
                selector: membership.clone(),
            });
        }
        let kind = change.kind();
        let total = self.totals.for_kind(kind);
        total.push(decision.clone());
        if self.selects_paths {
            total.push(membership.clone());
        }
        group
            .counts
            .for_kind(kind)
            .extend([decision.clone(), membership]);
        Ok(())
    }

    pub(crate) fn write(
        self,
        uses: &SourceUses,
        fields: &mut FieldOrigins,
    ) -> Result<(), ReportOriginsError> {
        self.totals.write("/entries/counts", None, uses, fields)?;
        for (index, group) in self.groups.into_iter().enumerate() {
            let first = group.first.ok_or(SourceEvidenceError::ForeignEvidence)?;
            let path = format!("/entries/directories/{index}");
            put(fields, &format!("{path}/directory"), first.display)?;
            group
                .counts
                .write(&path, Some(&first.selector), uses, fields)?;
        }
        Ok(())
    }
}

#[derive(Default)]
struct GroupOrigins {
    first: Option<GroupName>,
    counts: CountOrigins,
}

struct GroupName {
    display: FieldOrigin,
    selector: FieldOrigin,
}

#[derive(Default)]
struct CountOrigins {
    added: Vec<FieldOrigin>,
    removed: Vec<FieldOrigin>,
    changed: Vec<FieldOrigin>,
}

impl CountOrigins {
    const fn for_kind(&mut self, kind: ChangeKind) -> &mut Vec<FieldOrigin> {
        match kind {
            ChangeKind::Added => &mut self.added,
            ChangeKind::Removed => &mut self.removed,
            ChangeKind::Changed => &mut self.changed,
        }
    }

    fn write(
        self,
        path: &str,
        anchor: Option<&FieldOrigin>,
        uses: &SourceUses,
        fields: &mut FieldOrigins,
    ) -> Result<(), ReportOriginsError> {
        for (field, origins) in [
            ("added", self.added),
            ("removed", self.removed),
            ("changed", self.changed),
        ] {
            let origin = if origins.is_empty() {
                uses.origin(&[], false)
            } else {
                uses.origin_with_origins(origins.iter().chain(anchor), true)?
            };
            put(fields, &format!("{path}/{field}"), origin)?;
        }
        Ok(())
    }
}

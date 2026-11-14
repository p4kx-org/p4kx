// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::cryxml::{SourceDeclaration, SourceDeclarationKind, SourceElement};
use p4kx_extraction::{Archive, PathIndex};
use p4kx_media::{
    ContainerIdentity, Identity, MediaKind, MediaRecord, Reference, ReferenceResolution,
    ReferenceTarget,
};
use serde_json::Value;

use super::{Family, TestResult, require};

pub(super) struct Selector<'a> {
    kind: &'static str,
    value: &'a str,
    edge_allowed: bool,
}

impl<'a> Selector<'a> {
    pub(super) fn of(
        family: Family,
        node: SourceElement<'a>,
        root: bool,
        attribute: &SourceDeclaration<'a>,
        edge_allowed: bool,
    ) -> Option<Self> {
        if node.namespace().is_some()
            || !matches!(
                attribute.kind(),
                SourceDeclarationKind::Attribute { namespace: None }
            )
        {
            return None;
        }
        let tag = node.name().to_ascii_lowercase();
        let field = attribute.qualified_name().to_ascii_lowercase();
        let kind = match (family, tag.as_str(), field.as_str(), root) {
            (Family::Adb, "animdb", "tagdef", true) => "tag-definition",
            (Family::Adb, "animdb", "fragdef", true) => "fragment-definition",
            (Family::Adb, "subadb", "file", _) => "sub-database",
            (Family::Comb, "blendspace", "aname", _) => "combined-space",
            _ => return None,
        };
        Some(Self {
            kind,
            value: attribute.value(),
            edge_allowed: kind == "sub-database" && edge_allowed,
        })
    }

    pub(super) const fn edge_allowed(&self) -> bool {
        self.edge_allowed
    }

    pub(super) fn verify(
        &self,
        payload: &Value,
        paths: &Paths<'_>,
        chain: &[ContainerIdentity],
    ) -> TestResult<Option<usize>> {
        require(
            payload.get("kind").and_then(Value::as_str) == Some(self.kind),
            "typed file selector role matches its original",
        )?;
        require(
            payload.get("authored_path").and_then(Value::as_str) == Some(self.value),
            "file selector retains exact authored bytes",
        )?;
        let canonical = self.path();
        require(
            payload.get("archive_path").and_then(Value::as_str) == canonical.as_deref(),
            "file selector normalization preserves observed scope",
        )?;
        let reference: Reference =
            serde_json::from_value(payload.get("reference").ok_or("file reference")?.clone())?;
        let Some(canonical) = canonical else {
            require(
                matches!(
                    reference.resolution(),
                    ReferenceResolution::Unavailable { .. }
                ),
                "invalid scoped selector remains unavailable",
            )?;
            return Ok(None);
        };
        let kind = match self.kind {
            "sub-database" | "combined-space" => Some(MediaKind::AnimationDefinition),
            _ => None,
        };
        let possible = paths.targets(&canonical)?;
        verify_resolution(&possible, reference.resolution(), chain, kind)
    }

    fn path(&self) -> Option<String> {
        let (_, extension) = self.value.rsplit_once('.')?;
        let accepted: &[_] = match self.kind {
            "sub-database" => &["adb"],
            "combined-space" => &["bspace", "comb"],
            "tag-definition" | "fragment-definition" => &["xml"],
            _ => return None,
        };
        if !accepted
            .iter()
            .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        {
            return None;
        }
        let path = self.value.replace('\\', "/");
        let (root, _) = path.split_once('/')?;
        if path.split('/').any(|part| {
            matches!(part, "" | "." | "..")
                || part.contains([':', '\0', '!', '*', '?'])
                || part.ends_with([' ', '.'])
        }) {
            return None;
        }
        if root.eq_ignore_ascii_case("data") {
            Some(path)
        } else if root.eq_ignore_ascii_case("animations") {
            Some(format!("Data/{path}"))
        } else {
            None
        }
    }
}

fn verify_resolution(
    possible: &[ContainerIdentity],
    resolution: &ReferenceResolution,
    chain: &[ContainerIdentity],
    kind: Option<MediaKind>,
) -> TestResult<Option<usize>> {
    match (possible, resolution) {
        ([], ReferenceResolution::Missing) => Ok(None),
        ([entry], ReferenceResolution::Resolved { target }) => {
            target_of(target, entry, chain, kind)?;
            Ok(Some(entry.entry_index))
        }
        ([entry], ReferenceResolution::NotSelected { target, .. }) => {
            target_of(target, entry, chain, kind)?;
            Ok(None)
        }
        (entries, ReferenceResolution::Ambiguous { candidates }) if entries.len() > 1 => {
            require(
                entries.len() == candidates.len(),
                "every ambiguous physical path candidate remains visible",
            )?;
            for (candidate, entry) in candidates.iter().zip(entries) {
                target_of(&candidate.target, entry, chain, kind)?;
            }
            Ok(None)
        }
        (
            _,
            ReferenceResolution::Unavailable {
                known_candidates, ..
            },
        ) => {
            for candidate in known_candidates {
                let Identity::Entry { entry, .. } = &candidate.target.source else {
                    return Err("nonphysical observed directory candidate".into());
                };
                require(
                    possible.contains(entry),
                    "an unavailable candidate still belongs to the consulted directory",
                )?;
                target_of(&candidate.target, entry, chain, kind)?;
            }
            Ok(None)
        }
        _ => Err("file link disposition disagrees with the original physical directory".into()),
    }
}

pub(super) struct Paths<'a> {
    archive: &'a Archive,
    index: std::cell::OnceCell<PathIndex<'a>>,
}

impl<'a> Paths<'a> {
    pub(super) const fn new(archive: &'a Archive) -> Self {
        Self {
            archive,
            index: std::cell::OnceCell::new(),
        }
    }

    fn targets(&self, path: &str) -> TestResult<Vec<ContainerIdentity>> {
        let index = self.index.get_or_init(|| PathIndex::new(self.archive));
        index
            .matching_entries_with(path, |_| Ok::<(), std::convert::Infallible>(()))?
            .map(|entry| Ok(ContainerIdentity::try_new_with_entry(self.archive, entry?)?))
            .collect()
    }
}

fn target_of(
    target: &ReferenceTarget,
    expected: &ContainerIdentity,
    chain: &[ContainerIdentity],
    kind: Option<MediaKind>,
) -> TestResult {
    require(
        matches!(&target.source, Identity::Entry { entry, containers } if entry == expected && containers == chain),
        "reference stays in the declaring physical Archive, even when paths repeat across containers",
    )?;
    require(
        target.kind == kind && target.subresource.is_none(),
        "directory edges carry exact provider scope without claiming a clip identity",
    )
}

#[derive(Default)]
pub(super) struct Edges {
    documents: BTreeMap<usize, bool>,
    observed: BTreeSet<[u64; 4]>,
}

impl Edges {
    pub(super) fn document(&mut self, physical: usize, valid: bool) {
        self.documents.insert(physical, valid);
    }

    pub(super) fn observe(&mut self, entry: usize, [element, attribute]: [u64; 2], target: usize) {
        self.observed
            .insert([entry as u64, element, attribute, target as u64]);
    }

    pub(super) fn owns(row: &MediaRecord, chain: &[ContainerIdentity]) -> bool {
        if !matches!(
            row.subresource.namespace.as_str(),
            "animation-definition-sub-database-edge"
                | "animation-definition-sub-database-target-frontier"
                | "animation-definition-sub-database-cycle"
        ) {
            return false;
        }
        match (chain.split_last(), &row.source.identity) {
            (None, Identity::Root) => true,
            (Some((container, prefix)), Identity::Entry { containers, entry }) => {
                container == entry && prefix == containers
            }
            _ => false,
        }
    }

    pub(super) fn verify(&mut self, row: &MediaRecord) -> TestResult {
        require(
            row.status == p4kx_media::Status::Complete,
            "directory graph replay has a diagnosed loss",
        )?;
        let [source, element, declaration] = row.subresource.indices.as_slice() else {
            return require(
                row.subresource.namespace == "animation-definition-sub-database-cycle",
                "graph directory rows retain physical member coordinates",
            );
        };
        match row.subresource.namespace.as_str() {
            "animation-definition-sub-database-edge" => {
                let target = decimal(&row.data, "/payload/target_entry_index")?;
                require(
                    self.observed
                        .remove(&[*source, *element, *declaration, target]),
                    "graph edge must reproduce a proven physical file link",
                )?;
                require(
                    decimal(&row.data, "/payload/source_entry_index")? == *source
                        && decimal(&row.data, "/payload/source_element_index")? == *element
                        && decimal(&row.data, "/payload/source_declaration_ordinal")?
                            == *declaration,
                    "graph edge payload is physically addressable",
                )?;
                require(
                    self.documents.get(&usize::try_from(target)?) == Some(&true),
                    "proven graph edge has a readable scoped target root",
                )
            }
            "animation-definition-sub-database-target-frontier" => {
                let expected = self
                    .observed
                    .iter()
                    .find(|edge| {
                        edge.first_chunk::<3>() == Some(&[*source, *element, *declaration])
                    })
                    .copied()
                    .ok_or("target frontier must correspond to a physical directory selector")?;
                let [_, _, _, target] = expected;
                require(
                    self.documents.get(&usize::try_from(target)?) == Some(&false),
                    "frontier target retains independently unknown root semantics",
                )?;
                self.observed.remove(&expected);
                Ok(())
            }
            _ => Err("unexpected graph directory row".into()),
        }
    }

    pub(super) fn finish(&self) -> TestResult {
        require(
            self.observed.is_empty(),
            "all selected scoped physical SubADB edges have a graph row or frontier",
        )
    }
}

pub(super) fn decimal(value: &Value, path: &str) -> TestResult<u64> {
    Ok(value
        .pointer(path)
        .and_then(Value::as_str)
        .ok_or("expected decimal wire coordinate")?
        .parse()?)
}

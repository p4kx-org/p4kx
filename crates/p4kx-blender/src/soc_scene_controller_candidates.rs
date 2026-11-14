// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_assembly::{
    SocContainerCapture, SocGeometryTarget, archive_entry_candidates, visit_geometry_sources,
};
use p4kx_extraction::{
    Archive, CancelToken, DataForge, EntryOrigin, PathIndex, PathIndexBuildError, PathIndexError,
    PathKeys, RecordTree, SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses,
    SourceValue, SourcedValue, TreeBuilder, Value,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_classes::check;
use crate::soc_scene_geometry::directory_entry;

#[path = "soc_scene_controller_candidate.rs"]
mod wire;

use wire::{CandidateInput, ControllerCandidate};

const CLASS: &str = "EntityClassDefinition";
const CONTROLLER: &str = "SAnimationControllerParams";

#[derive(Default)]
pub(crate) struct GeometryEntries(BTreeSet<usize>);

impl GeometryEntries {
    pub(crate) fn read(
        capture: &SocContainerCapture,
        archive: &Archive,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        let mut entries = BTreeSet::new();
        for reference in capture
            .occurrences()
            .iter()
            .flat_map(|occurrence| &occurrence.streams)
            .flat_map(|stream| stream.geometry.iter())
        {
            check(cancel)?;
            if let SocGeometryTarget::Matched { entry, .. } = &reference.target
                && is_cga(entry.entry().path())
                && belongs_to_root(entry, archive)
            {
                entries.insert(entry.entry().index());
            }
        }
        Ok(Self(entries))
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn controllers(
        &self,
        forge: &DataForge<'_>,
        archive: &Archive,
        scope: &SourceScope,
        sources: &SourceUses,
        cancel: &CancelToken,
    ) -> Result<ControllerIndex, SocSceneError> {
        let mut result = ControllerIndex {
            available: true,
            ..ControllerIndex::default()
        };
        if self.is_empty() {
            return Ok(result);
        }
        result.inventory = Some(scope.at(
            SourceLocation::Metadata {
                field: "geometryControllerClassInventory".into(),
            },
            SourceUseRole::Selector,
        )?);
        let Some(probe) = forge.pointer_probe("Components", CONTROLLER) else {
            return Ok(result);
        };
        let keys = PathKeys::try_new_with(archive, |_| check(cancel)).map_err(index_error)?;
        let scanner = Scanner {
            scope,
            sources,
            cancel,
            directory: keys.index(archive),
            eligible: self,
            directory_receipt: sources.observe_root_directory_paths(archive)?,
        };
        let mut trees = TreeBuilder::new();
        for record in forge.records() {
            check(cancel)?;
            match record.type_name() {
                Ok(CLASS) => {}
                Ok(_) => continue,
                Err(_) => {
                    result.incomplete = true;
                    continue;
                }
            }
            match trees.holds(&probe, &record) {
                Ok(true) => match trees.build(&record) {
                    Ok(tree) => scanner.inspect(&tree, &mut result)?,
                    Err(_) => result.incomplete = true,
                },
                Ok(false) => {}
                Err(_) => result.incomplete = true,
            }
        }
        Ok(result)
    }
}

#[derive(Default)]
pub(crate) struct ControllerIndex {
    by_entry: BTreeMap<usize, Vec<LocatedCandidate>>,
    inventory: Option<SourceUse>,
    available: bool,
    incomplete: bool,
}

struct LocatedCandidate {
    value: ControllerCandidate,
    selectors: Vec<SourceUse>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(super) enum Candidates {
    GeometryUnresolved,
    OutsideRootDataforge,
    NonCga,
    ControllerSourceUnavailable,
    ControllerSourceIncomplete {
        candidates: Vec<SourcedValue<ControllerCandidate>>,
    },
    NoControllerObserved,
    Candidate {
        candidate: Box<SourcedValue<ControllerCandidate>>,
    },
    Ambiguous {
        candidates: Vec<SourcedValue<ControllerCandidate>>,
    },
}

impl ControllerIndex {
    pub(super) fn candidates(
        &self,
        target: &SocGeometryTarget,
        sources: &SourceUses,
        selector: &SourceUse,
    ) -> Result<SourcedValue<Candidates>, SocSceneError> {
        let mut uses = vec![selector.clone()];
        let SocGeometryTarget::Matched { entry, .. } = target else {
            return Ok(sources.value(
                SourceValue::Present(Candidates::GeometryUnresolved),
                &uses,
                false,
            ));
        };
        directory_entry(entry, sources, &mut uses)?;
        let (value, complete) = if !is_cga(entry.entry().path()) {
            (Candidates::NonCga, true)
        } else if entry.ancestors().len() != 0 {
            (Candidates::OutsideRootDataforge, true)
        } else if !self.available {
            (Candidates::ControllerSourceUnavailable, false)
        } else {
            uses.extend(self.inventory.iter().cloned());
            (
                self.classify(entry.entry().index(), sources, &mut uses),
                !self.incomplete,
            )
        };
        Ok(sources.value(SourceValue::Present(value), &uses, complete))
    }

    fn classify(
        &self,
        index: usize,
        sources: &SourceUses,
        uses: &mut Vec<SourceUse>,
    ) -> Candidates {
        let observed = self.by_entry.get(&index).map_or(&[][..], Vec::as_slice);
        let mut candidates = observed
            .iter()
            .map(|row| {
                let mut local = uses.clone();
                local.extend(row.selectors.iter().cloned());
                sources.value(SourceValue::Present(row.value.clone()), &local, true)
            })
            .collect::<Vec<_>>();
        uses.extend(
            observed
                .iter()
                .flat_map(|row| row.selectors.iter().cloned()),
        );
        if self.incomplete {
            return Candidates::ControllerSourceIncomplete { candidates };
        }
        match candidates.pop() {
            None => Candidates::NoControllerObserved,
            Some(candidate) if candidates.is_empty() => Candidates::Candidate {
                candidate: Box::new(candidate),
            },
            Some(last) => {
                candidates.push(last);
                Candidates::Ambiguous { candidates }
            }
        }
    }
}

struct Scanner<'a, 'archive> {
    scope: &'a SourceScope,
    sources: &'a SourceUses,
    cancel: &'a CancelToken,
    directory: PathIndex<'archive>,
    eligible: &'a GeometryEntries,
    directory_receipt: Option<SourceUse>,
}

impl Scanner<'_, '_> {
    fn inspect(
        &self,
        tree: &RecordTree<'_>,
        out: &mut ControllerIndex,
    ) -> Result<(), SocSceneError> {
        let Some(Value::Array(components)) = tree.field("Components") else {
            out.incomplete = true;
            return Ok(());
        };
        for (ordinal, component) in components.iter().enumerate() {
            check(self.cancel)?;
            if let Value::Struct(component) = component
                && component.type_name() == CONTROLLER
            {
                self.inspect_controller(tree, ordinal, component, out)?;
            } else if matches!(component, Value::Error(_)) {
                out.incomplete = true;
            }
        }
        Ok(())
    }

    fn inspect_controller(
        &self,
        tree: &RecordTree<'_>,
        ordinal: usize,
        controller: &p4kx_extraction::StructValue<'_>,
        out: &mut ControllerIndex,
    ) -> Result<(), SocSceneError> {
        visit_geometry_sources(tree, |model| {
            check(self.cancel)?;
            let original = match model.model() {
                Some(Value::String(text)) if is_cga(text.trim()) => *text,
                Some(Value::Error(_)) => {
                    out.incomplete = true;
                    return Ok(());
                }
                _ => return Ok(()),
            };
            let Some(entry) = self.lookup(original)? else {
                return Ok(());
            };
            let index = entry.index();
            if !self.eligible.0.contains(&index) {
                return Ok(());
            }
            let entry = self
                .directory
                .archive()
                .entry_origin(entry)
                .ok_or(SocSceneError::Position)?;
            let mut directory_uses = self.directory_receipt.iter().cloned().collect::<Vec<_>>();
            directory_uses.push(
                self.scope
                    .at(model.model_location(), SourceUseRole::Selector)?,
            );
            let directory = directory_entry(&entry, self.sources, &mut directory_uses)?;
            let directory = self.sources.value(
                SourceValue::Present(directory),
                &directory_uses,
                self.directory_receipt.is_some(),
            );
            let (value, selectors) = ControllerCandidate::read(CandidateInput {
                scope: self.scope,
                sources: self.sources,
                record: tree.record_index(),
                guid: tree.guid().to_string(),
                geometry: &model,
                geometry_path: original,
                directory,
                ordinal,
                controller,
            })?;
            let values = out.by_entry.entry(index).or_default();
            values.try_reserve(1)?;
            values.push(LocatedCandidate { value, selectors });
            Ok(())
        })
    }

    fn lookup(&self, path: &str) -> Result<Option<&p4kx_extraction::Entry>, SocSceneError> {
        if crate::soc_scene_class_resources::resource_path(
            path,
            crate::soc_scene_class_resources::ResourceKind::DataRelative,
        )
        .is_none()
        {
            return Ok(None);
        }
        for path in archive_entry_candidates(path.trim()) {
            let mut matching = self
                .directory
                .matching_entries_with(&path, |_| check(self.cancel))
                .map_err(index_error)?;
            if let Some(entry) = matching.next().transpose().map_err(index_error)? {
                return Ok(
                    if matching.next().transpose().map_err(index_error)?.is_none() {
                        Some(entry)
                    } else {
                        None
                    },
                );
            }
        }
        Ok(None)
    }
}

fn index_error(error: PathIndexBuildError<SocSceneError>) -> SocSceneError {
    match error {
        PathIndexBuildError::Interrupted(error) => error,
        PathIndexBuildError::Index(PathIndexError::Allocation(error)) => error.into(),
        _ => SocSceneError::Position,
    }
}

fn belongs_to_root(entry: &EntryOrigin, archive: &Archive) -> bool {
    entry.ancestors().len() == 0 && entry.input_origin().is_same_input(archive.input_origin())
}

fn is_cga(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("cga"))
}

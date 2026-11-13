// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{SocArchiveMember, SocCaptureError};
use std::sync::OnceLock;

use p4kx_extraction::cryxml::{SourceDocument, SourceReadOptions};
use p4kx_extraction::{
    Archive, CancelToken, Entry, EntryOrigin, PathIndex, SourceLocation, SourceUse, SourceUseRole,
    SourceUses, SourceValue, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_classes::check;
use crate::soc_scene_origin::XmlOrigin;

const MANNEQUIN_ROOT: &str = "Data/Animations/Mannequin/ADB/";

pub(crate) struct Resources<'a> {
    archive: &'a Archive,
    index: OnceLock<PathIndex<'a>>,
    pub(crate) sources: &'a SourceUses,
    pub(crate) cancel: &'a CancelToken,
}

#[derive(Clone, Copy)]
pub(crate) enum ResourceKind {
    Mannequin,
    Loadout,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum Resource<T> {
    EmptySelector,
    InvalidSelector,
    Missing,
    Ambiguous {
        candidates: Vec<p4kx_extraction::PhysicalSourceRef>,
    },
    Unreadable {
        source: p4kx_extraction::PhysicalSourceRef,
    },
    InvalidXml {
        source: p4kx_extraction::PhysicalSourceRef,
    },
    Present {
        source: p4kx_extraction::PhysicalSourceRef,
        data: T,
    },
}

impl<'a> Resources<'a> {
    pub(crate) const fn new(
        archive: &'a Archive,
        sources: &'a SourceUses,
        cancel: &'a CancelToken,
    ) -> Self {
        Self {
            archive,
            index: OnceLock::new(),
            sources,
            cancel,
        }
    }

    pub(crate) fn read<T>(
        &self,
        value: SourceValue<String>,
        selector: &SourceUse,
        kind: ResourceKind,
        project: impl FnOnce(&SourceDocument<'_>, &XmlOrigin<'_>) -> Result<Option<T>, SocSceneError>,
    ) -> Result<SourcedValue<Resource<T>>, SocSceneError> {
        let mut uses = vec![selector.clone()];
        let state = match value {
            SourceValue::Present(path) => {
                SourceValue::Present(self.project(&path, kind, &mut uses, project)?)
            }
            SourceValue::Absent => SourceValue::Absent,
            SourceValue::Null => SourceValue::Null,
            SourceValue::Unreadable => SourceValue::Unreadable,
            SourceValue::Unsupported => SourceValue::Unsupported,
            _ => SourceValue::Invalid,
        };
        let complete = !matches!(
            state,
            SourceValue::Unreadable | SourceValue::Present(Resource::Unreadable { .. })
        );
        Ok(self.sources.value(state, &uses, complete))
    }

    fn project<T>(
        &self,
        authored: &str,
        kind: ResourceKind,
        uses: &mut Vec<SourceUse>,
        project: impl FnOnce(&SourceDocument<'_>, &XmlOrigin<'_>) -> Result<Option<T>, SocSceneError>,
    ) -> Result<Resource<T>, SocSceneError> {
        check(self.cancel)?;
        if authored.is_empty() {
            return Ok(Resource::EmptySelector);
        }
        let Some(path) = resource_path(authored, kind) else {
            return Ok(Resource::InvalidSelector);
        };
        let index = self.index.get_or_init(|| PathIndex::new(self.archive));
        let matches = index
            .matching_entries_with(&path, |_| check(self.cancel))
            .map_err(path_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(path_error)?;
        uses.extend(self.sources.observe_root_directory_paths(self.archive)?);
        match matches.as_slice() {
            [] => Ok(Resource::Missing),
            [entry] => self.project_entry(entry, uses, project),
            entries => {
                let candidates = entries
                    .iter()
                    .map(|entry| self.entry_origin(entry))
                    .collect::<Result<Vec<_>, _>>()?;
                let candidates = candidates
                    .iter()
                    .map(|entry| {
                        let origin = XmlOrigin::new(self.sources, entry, None)?;

                        Ok::<_, SocSceneError>(origin.source())
                    })
                    .collect::<Result<_, _>>()?;
                Ok(Resource::Ambiguous { candidates })
            }
        }
    }

    fn project_entry<T>(
        &self,
        entry: &Entry,
        uses: &mut Vec<SourceUse>,
        project: impl FnOnce(&SourceDocument<'_>, &XmlOrigin<'_>) -> Result<Option<T>, SocSceneError>,
    ) -> Result<Resource<T>, SocSceneError> {
        let member = SocArchiveMember::read(self.archive, entry, || {
            if self.cancel.is_cancelled() {
                Err(SocCaptureError::Cancelled)
            } else {
                Ok(())
            }
        })?;
        let origin = XmlOrigin::new(self.sources, member.origin(), None)?;
        let source = origin.source();
        let payload = || {
            origin.scope.at(
                SourceLocation::Metadata {
                    field: "selectedXmlPayload".into(),
                },
                SourceUseRole::Payload,
            )
        };
        let Ok(bytes) = member.payload() else {
            uses.push(origin.scope.at(
                SourceLocation::Metadata {
                    field: "selectedXmlPayload".into(),
                },
                SourceUseRole::Attempted,
            )?);
            return Ok(Resource::Unreadable { source });
        };
        let Ok(document) = SourceDocument::parse(bytes, SourceReadOptions::default()) else {
            uses.push(payload()?);
            return Ok(Resource::InvalidXml { source });
        };
        uses.push(origin.node(document.root(), SourceUseRole::Payload)?);
        match project(&document, &origin)? {
            Some(data) => Ok(Resource::Present { source, data }),
            None => Ok(Resource::InvalidXml { source }),
        }
    }

    fn entry_origin(&self, entry: &Entry) -> Result<EntryOrigin, SocSceneError> {
        self.archive
            .entry_origin(entry)
            .ok_or(SocSceneError::Position)
    }
}

fn resource_path(authored: &str, kind: ResourceKind) -> Option<String> {
    let canonical = authored.trim().replace('\\', "/");
    if canonical.is_empty()
        || canonical.split('/').any(|part| {
            part.is_empty()
                || part == "."
                || part == ".."
                || part.chars().any(|c| c == ':' || c.is_control())
        })
    {
        return None;
    }
    let first = canonical.split('/').next()?;
    Some(if first.eq_ignore_ascii_case("Data") {
        canonical
    } else if matches!(kind, ResourceKind::Mannequin) && !first.eq_ignore_ascii_case("Animations") {
        format!("{MANNEQUIN_ROOT}{canonical}")
    } else {
        format!("Data/{canonical}")
    })
}

fn path_error(error: p4kx_extraction::PathIndexBuildError<SocSceneError>) -> SocSceneError {
    match error {
        p4kx_extraction::PathIndexBuildError::Interrupted(error) => error,
        _ => SocSceneError::Position,
    }
}

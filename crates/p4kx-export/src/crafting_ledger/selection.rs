// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForgeError, Filter, Record};

use crate::crafting_quantization::{QUANTIZATIONS, RESOURCE_TYPES};
use crate::crafting_records::{
    self, DISMANTLES, DISTRIBUTIONS, GLOBALS, OVERRIDES, POOLS, PROPERTIES,
};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{SourceDisposition, SourceObservation};
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::record_observation::RecordObservation;
use crate::selection::{self, BlueprintSelection, Kind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Action {
    Blueprint,
    Property,
    Distribution,
    Quantization,
    Pool,
    Dismantle,
    Globals,
    ResourceLinks,
}

impl Action {
    pub(super) const fn kind(self) -> Option<Kind> {
        match self {
            Self::Blueprint => Some(Kind::Blueprints),
            Self::Property => Some(Kind::GameplayProperties),
            Self::Distribution => Some(Kind::QualityDistributions),
            Self::Quantization | Self::ResourceLinks => Some(Kind::QualityQuantizations),
            Self::Pool => Some(Kind::BlueprintPools),
            Self::Dismantle => Some(Kind::DismantleProcesses),
            Self::Globals => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReadPolicy {
    Read,
    SkipBefore,
    SkipAfter,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Selection {
    pub(super) action: Action,
    pub(super) role: ExportSourceRole,
    pub(super) policy: ReadPolicy,
}

impl Selection {
    pub(super) fn observation(
        self,
        admission: &RecordObservation<'_, '_>,
        disposition: SourceDisposition,
    ) -> Result<SourceObservation, crate::CatalogError> {
        admission.source(self.action.kind(), self.role, disposition)
    }

    pub(super) const fn successful_disposition(self) -> SourceDisposition {
        match self.policy {
            ReadPolicy::Read => SourceDisposition::Projected,
            ReadPolicy::SkipBefore => SourceDisposition::SkippedBeforeRead,
            ReadPolicy::SkipAfter => SourceDisposition::SkippedAfterRead,
        }
    }
}

fn selected(
    action: Action,
    record: &Record<'_>,
    path: &str,
    filter: Option<&Filter>,
) -> Result<Selection, DataForgeError> {
    let dependency = matches!(action, Action::Globals | Action::ResourceLinks)
        || filter.is_some_and(|filter| !filter.matches(path));
    let role = if dependency {
        ExportSourceRole::Dependency
    } else {
        ExportSourceRole::Selected
    };
    let policy = if dependency {
        ReadPolicy::Read
    } else {
        let name = record.name()?;
        if action
            .kind()
            .is_some_and(|kind| selection::is_kind_leftover(kind, path, name))
        {
            ReadPolicy::SkipAfter
        } else {
            ReadPolicy::Read
        }
    };
    Ok(Selection {
        action,
        role,
        policy,
    })
}

fn other(path: &str, name: &str) -> Option<Action> {
    if DISMANTLES.selects(path, name) {
        Some(Action::Dismantle)
    } else if PROPERTIES.selects(path, name) {
        Some(Action::Property)
    } else if DISTRIBUTIONS.selects(path, name) || OVERRIDES.selects(path, name) {
        Some(Action::Distribution)
    } else if QUANTIZATIONS.selects(path, name) {
        Some(Action::Quantization)
    } else if POOLS.selects(path, name) {
        Some(Action::Pool)
    } else if GLOBALS.selects(path, name) {
        Some(Action::Globals)
    } else {
        None
    }
}

pub(super) fn choose(
    record: &Record<'_>,
    filter: Option<&Filter>,
) -> Result<Option<Selection>, DataForgeError> {
    choose_observed(record, filter, |_, _| {})
}

pub(super) fn choose_observed(
    record: &Record<'_>,
    filter: Option<&Filter>,
    mut observed: impl FnMut(RecordHeaderInput, bool),
) -> Result<Option<Selection>, DataForgeError> {
    let path = record.record_path();
    observed(RecordHeaderInput::NormalizedPath, path.is_ok());
    let path = path?;
    if RESOURCE_TYPES.matches_path(path) {
        return resource_selection(record, path, filter, observed);
    }
    if !crafting_records::is_crafting_path(path) {
        return Ok(None);
    }
    if Kind::Blueprints.records().matches_path(path)
        && filter.is_some_and(|filter| !filter.matches(path))
    {
        return Ok(None);
    }
    match selection::selects_blueprint(record, filter)? {
        BlueprintSelection::Kept => {
            return selected(Action::Blueprint, record, path, filter).map(Some);
        }
        BlueprintSelection::Leftover => {
            return Ok(Some(Selection {
                action: Action::Blueprint,
                role: ExportSourceRole::Selected,
                policy: ReadPolicy::SkipBefore,
            }));
        }
        BlueprintSelection::Outside => {}
    }
    let Some(action) = other(path, record.type_name()?) else {
        return Ok(None);
    };
    if action == Action::Pool && filter.is_some_and(|filter| !filter.matches(path)) {
        return Ok(None);
    }
    selected(action, record, path, filter).map(Some)
}

fn resource_selection(
    record: &Record<'_>,
    path: &str,
    filter: Option<&Filter>,
    mut observed: impl FnMut(RecordHeaderInput, bool),
) -> Result<Option<Selection>, DataForgeError> {
    let source_type = record.type_name();
    observed(
        RecordHeaderInput::Field(RecordHeaderField::SourceType),
        source_type.is_ok(),
    );
    if source_type? != RESOURCE_TYPES.type_name {
        return Ok(None);
    }
    let name = record.name();
    observed(
        RecordHeaderInput::Field(RecordHeaderField::RecordName),
        name.is_ok(),
    );
    if selection::is_leftover(path, name?) {
        return Ok(None);
    }
    selected(Action::ResourceLinks, record, path, filter).map(Some)
}

pub(super) fn candidate_kind(record: &Record<'_>) -> Option<Kind> {
    let path = record.record_path().ok()?;
    if RESOURCE_TYPES.matches_path(path) {
        return Some(Kind::QualityQuantizations);
    }
    [
        Kind::Blueprints,
        Kind::GameplayProperties,
        Kind::QualityDistributions,
        Kind::QualityQuantizations,
        Kind::BlueprintPools,
        Kind::DismantleProcesses,
    ]
    .into_iter()
    .find(|kind| kind.rule().is_some_and(|rule| rule.matches_path(path)))
}

pub(super) fn outside(record: &Record<'_>, filter: Option<&Filter>) -> bool {
    let (Some(filter), Ok(path)) = (filter, record.record_path()) else {
        return false;
    };
    !filter.matches(path)
        && !record
            .type_name()
            .is_ok_and(|name| GLOBALS.selects(path, name) || RESOURCE_TYPES.selects(path, name))
}

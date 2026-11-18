// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForgeError, Filter, Record, RecordTree, SourceUseRole};

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::gameplay_property_catalog::{self as gameplay, GameplayPropertyInput};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::ExportLedger;
use crate::quality_catalog::evidence::Evidence;
use crate::quality_catalog::{Input, Retention, Step, scan};
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::record_observation::RecordObservation;

use super::selection::{self, Action, ReadPolicy, Selection};

#[derive(Default)]
pub(super) struct CraftingRules<'source> {
    pub(super) rules: crate::blueprint_rule_inputs::BlueprintRules,
    pub(super) global: Option<crate::catalog_v3::crafting_global::PreparedCraftingGlobal<'source>>,
}

/// Independent current consumers of the shared crafting selection and canonical read.
#[derive(Clone, Copy)]
pub(crate) enum ReadMode<'context, 'source> {
    Interpreted,
    Catalog {
        projection: &'context CatalogProjection<'context, 'source>,
        #[cfg(test)]
        search_reward_sources: bool,
    },
}

impl<'context, 'source> ReadMode<'context, 'source> {
    pub(crate) const fn catalog(
        projection: &'context CatalogProjection<'context, 'source>,
    ) -> Self {
        Self::Catalog {
            projection,
            #[cfg(test)]
            search_reward_sources: true,
        }
    }

    #[cfg(test)]
    pub(crate) const fn resource_oracle(
        projection: &'context CatalogProjection<'context, 'source>,
    ) -> Self {
        Self::Catalog {
            projection,
            search_reward_sources: false,
        }
    }

    pub(super) const fn projection(self) -> Option<&'context CatalogProjection<'context, 'source>> {
        match self {
            Self::Interpreted => None,
            Self::Catalog { projection, .. } => Some(projection),
        }
    }

    #[cfg(test)]
    pub(super) const fn searches_reward_sources(self) -> bool {
        match self {
            Self::Interpreted => true,
            Self::Catalog {
                search_reward_sources,
                ..
            } => search_reward_sources,
        }
    }

    pub(super) const fn input(self) -> Option<&'context OriginalCatalogInput<'context, 'source>> {
        match self.projection() {
            Some(projection) => Some(projection.original()),
            None => None,
        }
    }

    pub(super) fn admission<'read, 'record>(
        self,
        record: &'read Record<'record>,
    ) -> Result<RecordObservation<'read, 'record>, CatalogError> {
        match self {
            Self::Catalog { projection, .. } => {
                RecordObservation::capture(projection.original(), record)
            }
            Self::Interpreted => Ok(RecordObservation::accounting(record)),
        }
    }

    pub(super) const fn origins(self) -> Option<&'context CatalogOrigins> {
        match self.projection() {
            Some(projection) => Some(projection.origins()),
            None => None,
        }
    }

    pub(super) fn select(
        self,
        record: &Record<'_>,
        filter: Option<&Filter>,
    ) -> Result<(Result<Option<Selection>, DataForgeError>, Evidence), CatalogError> {
        let Some(input) = self.input() else {
            return Ok((selection::choose(record, filter), Evidence::default()));
        };
        let context = input.record_context(record)?;
        let mut inventory = Evidence::default();
        inventory.header(
            &context,
            RecordHeaderInput::Field(RecordHeaderField::RecordIndex),
            true,
        )?;
        let mut failure = None;
        let selected = selection::choose_observed(record, filter, |field, readable| {
            if failure.is_none() {
                failure = inventory.header(&context, field, readable).err();
            }
        });
        if let Some(error) = failure {
            return Err(error);
        }
        Ok((selected, inventory))
    }

    pub(super) fn project<'f>(
        self,
        selected: Selection,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
        ledger: &mut ExportLedger,
        cancel: Option<&CancelToken>,
    ) -> Result<Option<Input<'f>>, CatalogError> {
        let Some(input) = self.input() else {
            return Ok(None);
        };
        let retention = match (selected.role, selected.policy) {
            (_, ReadPolicy::SkipAfter) => Retention::Leftover,
            (ExportSourceRole::Dependency, _) => Retention::Dependency,
            _ => Retention::Selected,
        };
        match selected.action {
            Action::Distribution => scan::distribution(
                tree,
                admission,
                input,
                retention,
                selected.role,
                ledger,
                cancel,
            )
            .map(Some),
            Action::Quantization => scan::quantization(
                tree,
                admission,
                input,
                retention,
                selected.role,
                ledger,
                cancel,
            )
            .map(Some),
            Action::ResourceLinks => scan::resources(tree, input, admission, cancel).map(Some),
            _ => Ok(None),
        }
    }

    pub(super) fn property(
        self,
        selected: Selection,
        admission: &RecordObservation<'_, 'source>,
        tree: &RecordTree<'source>,
        ledger: &mut ExportLedger,
        cancel: Option<&CancelToken>,
    ) -> Result<Option<GameplayPropertyInput<'source>>, CatalogError> {
        let Some(projection) = self.projection() else {
            return Ok(None);
        };
        if selected.action != Action::Property {
            return Ok(None);
        }
        if selected.policy == ReadPolicy::SkipAfter {
            ledger.skipped_rows(crate::Kind::GameplayProperties, 1);
            return Ok(Some(GameplayPropertyInput::Leftover));
        }
        if selected.role == ExportSourceRole::Dependency {
            return Ok(Some(GameplayPropertyInput::Dependency));
        }
        let found = gameplay::read(
            tree,
            &projection.for_kind(crate::Kind::GameplayProperties),
            admission,
            cancel,
        )?;
        crate::source_projection_notes::record(
            crate::Kind::GameplayProperties,
            selected.role,
            admission,
            tree,
            found.notes,
            ledger,
        )?;
        Ok(Some(GameplayPropertyInput::Selected(Box::new(found.value))))
    }

    pub(super) fn blueprint_rules(
        self,
        action: Action,
        admission: &RecordObservation<'_, 'source>,
        tree: &RecordTree<'source>,
        ledger: &mut ExportLedger,
        cancel: Option<&CancelToken>,
    ) -> Result<CraftingRules<'source>, CatalogError> {
        use crate::blueprint_rule_inputs::BlueprintRules;
        match (self.input(), action) {
            (Some(input), Action::Globals) => {
                let found =
                    crate::catalog_v3::crafting_global::read(tree, input, admission, cancel)?;
                crate::source_projection_notes::record_dependency(
                    admission,
                    tree,
                    found.notes,
                    ledger,
                )?;
                Ok(CraftingRules {
                    rules: found.rules,
                    global: Some(found.prepared),
                })
            }
            (Some(input), Action::Dismantle) => Ok(CraftingRules {
                rules: BlueprintRules::read_dismantle(tree, admission, input, cancel)?,
                global: None,
            }),
            _ => Ok(CraftingRules::default()),
        }
    }

    pub(super) fn failed<'f>(
        self,
        record: &Record<'f>,
        action: Option<Action>,
    ) -> Result<Step<'f>, CatalogError> {
        let mut step = Step::default();
        let Some(input) = self.input() else {
            return Ok(step);
        };
        if !matches!(action, Some(Action::Quantization | Action::ResourceLinks)) {
            return Ok(step);
        }
        let context = input.record_context(record)?;
        let mut evidence = Evidence::default();
        evidence.header(
            &context,
            RecordHeaderInput::Field(RecordHeaderField::RecordIndex),
            true,
        )?;
        evidence.keep(context.observe("", SourceUseRole::Attempted)?)?;
        evidence.complete = false;
        if matches!(action, Some(Action::Quantization)) {
            step.input = Some(Input::UnavailableQuantization {
                guid: record.guid(),
                record_index: record.index(),
                evidence,
            });
        } else {
            step.inventory = evidence;
        }
        Ok(step)
    }
}

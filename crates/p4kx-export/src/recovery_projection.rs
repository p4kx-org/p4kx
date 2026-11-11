// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree, SourceValue, SourcedValue, StructValue};

use super::economy::{self, ECONOMY, ECONOMY_TYPE};
use super::overrides;
use super::policy_contract::ItemRecoveryPolicy;
use super::policy_read;
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::{CapturedNumber, NumberCapture};
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_read::{CatalogFields, CatalogRecordContext};
use crate::contract_recovery::{
    ItemRecoveryEconomy, ItemRecoveryOverrides, RecoveryEconomy, RecoveryOverride,
};
use crate::record_observation::RecordObservation;
use crate::source_fields::{FieldCapture, FieldError};
use crate::template_field_capture::TemplateFieldCapture;

pub(crate) struct RecoveryProjection<'source> {
    pub(crate) economy: ItemRecoveryEconomy<'source>,
    pub(crate) overrides: ItemRecoveryOverrides,
    pub(crate) policy: ItemRecoveryPolicy<'source>,
    pub(crate) numbers: Vec<CapturedNumber>,
    pub(crate) notes: Vec<FieldError>,
}

pub(crate) struct RecoveryReader<'read, 'source, 'context> {
    pub(crate) context: &'read CatalogRecordContext<'context>,
    pub(crate) capture: &'read dyn FieldCapture<'source>,
    pub(crate) cancel: Option<&'read CancelToken>,
}

/// Projects the selected policy's economy and override declarations from the same original read.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<RecoveryProjection<'source>, CatalogError> {
    let input = projection.original();
    let scope = input.record_scope(admission.record(), tree)?;
    let original = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&original, input, admission)?;
    let numbers = NumberCapture::default();
    let context = input
        .record_context(admission.record())?
        .with_cancel(cancel)
        .with_numbers(&numbers);
    let reader = RecoveryReader {
        context: &context,
        capture: &capture,
        cancel,
    };
    let root = CatalogFields::observed(tree.root(), "", &context, &capture)?;
    let mut notes = Vec::new();
    let policy = reader.policy(&root, &mut notes)?;
    let source = CanonicalRecordHeader::read(tree, input.origins())?;
    let authored_policy = policy_read::read(&reader, &root, &source, projection, &mut notes)?;
    let overrides = ItemRecoveryOverrides {
        owner_guid: source.guid.clone(),
        owner_record_index: source.record_index.clone(),
        overrides: policy.overrides,
    };
    Ok(RecoveryProjection {
        economy: ItemRecoveryEconomy {
            source,
            economy_params: policy.economy,
        },
        overrides,
        policy: authored_policy,
        numbers: numbers.finish(),
        notes,
    })
}

struct Policy {
    economy: SourcedValue<RecoveryEconomy>,
    overrides: SourcedValue<Vec<SourcedValue<RecoveryOverride>>>,
}

impl<'source> RecoveryReader<'_, 'source, '_> {
    fn policy(
        &self,
        root: &CatalogFields<'_, 'source, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Policy, CatalogError> {
        let input = root.structure(ECONOMY, notes)?;
        let Some(source) = input.value().copied() else {
            return Ok(Policy {
                economy: input.clone().try_map(|_| absent())?,
                overrides: input.try_map(|_| absent())?,
            });
        };
        let path = root.location(ECONOMY);
        let fields = CatalogFields::observed(source, &path, self.context, self.capture)?;
        if source.type_name() != ECONOMY_TYPE {
            fields.note_unsupported(notes)?;
            let concrete = fields.location("$type");
            return Ok(Policy {
                economy: self.context.selector(&concrete, SourceValue::Unsupported)?,
                overrides: self.context.selector(&concrete, SourceValue::Unsupported)?,
            });
        }
        let overrides = overrides::read(self, &fields, notes)?;
        let economy = input.try_map(|_| economy::read(self, &fields, notes))?;
        Ok(Policy { economy, overrides })
    }

    pub(crate) fn block<T>(
        &self,
        parent: &CatalogFields<'_, 'source, '_>,
        name: &str,
        expected: &str,
        notes: &mut Vec<FieldError>,
        project: impl FnOnce(
            &CatalogFields<'_, 'source, '_>,
            &mut Vec<FieldError>,
        ) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let path = parent.location(name);
        let value = parent.structure(name, notes)?;
        self.typed(value, &path, expected, notes, project)
    }

    pub(crate) fn typed<T>(
        &self,
        value: SourcedValue<&StructValue<'source>>,
        path: &str,
        expected: &str,
        notes: &mut Vec<FieldError>,
        project: impl FnOnce(
            &CatalogFields<'_, 'source, '_>,
            &mut Vec<FieldError>,
        ) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        if let Some(source) = value.value()
            && source.type_name() != expected
        {
            let observed = CatalogFields::observed(source, path, self.context, self.capture)?;
            observed.note_unsupported(notes)?;
            return self
                .context
                .selector(&observed.location("$type"), SourceValue::Unsupported);
        }
        value.try_map(|source| {
            let fields = CatalogFields::observed(source, path, self.context, self.capture)?;
            project(&fields, notes)
        })
    }
}

fn absent<T>() -> Result<T, CatalogError> {
    Err(crate::ExportAccountingError::ConflictingSourceDatum.into())
}

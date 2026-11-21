// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataType, Guid, Record, Reference, ReferenceOrigin, SourceUse, SourceUseRole, SourceValue,
};

use super::original_instance_capture;
use super::original_reference_admission::OriginalReferenceAdmission;
use super::{
    OriginalDatum, OriginalReferenceCapture, OriginalTargetShape, OriginalValueFact,
    OriginalValuePart,
};
use crate::record_header_field::RecordHeaderField;
use crate::{CatalogError, ExportAccountingError};

/// One actual reference admission retained together with its exact caller fact.
pub(crate) struct OriginalReferenceObservation<'fact, 'scope, 'read, 'source> {
    caller: &'fact OriginalValueFact<'scope, 'read, 'source>,
    admitted: OriginalReferenceAdmission<'read, 'source>,
}

impl<'fact, 'scope, 'read, 'source> OriginalReferenceObservation<'fact, 'scope, 'read, 'source> {
    pub(crate) const fn caller(&self) -> &'fact OriginalValueFact<'scope, 'read, 'source> {
        self.caller
    }

    pub(crate) const fn reference(&self) -> &Reference<'source> {
        self.admitted.reference
    }

    pub(crate) fn record(&self) -> Result<Record<'source>, CatalogError> {
        self.admitted
            .origin
            .record()
            .map_err(CatalogError::DataForge)
    }

    pub(crate) const fn record_index(&self) -> u32 {
        self.admitted.origin.record_index()
    }

    pub(crate) fn is_instance(&self) -> bool {
        self.admitted.origin.instance_positions().is_some()
    }

    pub(super) const fn admitted(&self) -> &OriginalReferenceAdmission<'read, 'source> {
        &self.admitted
    }

    pub(crate) fn guid(&self) -> Result<OriginalDatum<Guid>, CatalogError> {
        self.guid_with_role(SourceUseRole::Payload)
    }

    /// Retains the actual GUID inputs used to select a linked Record.
    pub(crate) fn guid_selector(&self) -> Result<OriginalDatum<Guid>, CatalogError> {
        self.guid_with_role(SourceUseRole::Selector)
    }

    fn guid_with_role(&self, role: SourceUseRole) -> Result<OriginalDatum<Guid>, CatalogError> {
        let selector = self
            .caller
            .observe(OriginalValuePart::Value, SourceUseRole::Selector)?;
        self.caller.original_guid(
            self.admitted.reference.guid(),
            self.admitted.storage,
            self.admitted.origin,
            role,
            &selector,
        )
    }

    /// Reads the admitted target path from its exact Record header.
    pub(crate) fn record_path(&self) -> Result<OriginalDatum<&'source str>, CatalogError> {
        let selector = self
            .caller
            .observe(OriginalValuePart::Value, SourceUseRole::Selector)?;
        self.caller
            .original_record_path(self.admitted.origin, &selector)
    }

    /// Tests the admitted Record against the authored Reference target declaration.
    /// The unconstrained target word contributes its declaration without testing ancestry.
    pub(crate) fn matches_record_declaration(
        &self,
        selectors: &mut Vec<SourceUse>,
        check_cancel: impl Fn() -> Result<(), CatalogError>,
    ) -> Result<bool, CatalogError> {
        let storage = self.caller.field().storage();
        if storage.data_type() != DataType::Reference || self.is_instance() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let scope = self.caller.scope().dataforge();
        selectors.push(
            self.caller
                .observe(OriginalValuePart::Target, SourceUseRole::Selector)?,
        );
        let expected = storage.target();
        if expected == u16::MAX {
            return Ok(true);
        }
        selectors.push(scope.at(
            RecordHeaderField::SourceType.location(self.record_index()),
            SourceUseRole::Selector,
        )?);
        let current = self
            .record()?
            .struct_type()
            .map_err(CatalogError::DataForge)?;
        self.caller
            .observe_struct_ancestry(current, u32::from(expected), selectors, check_cancel)
    }

    pub(crate) fn capture(self) -> Result<OriginalReferenceCapture<'source>, CatalogError> {
        self.capture_view()
    }

    pub(crate) fn capture_view(&self) -> Result<OriginalReferenceCapture<'source>, CatalogError> {
        self.caller.reference_facts(
            self.admitted.reference,
            self.admitted.origin,
            self.admitted.storage,
        )
    }
}

impl<'scope, 'read, 'source> OriginalValueFact<'scope, 'read, 'source> {
    pub(crate) fn reference_observation(
        &self,
    ) -> Result<OriginalReferenceObservation<'_, 'scope, 'read, 'source>, CatalogError> {
        Ok(OriginalReferenceObservation {
            caller: self,
            admitted: self.admit_reference()?,
        })
    }
}

impl<'source> OriginalValueFact<'_, '_, 'source> {
    fn reference_facts(
        &self,
        reference: &Reference<'source>,
        admitted: ReferenceOrigin<'_, 'source>,
        storage: DataType,
    ) -> Result<OriginalReferenceCapture<'source>, CatalogError> {
        let selector = self.observe(OriginalValuePart::Value, SourceUseRole::Selector)?;
        let shape = self.target_shape(reference, admitted, storage, &selector)?;
        let guid = self.original_guid(
            reference.guid(),
            storage,
            admitted,
            SourceUseRole::Payload,
            &selector,
        )?;
        let record_index = self.target_header(
            admitted,
            RecordHeaderField::RecordIndex,
            admitted.record_index(),
            SourceUseRole::Selector,
            &selector,
        )?;
        let record_path = self.original_record_path(admitted, &selector)?;
        Ok(OriginalReferenceCapture {
            guid,
            record_index,
            record_path,
            shape,
        })
    }

    fn target_shape(
        &self,
        reference: &Reference<'source>,
        admitted: ReferenceOrigin<'_, 'source>,
        storage: DataType,
        selector: &SourceUse,
    ) -> Result<OriginalTargetShape<'source>, CatalogError> {
        match (admitted.record_type(), admitted.instance_location()) {
            (Some(source_type), None) => Ok(OriginalTargetShape::Record {
                source_type: self.target_header(
                    admitted,
                    RecordHeaderField::SourceType,
                    source_type,
                    SourceUseRole::Selector,
                    selector,
                )?,
            }),
            (None, Some(location)) if storage == DataType::WeakPointer => {
                original_instance_capture::read(self, reference, admitted, location, selector)
            }
            _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
        }
    }

    fn original_guid(
        &self,
        configured: Guid,
        storage: DataType,
        admitted: ReferenceOrigin<'_, '_>,
        role: SourceUseRole,
        selector: &SourceUse,
    ) -> Result<OriginalDatum<Guid>, CatalogError> {
        if storage == DataType::Reference {
            if role == SourceUseRole::Selector {
                return Ok(OriginalDatum::observed(
                    SourceValue::Present(configured),
                    vec![selector.clone()],
                    true,
                ));
            }
            // The Reference decoder returns the target of this field's stored GUID.
            return self.bind(
                OriginalValuePart::Value,
                SourceValue::Present(configured),
                role,
                true,
            );
        }
        let mut guid = self.target_header(
            admitted,
            RecordHeaderField::Guid,
            admitted.record_guid(),
            role,
            selector,
        )?;
        guid.contribute(self.observe(OriginalValuePart::Value, SourceUseRole::Conversion)?);
        Ok(guid)
    }

    fn original_record_path(
        &self,
        admitted: ReferenceOrigin<'_, 'source>,
        selector: &SourceUse,
    ) -> Result<OriginalDatum<&'source str>, CatalogError> {
        self.target_header(
            admitted,
            RecordHeaderField::FileName,
            admitted.record_path(),
            SourceUseRole::Conversion,
            selector,
        )
    }

    fn target_header<T>(
        &self,
        admitted: ReferenceOrigin<'_, '_>,
        field: RecordHeaderField,
        value: T,
        role: SourceUseRole,
        selector: &SourceUse,
    ) -> Result<OriginalDatum<T>, CatalogError> {
        let header = self
            .scope()
            .dataforge()
            .at(field.location(admitted.record_index()), role)?;
        Ok(OriginalDatum::observed(
            SourceValue::Present(value),
            vec![header, selector.clone()],
            true,
        ))
    }
}

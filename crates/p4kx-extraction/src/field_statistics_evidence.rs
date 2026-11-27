// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Binds the original operands of one observed DataForge field-statistics operation.

use std::ops::Range;

use p4kx_dataforge::query::{QueryByteUse, QuerySource};
use p4kx_dataforge::{
    DataType, FieldStatisticsGap, FieldStatisticsInput, FieldStatisticsObservation,
    FieldStatisticsOperand, FieldStatisticsRouteTarget, FieldStatisticsSlot,
    FieldStatisticsSourceScope, FieldStatisticsUse, RecordJsonSource,
};

use crate::dataforge_layout_evidence;
use crate::query_source_evidence::QuerySourceSite;
use crate::source_provenance::RecordJsonLocation;
use crate::{SourceEvidenceError, SourceLocation, SourceScope, SourceUse, SourceUseRole};

#[path = "field_statistics_scalar_sources.rs"]
mod scalar_sources;

/// Registered source uses for one actual field-statistics observation.
#[derive(Debug, Default)]
pub struct FieldStatisticsSourceUses {
    selectors: Vec<SourceUse>,
    operands: Vec<SourceUse>,
    decoder_uses: Vec<SourceUse>,
    attempts: Vec<SourceUse>,
}

impl FieldStatisticsSourceUses {
    /// Registers the observed local and enclosing sites in the same original DataForge scope.
    ///
    /// # Errors
    ///
    /// Returns the error of registering a source use in the bound inventory.
    pub fn bind(
        scope: &SourceScope,
        observation: &FieldStatisticsObservation<'_>,
    ) -> Result<Self, SourceEvidenceError> {
        let mut binder = Binder {
            scope,
            sites: Self::default(),
        };
        for edge in observation.enclosing_route() {
            binder.selector(edge.selector())?;
            binder.branch(edge.selector(), edge.target())?;
        }
        binder.local(observation.input(), observation.usage())?;
        Ok(binder.sites)
    }

    /// Registers the retained original payload of a reduced comparison operand.
    ///
    /// # Errors
    ///
    /// Returns an error if the original source inventory cannot register the use.
    pub fn bind_operand(
        scope: &SourceScope,
        operand: &FieldStatisticsOperand,
    ) -> Result<Self, SourceEvidenceError> {
        let mut binder = Binder {
            scope,
            sites: Self::default(),
        };
        binder.selector(operand.input())?;
        binder.scalar_operand(operand.input(), operand.text_span())?;
        binder.scalar_read(operand.input(), operand.scalar_read())?;
        Ok(binder.sites)
    }

    /// Returns the eligible selector uses for the observed path and local admission.
    #[must_use]
    pub fn selectors(&self) -> &[SourceUse] {
        &self.selectors
    }

    /// Returns the physical value operands actually decoded and counted.
    #[must_use]
    pub fn operands(&self) -> &[SourceUse] {
        &self.operands
    }

    /// Returns original decoder inputs, separate from path selectors and decoded payload.
    #[must_use]
    pub fn decoder_uses(&self) -> &[SourceUse] {
        &self.decoder_uses
    }

    /// Returns located read attempts, separately from successful contributors.
    #[must_use]
    pub fn attempts(&self) -> &[SourceUse] {
        &self.attempts
    }

    /// Returns the remaining aggregate selection and path coverage requirement.
    #[must_use]
    pub const fn source_scope(&self) -> FieldStatisticsSourceScope {
        FieldStatisticsSourceScope::SelectionAndPathsUnbound
    }
}

struct Binder<'scope> {
    scope: &'scope SourceScope,
    sites: FieldStatisticsSourceUses,
}

impl Binder<'_> {
    fn branch(
        &mut self,
        selector: &FieldStatisticsInput,
        target: FieldStatisticsRouteTarget,
    ) -> Result<(), SourceEvidenceError> {
        if storage(selector) == Some(DataType::Class) {
            self.target_property(selector)?;
        }
        match target {
            FieldStatisticsRouteTarget::Instance {
                owner_structure,
                owner_instance,
                layout_structure,
                ..
            } => {
                if storage(selector) == Some(DataType::StrongPointer) {
                    self.bytes(
                        payload(selector),
                        QueryByteUse::Pointer,
                        SourceUseRole::Selector,
                    )?;
                }
                self.query_selector(QuerySource::Instance {
                    structure: owner_structure,
                    instance: owner_instance,
                })?;
                self.query_selector(QuerySource::Structure {
                    index: layout_structure,
                })
            }
            _ => Ok(()),
        }
    }

    fn local(
        &mut self,
        input: &FieldStatisticsInput,
        usage: &FieldStatisticsUse,
    ) -> Result<(), SourceEvidenceError> {
        if matches!(
            usage,
            FieldStatisticsUse::Skipped(FieldStatisticsGap::MissingRecord)
        ) {
            return Ok(());
        }
        self.selector(input)?;
        match usage {
            FieldStatisticsUse::Instance {
                structure,
                instance,
                path_name,
                ..
            } => {
                self.query_selector(QuerySource::Instance {
                    structure: *structure,
                    instance: *instance,
                })?;
                self.bytes(
                    path_name.as_ref(),
                    QueryByteUse::Text,
                    SourceUseRole::Selector,
                )?;
            }
            FieldStatisticsUse::Record {
                structure,
                instance,
            } => {
                self.query_selector(QuerySource::Structure { index: *structure })?;
                self.query_selector(QuerySource::Instance {
                    structure: *structure,
                    instance: *instance,
                })?;
            }
            FieldStatisticsUse::Layout(source) => dataforge_layout_evidence::bind(
                source,
                self.scope,
                &mut self.sites.selectors,
                &mut self.sites.attempts,
            )?,
            FieldStatisticsUse::Counted {
                value, scalar_read, ..
            } => {
                self.operand(input, value)?;
                self.scalar_read(input, *scalar_read)?;
            }
            FieldStatisticsUse::Skipped(gap) => self.attempt(input, *gap)?,
            _ => {}
        }
        Ok(())
    }

    fn attempt(
        &mut self,
        input: &FieldStatisticsInput,
        gap: FieldStatisticsGap,
    ) -> Result<(), SourceEvidenceError> {
        let field = match gap {
            FieldStatisticsGap::UnreadableValue => {
                return self.bytes(
                    payload(input),
                    QueryByteUse::Value,
                    SourceUseRole::Attempted,
                );
            }
            FieldStatisticsGap::InvalidArrayExtent => "arrayExtent",
            FieldStatisticsGap::MissingInstance => "instance",
            FieldStatisticsGap::UnreadableLayout => "layout",
            _ => return Ok(()),
        };
        let coordinate = match input {
            FieldStatisticsInput::Record(index) => {
                QuerySourceSite::record(*index, field, SourceUseRole::Attempted)
            }
            FieldStatisticsInput::ArrayElement {
                header, pool_index, ..
            } => QuerySourceSite {
                location: SourceLocation::Subresource {
                    namespace: "dataforgeArrayElement".into(),
                    indices: vec![
                        u64::from(header.owner_structure),
                        u64::from(header.owner_instance),
                        header.slot_position,
                        u64::from(*pool_index),
                    ],
                    field: field.into(),
                },
                role: SourceUseRole::Attempted,
            },
            FieldStatisticsInput::Slot(slot) => QuerySourceSite {
                location: SourceLocation::Subresource {
                    namespace: "dataforgeInstance".into(),
                    indices: vec![
                        u64::from(slot.owner_structure),
                        u64::from(slot.owner_instance),
                        slot.slot_position,
                    ],
                    field: field.into(),
                },
                role: SourceUseRole::Attempted,
            },
            _ => return Ok(()),
        };
        self.sites.attempts.push(coordinate.bind(self.scope)?);
        Ok(())
    }

    fn selector(&mut self, input: &FieldStatisticsInput) -> Result<(), SourceEvidenceError> {
        match input {
            FieldStatisticsInput::Slot(slot) => self.slot(slot)?,
            FieldStatisticsInput::ArrayElement { header, index, .. } => {
                self.slot(header)?;
                if let Some(span) = &header.bytes {
                    self.query_selector(QuerySource::ArrayElement {
                        header_start: span.start,
                        index: *index,
                    })?;
                }
            }
            FieldStatisticsInput::Record(index) => {
                self.query_selector(QuerySource::Record { index: *index })?;
                let source = QuerySourceSite::record(*index, "sourceType", SourceUseRole::Selector);
                self.sites.selectors.push(source.bind(self.scope)?);
            }
            _ => {}
        }
        Ok(())
    }

    fn slot(&mut self, slot: &FieldStatisticsSlot) -> Result<(), SourceEvidenceError> {
        if slot.kind.is_array() {
            self.bytes(
                slot.bytes.as_ref(),
                QueryByteUse::Array,
                SourceUseRole::Selector,
            )?;
        }
        self.query_selector(QuerySource::Record {
            index: slot.record_index,
        })?;
        self.query_selector(QuerySource::Instance {
            structure: slot.owner_structure,
            instance: slot.owner_instance,
        })?;
        self.query_selector(QuerySource::Structure {
            index: slot.layout_structure,
        })?;
        if let Some(property) = RecordJsonLocation::of(RecordJsonSource::PropertyStorage {
            index: slot.property_index,
            target_used: false,
        }) {
            self.sites
                .selectors
                .push(self.scope.at(property.location, SourceUseRole::Selector)?);
        }
        self.bytes(
            slot.name.as_ref(),
            QueryByteUse::Text,
            SourceUseRole::Selector,
        )
    }

    fn target_property(&mut self, input: &FieldStatisticsInput) -> Result<(), SourceEvidenceError> {
        if let Some(site) = property_target(input) {
            self.sites
                .selectors
                .push(self.scope.at(site, SourceUseRole::Selector)?);
        }
        Ok(())
    }

    fn query_selector(&mut self, source: QuerySource) -> Result<(), SourceEvidenceError> {
        if let Some(site) = QuerySourceSite::of(source, SourceUseRole::Selector) {
            self.sites.selectors.push(site.bind(self.scope)?);
        }
        Ok(())
    }

    fn bytes(
        &mut self,
        span: Option<&Range<u64>>,
        kind: QueryByteUse,
        role: SourceUseRole,
    ) -> Result<(), SourceEvidenceError> {
        let Some(span) = span else {
            return Ok(());
        };
        let source = QuerySource::Bytes {
            start: span.start,
            end: span.end,
            kind,
        };
        if let Some(site) = QuerySourceSite::of(source, role) {
            let registered = site.bind(self.scope)?;
            match role {
                SourceUseRole::Selector => self.sites.selectors.push(registered),
                SourceUseRole::Payload => self.sites.operands.push(registered),
                SourceUseRole::Attempted => self.sites.attempts.push(registered),
                _ => {}
            }
        }
        Ok(())
    }
}

pub(super) const fn storage(input: &FieldStatisticsInput) -> Option<DataType> {
    match input {
        FieldStatisticsInput::Slot(slot) => Some(slot.kind.data_type()),
        FieldStatisticsInput::ArrayElement { header, .. } => Some(header.kind.data_type()),
        _ => None,
    }
}

fn property_target(input: &FieldStatisticsInput) -> Option<SourceLocation> {
    let index = match input {
        FieldStatisticsInput::Slot(slot) => slot.property_index,
        FieldStatisticsInput::ArrayElement { header, .. } => header.property_index,
        _ => return None,
    };
    RecordJsonLocation::of(RecordJsonSource::PropertyStorage {
        index,
        target_used: true,
    })
    .map(|site| site.location)
}

const fn payload(input: &FieldStatisticsInput) -> Option<&Range<u64>> {
    match input {
        FieldStatisticsInput::Slot(slot) => slot.bytes.as_ref(),
        FieldStatisticsInput::ArrayElement { bytes, .. } => bytes.as_ref(),
        _ => None,
    }
}

#[cfg(test)]
#[path = "field_statistics_evidence_tests.rs"]
mod tests;

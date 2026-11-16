// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, FieldOrigins, RecordJsonLocation, SourceScope, SourceUses};

use crate::field_change_evidence::{Coordinates, OperandEvidence};
use crate::record_report::FieldChangeReport;
use crate::report_origins::{ReportOriginsError, put};
use crate::value_report::ValueReport;
use crate::{ChangedValue, FieldChange, RecordSide, Value};

pub(crate) struct BodyOrigins<'a> {
    pub(crate) uses: &'a SourceUses,
    pub(crate) old: Option<&'a SourceScope>,
    pub(crate) new: Option<&'a SourceScope>,
}

impl BodyOrigins<'_> {
    pub(crate) fn write(
        &self,
        fields: &mut FieldOrigins,
        path: &str,
        change: &FieldChange<'_>,
        reported: &FieldChangeReport<'_>,
    ) -> Result<(), ReportOriginsError> {
        let evidence = change.evidence.as_deref();
        let location = evidence.map(|body| &body.path);
        put(fields, &format!("{path}/path"), self.origin(location)?)?;
        for (name, wire, original, operand) in [
            (
                "old",
                reported.old.as_ref(),
                change.old_value(),
                evidence.and_then(|body| body.old.as_ref()),
            ),
            (
                "new",
                reported.new.as_ref(),
                change.new_value(),
                evidence.and_then(|body| body.new.as_ref()),
            ),
        ] {
            if let Some(wire) = wire {
                self.side(fields, &format!("{path}/{name}"), wire, original, operand)?;
            }
        }
        Ok(())
    }

    fn side(
        &self,
        fields: &mut FieldOrigins,
        path: &str,
        wire: &ValueReport<'_>,
        original: Option<&ChangedValue<'_>>,
        operand: Option<&OperandEvidence>,
    ) -> Result<(), ReportOriginsError> {
        let kind = self.origin(
            operand
                .into_iter()
                .flat_map(|proof| [&proof.route, &proof.kind]),
        )?;
        put(fields, &format!("{path}/type"), kind)?;
        match wire {
            ValueReport::Null => Ok(()),
            ValueReport::Reference(_) => self.reference(fields, path, original),
            _ => {
                let value = self.origin(
                    operand
                        .into_iter()
                        .flat_map(|proof| [&proof.route, &proof.value]),
                )?;
                put(fields, &format!("{path}/value"), value)
            }
        }
    }

    fn reference(
        &self,
        fields: &mut FieldOrigins,
        path: &str,
        original: Option<&ChangedValue<'_>>,
    ) -> Result<(), ReportOriginsError> {
        let unknown = self.uses.origin(&[], false);
        for member in ["guid", "record_path", "type"] {
            put(fields, &format!("{path}/value/{member}"), unknown.clone())?;
        }
        if matches!(
            original,
            Some(ChangedValue::Value(Value::Reference(target))) if target.location().is_some()
        ) {
            put(fields, &format!("{path}/value/at"), unknown)?;
        }
        Ok(())
    }

    fn origin<'a>(
        &self,
        coordinates: impl IntoIterator<Item = &'a Coordinates>,
    ) -> Result<FieldOrigin, ReportOriginsError> {
        let mut complete = true;
        let mut observations = Vec::new();
        for set in coordinates {
            complete &= set.complete;
            for input in &set.uses {
                let scope = match input.side {
                    RecordSide::Old => self.old,
                    RecordSide::New => self.new,
                };
                let fact = RecordJsonLocation::of(input.coordinate);
                if let Some((scope, location)) = scope.zip(fact) {
                    observations.push(scope.at(location.location, input.role)?);
                } else {
                    complete = false;
                }
            }
        }
        let complete = complete && !observations.is_empty();
        Ok(self.uses.origin(&observations, complete))
    }
}

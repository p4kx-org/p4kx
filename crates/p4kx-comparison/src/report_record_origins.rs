// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataForgeOrigin, FieldOrigins, InputScope, RecordJsonLocation, RecordJsonRecordField,
    RecordJsonSource, SourceEvidenceError, SourceInput, SourceScope, SourceUse, SourceUses,
};

use crate::report_origins::{ReportOriginFields, ReportOriginsError, put};
use crate::report_record_body::BodyOrigins;
use crate::{Comparison, RecordState};

use super::RecordChangeReport;

pub(super) fn capture(
    comparison: &Comparison<'_>,
    reported: &[RecordChangeReport<'_>],
) -> Result<ReportOriginFields, ReportOriginsError> {
    let uses = SourceUses::with_compact_use_refs();
    let scopes = [
        scope(&uses, comparison.record_sources.old.as_ref())?,
        scope(&uses, comparison.record_sources.new.as_ref())?,
    ];
    let [old_scope, new_scope] = scopes.each_ref().map(Option::as_ref);
    let mut fields = FieldOrigins::new();
    let body = BodyOrigins {
        uses: &uses,
        old: old_scope,
        new: new_scope,
    };
    for (position, (change, row)) in comparison.record_changes().iter().zip(reported).enumerate() {
        let path = format!("/records/changes/{position}");
        let key = change.key();
        let old = key.old_position().map(|index| Header {
            index,
            scope: old_scope,
        });
        let new = key.new_position().map(|index| Header {
            index,
            scope: new_scope,
        });
        if let Some(header) = new.or(old) {
            let evidence = header.word(RecordJsonRecordField::Guid)?;
            put(
                &mut fields,
                &format!("{path}/guid"),
                uses.origin(evidence.as_slice(), header.scope.is_some()),
            )?;
        }
        if let Some((header, state)) = old.zip(change.old_record()) {
            header.write(&uses, &mut fields, &format!("{path}/old"), state)?;
        }
        if let Some((header, state)) = new.zip(change.new_record()) {
            header.write(&uses, &mut fields, &format!("{path}/new"), state)?;
        }
        for (index, (field, reported)) in change.fields().iter().zip(&row.fields).enumerate() {
            body.write(
                &mut fields,
                &format!("{path}/fields/{index}"),
                field,
                reported,
            )?;
        }
    }
    ReportOriginFields::finish(fields, &uses)
}

fn scope(
    uses: &SourceUses,
    origin: Option<&DataForgeOrigin>,
) -> Result<Option<SourceScope>, SourceEvidenceError> {
    match origin {
        Some(DataForgeOrigin::Archive(entry)) => uses
            .observe(SourceInput::Archive(entry), InputScope::Payload)
            .map(Some),
        _ => Ok(None),
    }
}

#[derive(Clone, Copy)]
struct Header<'a> {
    index: u32,
    scope: Option<&'a SourceScope>,
}

impl Header<'_> {
    fn word(self, field: RecordJsonRecordField) -> Result<Option<SourceUse>, SourceEvidenceError> {
        self.observation(RecordJsonSource::Record {
            index: self.index,
            field,
        })
    }

    fn observation(
        self,
        observation: RecordJsonSource<'_>,
    ) -> Result<Option<SourceUse>, SourceEvidenceError> {
        self.scope
            .map(|scope| {
                let fact = RecordJsonLocation::of(observation)
                    .ok_or(SourceEvidenceError::ForeignEvidence)?;
                scope.at(fact.location, fact.role)
            })
            .transpose()
    }

    fn write(
        self,
        uses: &SourceUses,
        fields: &mut FieldOrigins,
        path: &str,
        state: RecordState<'_>,
    ) -> Result<(), ReportOriginsError> {
        for (output, author) in [
            ("record_path", RecordJsonRecordField::FileName),
            ("name", RecordJsonRecordField::Name),
        ] {
            let observation = self.word(author)?;
            put(
                fields,
                &format!("{path}/{output}"),
                uses.origin(observation.as_slice(), self.scope.is_some()),
            )?;
        }
        if state.tag().is_some() {
            let tag = self.word(RecordJsonRecordField::Tag)?;
            put(
                fields,
                &format!("{path}/tag"),
                uses.origin(tag.as_slice(), self.scope.is_some()),
            )?;
        }
        let structure = self.word(RecordJsonRecordField::Struct)?;
        let name = self.observation(RecordJsonSource::StructName {
            index: state.structure,
        })?;
        let definition: Vec<_> = structure.into_iter().chain(name).collect();
        put(
            fields,
            &format!("{path}/type"),
            uses.origin(&definition, self.scope.is_some()),
        )
    }
}

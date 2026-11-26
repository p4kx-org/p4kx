// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::DataType;
use p4kx_extraction::SourceEvidenceError;

use crate::report_origins::ReportOriginsError;
use crate::schema_report_origins::{OriginWriter, SchemaSource};
use crate::schema_side::StructDef;
use crate::schema_witness::SchemaWitness;
use crate::schema_witness::Sites;

const CHANGE_KINDS: [&str; 3] = ["added", "removed", "changed"];

pub(crate) fn write(
    writer: &mut OriginWriter<'_>,
    sides: [SchemaSource<'_, '_>; 2],
) -> Result<(), ReportOriginsError> {
    let mut census = DeclarationCensus::default();
    for (side, side_name) in sides.into_iter().zip(["old", "new"]) {
        census.observe_side(writer, side, side_name)?;
    }
    census.write_counts(writer, sides.into_iter().all(SchemaSource::survey_complete))
}

#[derive(Default)]
struct DeclarationCensus {
    struct_names: Sites,
    struct_members: Sites,
    struct_shapes: Sites,
    enum_names: Sites,
    enum_members: Sites,
}

impl DeclarationCensus {
    fn observe_side(
        &mut self,
        writer: &mut OriginWriter<'_>,
        side: SchemaSource<'_, '_>,
        side_name: &str,
    ) -> Result<(), ReportOriginsError> {
        let witness = side.witness;
        let structs = witness.struct_names()?;
        let enums = witness.enum_names()?;
        let members = vec![
            witness.structure_headers("propertyRange")?,
            witness.property_headers("name")?,
        ];
        let options = enum_options(side)?;
        let parents = witness.structure_headers("parent")?;
        let mut failures = witness.record_types()?;
        failures.extend(
            structs
                .iter()
                .chain(&enums)
                .chain(&members)
                .chain(&options)
                .cloned(),
        );
        failures.push(parents.clone());
        writer.bind(
            &format!("/unreadable/{side_name}"),
            &failures,
            side.survey_complete(),
        )?;

        self.struct_shapes.extend([
            parents,
            witness.property_headers("dataType")?,
            witness.property_headers("conversion")?,
            witness.property_headers("target")?,
        ]);
        self.struct_shapes.extend(enums.iter().cloned());
        self.struct_names.extend(structs);
        self.struct_members.extend(members);
        self.enum_names.extend(enums);
        self.enum_members.extend(options);
        Ok(())
    }

    fn write_counts(
        &self,
        writer: &mut OriginWriter<'_>,
        complete: bool,
    ) -> Result<(), ReportOriginsError> {
        let struct_changes = combined(&[
            &self.struct_names,
            &self.struct_members,
            &self.struct_shapes,
        ]);
        let property_members = combined(&[&self.struct_names, &self.struct_members]);
        let enum_members = combined(&[&self.enum_names, &self.enum_members]);
        for kind in CHANGE_KINDS {
            let (structures, properties, enums) = if kind == "changed" {
                (&struct_changes, &struct_changes, &enum_members)
            } else {
                (&self.struct_names, &property_members, &self.enum_names)
            };
            writer.bind(&format!("/counts/structs/{kind}"), structures, complete)?;
            writer.bind(&format!("/counts/properties/{kind}"), properties, complete)?;
            writer.bind(&format!("/counts/enums/{kind}"), enums, complete)?;
            if kind != "changed" {
                writer.bind(&format!("/counts/options/{kind}"), &enum_members, complete)?;
            }
        }
        writer.bind("/counts/options/changed", &[], true)?;
        Ok(())
    }
}

pub(crate) fn record_sites(
    side: SchemaSource<'_, '_>,
    enumeration: bool,
) -> Result<Sites, SourceEvidenceError> {
    let witness = side.witness;
    let mut records = witness.record_types()?;
    for &index in &side.schema.record_type_names {
        records.push(witness.struct_name(index)?);
    }
    for name in &side.schema.record_lineage {
        if let Some(definition) = side.schema.structs.get(name) {
            records.push(witness.struct_name(definition.declaration.index())?);
            records.extend(witness.parent(definition.declaration)?);
            if enumeration {
                record_enum_sites(witness, definition, &mut records)?;
            }
        }
    }
    Ok(records)
}

fn record_enum_sites(
    witness: &SchemaWitness,
    definition: &StructDef<'_>,
    records: &mut Sites,
) -> Result<(), SourceEvidenceError> {
    records.extend(witness.property_names(definition.declaration)?);
    for property in definition.properties.values() {
        let declaration = property.declaration;
        if matches!(declaration.data_type(), Ok(DataType::Enum)) {
            records.extend(witness.target(declaration)?);
        } else {
            records.push(witness.data_type(declaration)?);
        }
    }
    Ok(())
}

fn enum_options(side: SchemaSource<'_, '_>) -> Result<Sites, SourceEvidenceError> {
    let witness = side.witness;
    let mut sites = vec![witness.enum_headers("optionRange")?];
    for definition in side.schema.enums.values() {
        sites.extend(witness.option_names(definition.declaration)?);
    }
    Ok(sites)
}

fn combined(slices: &[&Sites]) -> Sites {
    slices
        .iter()
        .flat_map(|sites| sites.iter().cloned())
        .collect()
}

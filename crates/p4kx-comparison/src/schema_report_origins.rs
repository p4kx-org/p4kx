// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::slice;

use p4kx_dataforge::Property;
use p4kx_extraction::{FieldOrigins, SourceEvidenceError, SourceUse, SourceUses};

use crate::report_origins::{ReportOriginFields, ReportOriginsError, put};
use crate::schema_origin_aggregates;
use crate::schema_record_origins::{RecordFamily, RecordOrigins};
use crate::schema_side::{EnumDef, SideSchema, StructDef};
use crate::schema_types::{EnumChange, PropertyAspect, PropertyChange, SchemaDiff, StructChange};
use crate::schema_witness::{SchemaWitness, Sites};

const SIDE_NAMES: [SideName; 2] = [SideName::Old, SideName::New];

#[derive(Clone, Copy)]
enum SideName {
    Old,
    New,
}

impl SideName {
    const fn key(self) -> &'static str {
        match self {
            Self::Old => "old",
            Self::New => "new",
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct SchemaSource<'a, 'f> {
    pub(crate) schema: &'a SideSchema<'f>,
    pub(crate) witness: &'a SchemaWitness,
}

impl SchemaSource<'_, '_> {
    pub(crate) const fn survey_complete(self) -> bool {
        self.witness.complete && self.schema.unreadable == 0
    }
}

pub(crate) fn capture(
    diff: &SchemaDiff,
    uses: &SourceUses,
    sides: [SchemaSource<'_, '_>; 2],
) -> Result<ReportOriginFields, ReportOriginsError> {
    let mut fields = FieldOrigins::new();
    let mut writer = OriginWriter {
        uses,
        fields: &mut fields,
        records: [
            RecordOrigins::capture(uses, sides[0], diff)?,
            RecordOrigins::capture(uses, sides[1], diff)?,
        ],
    };
    for (index, change) in diff.structs.iter().enumerate() {
        writer.structure(&format!("/structs/{index}"), change, sides)?;
    }
    for (index, change) in diff.enums.iter().enumerate() {
        writer.enumeration(&format!("/enums/{index}"), change, sides)?;
    }
    schema_origin_aggregates::write(&mut writer, sides)?;
    ReportOriginFields::finish(fields, uses)
}

struct Decision {
    sites: Sites,
    complete: bool,
}

pub(crate) struct OriginWriter<'a> {
    uses: &'a SourceUses,
    fields: &'a mut FieldOrigins,
    records: [RecordOrigins; 2],
}

impl OriginWriter<'_> {
    pub(crate) fn bind(
        &mut self,
        path: &str,
        sites: &[SourceUse],
        complete: bool,
    ) -> Result<(), ReportOriginsError> {
        put(self.fields, path, self.uses.origin(sites, complete))
    }

    fn bind_record(
        &mut self,
        path: &str,
        side_name: SideName,
        family: RecordFamily,
        name: &SourceUse,
    ) -> Result<(), ReportOriginsError> {
        let [old, new] = &self.records;
        let survey = match side_name {
            SideName::Old => old,
            SideName::New => new,
        };
        let origin = survey.origin(self.uses, family, name)?;
        put(self.fields, path, origin)
    }

    fn structure(
        &mut self,
        path: &str,
        change: &StructChange,
        sides: [SchemaSource<'_, '_>; 2],
    ) -> Result<(), ReportOriginsError> {
        let owners = sides.map(|side| side.schema.structs.get(change.name.as_str()));
        let mut decision = Sites::new();
        for ((side, owner), side_name) in sides.into_iter().zip(owners).zip(SIDE_NAMES) {
            decision.extend(self.structure_side(path, change, side_name, side, owner)?);
        }
        let (side, owner) = first_owner(sides, owners)?;
        let name = side.witness.struct_name(owner.declaration.index())?;
        self.bind(
            &format!("{path}/name"),
            slice::from_ref(&name),
            side.witness.complete,
        )?;
        let mut reliable = complete(sides);
        if change.parent.is_some() {
            reliable &= sides.into_iter().zip(owners).all(|(side, owner)| {
                owner.is_none_or(|owner| side.witness.parent_complete(owner.declaration))
            });
        }
        for (index, property) in change.properties.iter().enumerate() {
            let property = self.property(
                &format!("{path}/properties/{index}"),
                property,
                sides,
                owners,
            )?;
            if matches!(change.change, crate::SchemaChangeKind::Changed) {
                decision.extend(property.sites);
                reliable &= property.complete;
            }
        }
        self.bind(&format!("{path}/change"), &decision, reliable)
    }

    fn structure_side(
        &mut self,
        path: &str,
        change: &StructChange,
        side_key: SideName,
        side: SchemaSource<'_, '_>,
        owner: Option<&StructDef<'_>>,
    ) -> Result<Sites, ReportOriginsError> {
        let side_name = side_key.key();
        let witness = side.witness;
        let Some(owner) = owner else {
            let names = witness.struct_names()?;
            self.bind(
                &format!("{path}/records/{side_name}"),
                &names,
                side.survey_complete(),
            )?;
            self.bind(
                &format!("{path}/mapped_instances/{side_name}"),
                &names,
                side.survey_complete(),
            )?;
            return Ok(names);
        };
        let declaration = owner.declaration;
        let name = witness.struct_name(declaration.index())?;
        self.bind_record(
            &format!("{path}/records/{side_name}"),
            side_key,
            RecordFamily::Struct,
            &name,
        )?;
        let mut decision = vec![name];
        let mappings = witness.mapped_instances(declaration.index())?;
        self.bind(
            &format!("{path}/mapped_instances/{side_name}"),
            &mappings,
            witness.complete,
        )?;
        if change.parent.is_some() {
            let parent = witness.parent(declaration)?;
            self.bind(
                &format!("{path}/parent/{side_name}"),
                &parent,
                witness.parent_complete(declaration),
            )?;
            decision.extend(parent);
        }
        Ok(decision)
    }

    fn property(
        &mut self,
        path: &str,
        change: &PropertyChange,
        sides: [SchemaSource<'_, '_>; 2],
        owners: [Option<&StructDef<'_>>; 2],
    ) -> Result<Decision, ReportOriginsError> {
        let properties = owners.map(|owner| {
            owner
                .and_then(|owner| owner.properties.get(change.name.as_str()))
                .map(|def| def.declaration)
        });
        let (side, property) = first_owner(sides, properties.each_ref().map(Option::as_ref))?;
        let name = side.witness.property_name(*property)?;
        self.bind(
            &format!("{path}/name"),
            slice::from_ref(&name),
            side.witness.complete,
        )?;

        let mut decision = Decision {
            sites: Sites::new(),
            complete: complete(sides),
        };
        for (((side, owner), property), side_name) in sides
            .into_iter()
            .zip(owners)
            .zip(properties)
            .zip(SIDE_NAMES)
        {
            let path = format!("{path}/{}", side_name.key());
            decision
                .sites
                .extend(self.property_side(&path, side, owner, property)?);
            decision.complete &=
                property.is_none_or(|property| side.witness.target_complete(property));
        }
        self.bind(
            &format!("{path}/change"),
            &decision.sites,
            decision.complete,
        )?;
        self.bind(
            &format!("{path}/aspects"),
            &decision.sites,
            decision.complete,
        )?;
        for (index, aspect) in change.aspects.iter().enumerate() {
            let mut operands = Sites::new();
            for (side, property) in sides.into_iter().zip(properties) {
                let property = property.ok_or(SourceEvidenceError::ForeignEvidence)?;
                match aspect {
                    PropertyAspect::DataType => operands.push(side.witness.data_type(property)?),
                    PropertyAspect::Conversion => operands.push(side.witness.conversion(property)?),
                    PropertyAspect::Target => operands.extend(side.witness.target(property)?),
                }
            }
            self.bind(
                &format!("{path}/aspects/{index}"),
                &operands,
                decision.complete,
            )?;
        }
        Ok(decision)
    }

    fn property_side(
        &mut self,
        path: &str,
        side: SchemaSource<'_, '_>,
        owner: Option<&StructDef<'_>>,
        property: Option<Property<'_>>,
    ) -> Result<Sites, ReportOriginsError> {
        let witness = side.witness;
        let Some(property) = property else {
            let names = match owner {
                Some(owner) => witness.property_names(owner.declaration)?,
                None => witness.struct_names()?,
            };
            return Ok(names);
        };
        let data_type = witness.data_type(property)?;
        let conversion = witness.conversion(property)?;
        let target = witness.target(property)?;
        self.bind(
            &format!("{path}/data_type"),
            slice::from_ref(&data_type),
            witness.complete,
        )?;
        self.bind(
            &format!("{path}/conversion"),
            slice::from_ref(&conversion),
            witness.complete,
        )?;
        self.bind(
            &format!("{path}/target"),
            &target,
            witness.target_complete(property),
        )?;
        let mut sites = vec![witness.property_name(property)?, data_type, conversion];
        sites.extend(target);
        Ok(sites)
    }

    fn enumeration(
        &mut self,
        path: &str,
        change: &EnumChange,
        sides: [SchemaSource<'_, '_>; 2],
    ) -> Result<(), ReportOriginsError> {
        let owners = sides.map(|side| side.schema.enums.get(change.name.as_str()));
        let (side, owner) = first_owner(sides, owners)?;
        let name = side.witness.enum_name(owner.declaration.index())?;
        self.bind(
            &format!("{path}/name"),
            slice::from_ref(&name),
            side.witness.complete,
        )?;
        let mut decision = Sites::new();
        for ((side, owner), side_name) in sides.into_iter().zip(owners).zip(SIDE_NAMES) {
            decision.extend(self.enumeration_side(path, change, side_name, side, owner)?);
        }
        self.bind(&format!("{path}/change"), &decision, complete(sides))?;
        for (index, option) in change.options.iter().enumerate() {
            let mut choice = Sites::new();
            let path = format!("{path}/options/{index}");
            for (side, owner) in sides.into_iter().zip(owners) {
                let owner = owner.ok_or(SourceEvidenceError::ForeignEvidence)?;
                if let Some(&original) = owner.options.get(option.name.as_str()) {
                    let name = side.witness.option_name(original)?;
                    self.bind(
                        &format!("{path}/name"),
                        slice::from_ref(&name),
                        side.witness.complete,
                    )?;
                    choice.push(name);
                } else {
                    choice.extend(side.witness.option_names(owner.declaration)?);
                }
            }
            self.bind(&format!("{path}/change"), &choice, complete(sides))?;
        }
        Ok(())
    }

    fn enumeration_side(
        &mut self,
        path: &str,
        change: &EnumChange,
        side_key: SideName,
        side: SchemaSource<'_, '_>,
        owner: Option<&EnumDef<'_>>,
    ) -> Result<Sites, ReportOriginsError> {
        let side_name = side_key.key();
        let witness = side.witness;
        let Some(owner) = owner else {
            let names = witness.enum_names()?;
            self.bind(
                &format!("{path}/records/{side_name}"),
                &names,
                side.survey_complete(),
            )?;
            return Ok(names);
        };
        let name = witness.enum_name(owner.declaration.index())?;
        self.bind_record(
            &format!("{path}/records/{side_name}"),
            side_key,
            RecordFamily::Enum,
            &name,
        )?;
        let mut decision = vec![name];
        if !change.options.is_empty() {
            decision.extend(witness.option_names(owner.declaration)?);
        }
        Ok(decision)
    }
}

pub(crate) fn complete(sides: [SchemaSource<'_, '_>; 2]) -> bool {
    sides.into_iter().all(SchemaSource::survey_complete)
}

fn first_owner<'a, 'f, 'd, T>(
    sides: [SchemaSource<'a, 'f>; 2],
    inputs: [Option<&'d T>; 2],
) -> Result<(SchemaSource<'a, 'f>, &'d T), ReportOriginsError> {
    sides
        .into_iter()
        .zip(inputs)
        .find_map(|(side, input)| input.map(|input| (side, input)))
        .ok_or_else(|| SourceEvidenceError::ForeignEvidence.into())
}

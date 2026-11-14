// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;
use std::slice;

use p4kx_extraction::{DataForgeOrigin, SourceEvidenceError, SourceUse, SourceUseRole};

use crate::forge_origins::{EvidenceError, ForgeOrigins, site};
use crate::forge_shapes::ForgeFieldOrigins;

#[derive(Clone, Copy)]
pub(crate) enum Kind {
    Structs,
    Enums,
}

impl Kind {
    pub(crate) const fn list(self) -> &'static str {
        match self {
            Self::Structs => "structs",
            Self::Enums => "enums",
        }
    }

    pub(crate) const fn singular(self) -> &'static str {
        match self {
            Self::Structs => "struct",
            Self::Enums => "enum",
        }
    }

    const fn namespace(self) -> &'static str {
        match self {
            Self::Structs => "dataforgeStructDefinition",
            Self::Enums => "dataforgeEnumDefinition",
        }
    }
}

pub(crate) enum Declaration {
    Struct {
        index: u32,
        parent: Option<u32>,
        range: Range<usize>,
        properties: Vec<PropertySite>,
    },
    Enum {
        index: u32,
        options: Range<usize>,
    },
}

pub(crate) struct PropertySite {
    pub(crate) index: usize,
    pub(crate) target: Option<Target>,
}

#[derive(Clone, Copy)]
pub(crate) enum Target {
    Struct(u32),
    Enum(u32),
}

pub(crate) fn project<'a>(
    origin: &DataForgeOrigin,
    kind: Kind,
    definitions: usize,
    filtered: bool,
    cursor: bool,
    rows: impl IntoIterator<Item = &'a Declaration>,
) -> Result<ForgeFieldOrigins, EvidenceError> {
    let mut origins = ForgeOrigins::new(origin)?;
    origins.put("archive", origins.uses.origin(&[], true))?;
    let count = site(
        &origins.scope,
        kind.namespace(),
        "definitionCount",
        Vec::new(),
        SourceUseRole::Selector,
    )?;
    let mut navigation = vec![count];
    if (filtered || cursor) && definitions != 0 {
        let names = site(
            &origins.scope,
            kind.namespace(),
            "definitionNames",
            vec![0, definitions as u64],
            SourceUseRole::Selector,
        )?;
        if filtered {
            navigation.push(names.clone());
        }
        origins.bind("total", &navigation)?;
        if cursor && !filtered {
            navigation.push(names);
        }
    } else {
        origins.bind("total", &navigation)?;
    }
    if cursor {
        origins.bind("next_cursor", &navigation)?;
    }
    let mut page = Vec::new();
    for (position, row) in rows.into_iter().enumerate() {
        let path = format!("{}/{position}", kind.list());
        match row {
            Declaration::Struct {
                index,
                parent,
                range,
                properties,
            } => {
                page.extend(origins.structure(&path, *index, *parent, range, properties)?);
            }
            Declaration::Enum { index, options } => {
                page.extend(origins.enumeration(&path, *index, options.clone())?);
            }
        }
    }
    origins.put(
        kind.list(),
        origins
            .uses
            .origin(&page, page.is_empty() || origins.complete),
    )?;
    origins.finish()
}

impl ForgeOrigins {
    fn structure(
        &mut self,
        path: &str,
        index: u32,
        parent: Option<u32>,
        range: &Range<usize>,
        properties: &[PropertySite],
    ) -> Result<Vec<SourceUse>, EvidenceError> {
        let mut row = Vec::new();
        let name = self.definition_name(Kind::Structs, index)?;
        self.bind(&format!("{path}/name"), slice::from_ref(&name))?;
        row.push(name);
        if let Some(parent) = parent {
            let selected = site(
                &self.scope,
                Kind::Structs.namespace(),
                "parent",
                vec![u64::from(index)],
                SourceUseRole::Selector,
            )?;
            let name = self.definition_name(Kind::Structs, parent)?;
            let uses = [selected, name];
            self.bind(&format!("{path}/parent"), &uses)?;
            row.extend(uses);
        }
        let field_path = format!("{path}/fields");
        let range = site(
            &self.scope,
            Kind::Structs.namespace(),
            "propertyRange",
            vec![u64::from(index), range.start as u64, range.end as u64],
            SourceUseRole::Selector,
        )?;
        let mut fields = vec![range];
        for (ordinal, property) in properties.iter().enumerate() {
            fields.extend(self.property(&format!("{field_path}/{ordinal}"), property)?);
        }
        self.bind(&field_path, &fields)?;
        row.extend(fields);
        Ok(row)
    }

    fn property(
        &mut self,
        path: &str,
        property: &PropertySite,
    ) -> Result<Vec<SourceUse>, EvidenceError> {
        let mut uses = Vec::new();
        for (output, field, role) in [
            ("name", "name", SourceUseRole::Payload),
            ("type", "dataType", SourceUseRole::Conversion),
            ("array", "conversion", SourceUseRole::Conversion),
        ] {
            let value = self.property_word(property.index, field, role)?;
            self.bind(&format!("{path}/{output}"), slice::from_ref(&value))?;
            uses.push(value);
        }
        if let Some(target) = property.target {
            let (output, kind, index) = match target {
                Target::Struct(index) => ("target", Kind::Structs, index),
                Target::Enum(index) => ("enum", Kind::Enums, index),
            };
            let target_word =
                self.property_word(property.index, "target", SourceUseRole::Selector)?;
            let type_word =
                self.property_word(property.index, "dataType", SourceUseRole::Selector)?;
            let name = self.definition_name(kind, index)?;
            let selected = [target_word, type_word, name];
            self.bind(&format!("{path}/{output}"), &selected)?;
            uses.extend(selected);
        }
        Ok(uses)
    }

    fn enumeration(
        &mut self,
        path: &str,
        index: u32,
        options: Range<usize>,
    ) -> Result<Vec<SourceUse>, EvidenceError> {
        let name = self.definition_name(Kind::Enums, index)?;
        self.bind(&format!("{path}/name"), slice::from_ref(&name))?;
        let selected = site(
            &self.scope,
            Kind::Enums.namespace(),
            "optionRange",
            vec![u64::from(index), options.start as u64, options.end as u64],
            SourceUseRole::Selector,
        )?;
        let values = format!("{path}/values");
        let mut uses = vec![selected];
        for (ordinal, option) in options.enumerate() {
            let value = site(
                &self.scope,
                "dataforgeEnumOption",
                "name",
                vec![option as u64],
                SourceUseRole::Payload,
            )?;
            self.bind(&format!("{values}/{ordinal}"), slice::from_ref(&value))?;
            uses.push(value);
        }
        self.bind(&values, &uses)?;
        uses.push(name);
        Ok(uses)
    }

    fn definition_name(&self, kind: Kind, index: u32) -> Result<SourceUse, SourceEvidenceError> {
        site(
            &self.scope,
            kind.namespace(),
            "name",
            vec![u64::from(index)],
            SourceUseRole::Payload,
        )
    }

    fn property_word(
        &self,
        index: usize,
        field: &str,
        role: SourceUseRole,
    ) -> Result<SourceUse, SourceEvidenceError> {
        site(
            &self.scope,
            "dataforgePropertyDefinition",
            field,
            vec![index as u64],
            role,
        )
    }

    fn bind(&mut self, path: &str, uses: &[SourceUse]) -> Result<(), EvidenceError> {
        self.put(path, self.uses.origin(uses, self.complete))
    }
}

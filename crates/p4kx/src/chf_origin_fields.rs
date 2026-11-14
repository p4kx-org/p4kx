// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::io::{self, Write};

use anyhow::{Result, anyhow};
use p4kx_assembly::character::{ChfJsonField, ChfJsonName, ChfJsonNameUse, LABEL_SUFFIX};
use p4kx_extraction::{
    FieldOrigin, FieldOrigins, FieldPath, InputScope, ObservedFileRead, OriginArtifactBinding,
    OriginOutputAddress, SourceInput, SourceInventory, SourceLocation, SourceScope, SourceUse,
    SourceUseRole, SourceUses, write_origin_companion,
};

use crate::chf_names::ObservedNames;

#[derive(Clone, Copy)]
pub(crate) enum ChfInputFormat {
    Chf,
    Json,
}

pub(crate) struct ChfOrigins {
    uses: SourceUses,
    input: SourceScope,
    complete: bool,
    format: ChfInputFormat,
}

pub(crate) struct ProjectedChfOrigins {
    origins: Vec<(OriginOutputAddress, FieldOrigin)>,
    tally: Option<Tally>,
    size_uses: Vec<SourceUse>,
    size_complete: bool,
}

struct Tally {
    uses: Vec<SourceUse>,
    complete: bool,
    unresolved: BTreeMap<u32, Vec<SourceUse>>,
}

pub(crate) struct ChfField {
    path: String,
    name: Option<ChfJsonName>,
}

impl ChfField {
    pub(crate) fn observed(field: ChfJsonField<'_>) -> Self {
        Self {
            path: field.pointer().to_owned(),
            name: field.name(),
        }
    }
}

impl ChfOrigins {
    pub(crate) fn new(input: &ObservedFileRead, format: ChfInputFormat) -> Result<Self> {
        let uses = SourceUses::new();
        let scope = input
            .origin()
            .map_or(SourceInput::Unknown, SourceInput::File);
        let input_scope = uses.observe(scope, InputScope::Payload)?;
        Ok(Self {
            uses,
            input: input_scope,
            complete: input.origin().is_some(),
            format,
        })
    }

    pub(crate) const fn uses(&self) -> &SourceUses {
        &self.uses
    }

    pub(crate) fn json_fields(
        &self,
        fields: Vec<ChfField>,
        names: Option<&ObservedNames>,
    ) -> Result<ProjectedChfOrigins> {
        let mut origins = Vec::new();
        origins.try_reserve_exact(fields.len())?;
        let mut tally = Tally {
            uses: vec![self.at("nameHashOccurrences", SourceUseRole::Selector)?],
            complete: self.complete,
            unresolved: BTreeMap::new(),
        };
        let mut size_uses = Vec::new();
        let mut size_complete = self.complete;
        for field in fields {
            let address = OriginOutputAddress::JsonField {
                path: FieldPath::parse(&field.path)?,
            };
            let datum = self.datum(&field, names)?;
            tally.retain(&field, &datum);
            let origin = self.uses.origin(&datum.uses, datum.complete);
            origins.push((address, origin));
            size_uses.extend(datum.uses);
            size_complete &= datum.complete;
        }
        Ok(ProjectedChfOrigins {
            origins,
            tally: Some(tally),
            size_uses,
            size_complete,
        })
    }

    pub(crate) fn opaque_output(&self) -> Result<ProjectedChfOrigins> {
        let conversion = self.at("characterAndContainer", SourceUseRole::Conversion)?;
        Ok(ProjectedChfOrigins {
            origins: vec![(
                OriginOutputAddress::OpaquePayload,
                self.uses.origin(&[conversion], self.complete),
            )],
            tally: None,
            size_uses: Vec::new(),
            size_complete: true,
        })
    }

    pub(crate) fn summary_fields(&self, data: &ProjectedChfOrigins) -> Result<FieldOrigins> {
        let mut fields = FieldOrigins::new();
        let input = self.input.for_scope(InputScope::FileMetadata).at(
            SourceLocation::Metadata {
                field: "path".to_owned(),
            },
            SourceUseRole::Payload,
        )?;
        fields.insert(
            FieldPath::parse("/input")?,
            self.uses.origin(&[input], self.complete),
        )?;
        for literal in ["/output", "/originReceipt"] {
            fields.insert(FieldPath::parse(literal)?, self.uses.origin(&[], true))?;
        }
        for (path, source) in [
            ("/versions/female", "/character/versions/female"),
            ("/versions/male", "/character/versions/male"),
        ] {
            let datum = self.at(source, SourceUseRole::Payload)?;
            fields.insert(
                FieldPath::parse(path)?,
                self.uses.origin(&[datum], self.complete),
            )?;
        }
        fields.insert(
            FieldPath::parse("/size")?,
            self.uses.origin(&data.size_uses, data.size_complete),
        )?;
        if let Some(tally) = &data.tally {
            tally.summary_fields(&self.uses, &mut fields)?;
        }
        Ok(fields)
    }

    pub(crate) fn inventory(&self) -> Result<SourceInventory> {
        Ok(self.uses.freeze()?)
    }

    fn datum(&self, field: &ChfField, names: Option<&ObservedNames>) -> Result<Datum> {
        let Some(name) = field
            .name
            .filter(|name| name.usage() == ChfJsonNameUse::Label)
        else {
            return Ok(Datum {
                uses: vec![self.at(&field.path, SourceUseRole::Payload)?],
                complete: self.complete,
            });
        };
        let hash = field
            .path
            .strip_suffix(LABEL_SUFFIX)
            .ok_or_else(|| anyhow!("CHF label lacks its hash field"))?;
        let mut uses = vec![self.at(hash, SourceUseRole::Selector)?];
        let mut complete = self.complete;
        if let Some(names) = names
            && let Some(archive) = names.contributions(name, &self.input)?
        {
            uses.extend(archive.uses);
            complete &= archive.complete;
        }
        Ok(Datum { uses, complete })
    }

    fn at(&self, field: &str, role: SourceUseRole) -> Result<SourceUse> {
        let namespace = match self.format {
            ChfInputFormat::Chf => "chf",
            ChfInputFormat::Json => "chfJson",
        };
        Ok(self.input.at(
            SourceLocation::Subresource {
                namespace: namespace.to_owned(),
                indices: Vec::new(),
                field: field.to_owned(),
            },
            role,
        )?)
    }
}

impl ProjectedChfOrigins {
    pub(crate) fn write(
        &self,
        writer: &mut dyn Write,
        binding: &OriginArtifactBinding,
        inventory: &SourceInventory,
    ) -> io::Result<()> {
        write_origin_companion(
            writer,
            binding,
            inventory,
            self.origins
                .iter()
                .map(|(address, origin)| (address, origin)),
        )
    }
}

struct Datum {
    uses: Vec<SourceUse>,
    complete: bool,
}

impl Tally {
    fn retain(&mut self, field: &ChfField, datum: &Datum) {
        if let Some(name) = field.name {
            match name.usage() {
                ChfJsonNameUse::Label => {
                    self.uses.extend(datum.uses.iter().cloned());
                    self.complete &= datum.complete;
                }
                ChfJsonNameUse::UnresolvedHash => {
                    self.uses.extend(datum.uses.iter().cloned());
                    self.complete &= datum.complete;
                    self.unresolved
                        .entry(name.hash())
                        .or_default()
                        .extend(datum.uses.iter().cloned());
                }
                ChfJsonNameUse::ResolvedHash => {}
                _ => self.complete = false,
            }
        }
    }

    fn summary_fields(&self, uses: &SourceUses, fields: &mut FieldOrigins) -> Result<()> {
        let counts = uses.origin(&self.uses, self.complete);
        for count in ["/named", "/unresolved"] {
            fields.insert(FieldPath::parse(count)?, counts.clone())?;
        }
        if self.unresolved.is_empty() {
            fields.insert(FieldPath::parse("/unresolved_hashes")?, counts)?;
        } else {
            for (index, contributions) in self.unresolved.values().enumerate() {
                let path = FieldPath::parse(&format!("/unresolved_hashes/{index}"))?;
                fields.insert(path, uses.origin(contributions, self.complete))?;
            }
        }
        Ok(())
    }
}

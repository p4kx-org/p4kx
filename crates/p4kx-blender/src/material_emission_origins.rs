// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::{
    EntryOrigin, FieldOrigins, FieldOriginsSeed, FieldPath, InputScope, OriginReaderLimits,
    SourceInput, SourceInventory, SourceLocation, SourceScope, SourceUse, SourceUseRole,
    SourceUses,
};
use p4kx_model::{
    EmissionAttribute, EmissionNumber, EmissionNumeric, EmissionTextureSlot, EmissionValue,
};
use serde::de::DeserializeSeed as _;
use serde_json::Value;

use crate::{PackageError, PackageResult};

#[derive(Debug)]
pub(crate) struct EmissionEvidence {
    uses: SourceUses,
    scope: Option<SourceScope>,
    complete: bool,
}

impl EmissionEvidence {
    pub(crate) fn new(entry: Option<&EntryOrigin>) -> PackageResult<Self> {
        let uses = SourceUses::with_compact_use_refs();
        let scope = entry
            .map(|entry| uses.observe(SourceInput::Archive(entry), InputScope::Payload))
            .transpose()
            .map_err(error)?;
        Ok(Self {
            uses,
            scope,
            complete: entry.is_some_and(|entry| entry.ancestors().all(|step| step.is_some())),
        })
    }

    pub(crate) fn finish(self) -> PackageResult<Value> {
        let inventory: SourceInventory = self.uses.freeze().map_err(error)?;
        serde_json::to_value(inventory).map_err(error)
    }
}

pub(super) struct Fields<'a> {
    evidence: &'a EmissionEvidence,
    origins: FieldOrigins,
}

impl<'a> Fields<'a> {
    pub(super) const fn new(evidence: &'a EmissionEvidence) -> Self {
        Self {
            evidence,
            origins: FieldOrigins::new(),
        }
    }

    pub(super) fn finish(self) -> FieldOrigins {
        self.origins
    }

    pub(super) fn attribute(
        &mut self,
        index: usize,
        attribute: &EmissionAttribute<'_>,
    ) -> PackageResult<()> {
        let owner = format!("/attributes/{index}");
        let value = &attribute.value;
        self.token(&format!("{owner}/source"), value)?;
        let used = self.at(value, SourceUseRole::Conversion)?;
        let output = format!("{owner}/value");
        let converted = used.as_slice();
        self.bind(&output, converted, true)?;
        self.bind(&format!("{output}/kind"), converted, true)?;
        match value.interpreted {
            EmissionNumeric::Scalar(number) => {
                self.number(&format!("{output}/value"), number, converted)?;
            }
            EmissionNumeric::Colour(numbers) => {
                let parts = format!("{output}/value");
                self.bind(&parts, converted, true)?;
                for (index, number) in numbers.into_iter().enumerate() {
                    self.number(&format!("{parts}/{index}"), number, converted)?;
                }
            }
            EmissionNumeric::Flags(_) => self.bind(&format!("{output}/value"), converted, true)?,
            _ => {}
        }
        Ok(())
    }

    pub(super) fn slot(
        &mut self,
        index: usize,
        slot: &EmissionTextureSlot<'_>,
    ) -> PackageResult<()> {
        let owner = format!("/texture_slots/{index}");
        self.token(&format!("{owner}/map"), &slot.map)?;
        let file = format!("{owner}/file");
        if let Some(value) = &slot.file {
            self.token(&file, value)
        } else {
            let at = self.element_at(&slot.map.element, "File", SourceUseRole::Selector)?;
            self.bind(&file, at.as_slice(), false)
        }
    }

    pub(super) fn derived<'value, 'source: 'value>(
        &mut self,
        path: &str,
        attributes: impl Iterator<Item = &'value EmissionAttribute<'source>>,
        element: &[usize],
        complete: bool,
    ) -> PackageResult<()> {
        let mut used = Vec::new();
        for attribute in attributes {
            used.extend(self.at(&attribute.value, SourceUseRole::Conversion)?);
        }
        if used.is_empty() {
            used.extend(self.element_at(
                element,
                path.trim_start_matches('/'),
                SourceUseRole::Selector,
            )?);
        }
        self.bind(path, &used, complete)
    }

    fn token(&mut self, path: &str, value: &EmissionValue<'_>) -> PackageResult<()> {
        let used = self.at(value, SourceUseRole::Payload)?;
        let used = used.as_slice();
        for field in ["name", "attribute", "element", "raw"] {
            self.bind(&format!("{path}/{field}"), used, true)?;
        }
        for index in 0..value.element.len() {
            self.bind(&format!("{path}/element/{index}"), used, true)?;
        }
        Ok(())
    }

    fn number(
        &mut self,
        path: &str,
        number: EmissionNumber,
        used: &[SourceUse],
    ) -> PackageResult<()> {
        self.bind(path, used, true)?;
        self.bind(&format!("{path}/kind"), used, true)?;
        if matches!(number, EmissionNumber::Finite(_)) {
            self.bind(&format!("{path}/value"), used, true)?;
        }
        Ok(())
    }

    pub(super) fn at(
        &self,
        value: &EmissionValue<'_>,
        role: SourceUseRole,
    ) -> PackageResult<Option<SourceUse>> {
        let Some(scope) = &self.evidence.scope else {
            return Ok(None);
        };
        let indices = value
            .element
            .iter()
            .copied()
            .chain(std::iter::once(value.ordinal))
            .map(|index| index as u64)
            .collect();
        scope
            .at(
                SourceLocation::Subresource {
                    namespace: "materialXmlAttribute".to_owned(),
                    indices,
                    field: value.raw.name.clone(),
                },
                role,
            )
            .map(Some)
            .map_err(error)
    }

    pub(super) fn element_at(
        &self,
        indices: &[usize],
        field: &str,
        role: SourceUseRole,
    ) -> PackageResult<Option<SourceUse>> {
        let Some(scope) = &self.evidence.scope else {
            return Ok(None);
        };
        scope
            .at(
                SourceLocation::Subresource {
                    namespace: "materialXmlElement".to_owned(),
                    indices: indices.iter().map(|&index| index as u64).collect(),
                    field: field.to_owned(),
                },
                role,
            )
            .map(Some)
            .map_err(error)
    }

    pub(super) fn bind(
        &mut self,
        pointer: &str,
        uses: &[SourceUse],
        complete: bool,
    ) -> PackageResult<()> {
        let path = FieldPath::parse(pointer).map_err(error)?;
        let origin = self
            .evidence
            .uses
            .origin(uses, complete && self.evidence.complete);
        self.origins.insert(path, origin).map_err(error)
    }
}

fn error(error: impl std::error::Error + Send + Sync + 'static) -> PackageError {
    PackageError::MaterialEmissionEvidence(Box::new(error))
}

pub(super) fn missing_inventory() -> PackageError {
    error(io::Error::other(
        "the material emission owner is unavailable",
    ))
}

pub(super) fn read<'de, D: serde::Deserializer<'de>>(input: D) -> Result<FieldOrigins, D::Error> {
    FieldOriginsSeed::new(OriginReaderLimits {
        fields: usize::MAX,
        metadata_bytes: usize::MAX,
        path_bytes: usize::MAX,
        field_uses: usize::MAX,
        reference_bytes: usize::MAX,
    })
    .deserialize(input)
}

#[cfg(test)]
mod tests {
    use p4kx_extraction::FieldPath;
    use p4kx_model::MaterialLibrary;

    use crate::PackageMaterialEmission;

    use super::EmissionEvidence;

    #[test]
    fn unobserved_material_controls_remain_unknown() -> Result<(), Box<dyn std::error::Error>> {
        let library = MaterialLibrary::parse(
            "synthetic/material",
            br#"<Material Shader="Illum" Glow="0"/>"#,
        )?;
        let source = library
            .emission_source(0)
            .ok_or("authored emission source")?;
        let material = library.materials.first().ok_or("parsed material")?;
        let evidence = EmissionEvidence::new(None)?;
        let emission = PackageMaterialEmission::of(&source, 0, material, &evidence)?;
        let path = FieldPath::parse("/attributes/1/value/value/value")?;
        let raw = emission
            .field_origins
            .get(&path)
            .ok_or("typed Glow origin")?;
        assert_eq!(
            raw.contains_encrypted_source(),
            None,
            "no original read means unknown"
        );
        assert!(
            !raw.source_set_complete(),
            "unobserved contributors are incomplete"
        );
        assert_eq!(
            raw.source_uses_refs().len(),
            0,
            "absence of proof cannot invent a source reference"
        );
        Ok(())
    }
}

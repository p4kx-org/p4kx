// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use serde::de::{Error as _, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

use crate::BuildProvenance;

impl<'de> Deserialize<'de> for BuildProvenance {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_struct("BuildProvenance", FIELDS, BuildVisitor)
    }
}

const FIELDS: &[&str] = &[
    "branch",
    "version",
    "change_number",
    "build_id",
    "public_patch",
];

struct BuildVisitor;
impl<'de> Visitor<'de> for BuildVisitor {
    type Value = BuildProvenance;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a public nullable-patch Build object")
    }
    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
        let mut members = Members::default();
        while let Some(key) = map.next_key::<String>()? {
            members.read(&key, &mut map)?;
        }
        members.finish()
    }
    fn visit_seq<S: SeqAccess<'de>>(self, mut sequence: S) -> Result<Self::Value, S::Error> {
        let branch = sequence
            .next_element()?
            .ok_or_else(|| S::Error::invalid_length(0, &self))?;
        let version = sequence
            .next_element()?
            .ok_or_else(|| S::Error::invalid_length(1, &self))?;
        let change_number = sequence
            .next_element()?
            .ok_or_else(|| S::Error::invalid_length(2, &self))?;
        let build_id = sequence
            .next_element()?
            .ok_or_else(|| S::Error::invalid_length(3, &self))?;
        let public_patch = sequence
            .next_element()?
            .ok_or_else(|| S::Error::invalid_length(4, &self))?;
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(S::Error::invalid_length(6, &self));
        }
        Ok(BuildProvenance {
            branch,
            version,
            change_number,
            build_id,
            public_patch,
            canonical_shape: false,
        })
    }
}

#[derive(Default)]
struct Members {
    branch: Option<String>,
    version: Option<String>,
    change_number: Option<String>,
    build_id: Option<String>,
    public_patch: Option<String>,
    patch_present: bool,
    unknown: bool,
}

impl Members {
    fn read<'de, M: MapAccess<'de>>(&mut self, key: &str, map: &mut M) -> Result<(), M::Error> {
        match key {
            "branch" => required(&mut self.branch, map, "branch"),
            "version" => required(&mut self.version, map, "version"),
            "change_number" => required(&mut self.change_number, map, "change_number"),
            "build_id" => required(&mut self.build_id, map, "build_id"),
            "public_patch" => {
                if self.patch_present {
                    return Err(M::Error::duplicate_field("public_patch"));
                }
                self.patch_present = true;
                self.public_patch = map.next_value()?;
                Ok(())
            }
            _ => {
                let _: IgnoredAny = map.next_value()?;
                self.unknown = true;
                Ok(())
            }
        }
    }
    fn finish<E: serde::de::Error>(self) -> Result<BuildProvenance, E> {
        Ok(BuildProvenance {
            branch: self.branch.ok_or_else(|| E::missing_field("branch"))?,
            version: self.version.ok_or_else(|| E::missing_field("version"))?,
            change_number: self
                .change_number
                .ok_or_else(|| E::missing_field("change_number"))?,
            build_id: self.build_id.ok_or_else(|| E::missing_field("build_id"))?,
            public_patch: self.public_patch,
            canonical_shape: self.patch_present && !self.unknown,
        })
    }
}

fn required<'de, M: MapAccess<'de>>(
    slot: &mut Option<String>,
    map: &mut M,
    field: &'static str,
) -> Result<(), M::Error> {
    if slot.is_some() {
        return Err(M::Error::duplicate_field(field));
    }
    *slot = Some(map.next_value()?);
    Ok(())
}

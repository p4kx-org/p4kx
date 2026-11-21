// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::ser::{Error as _, SerializeSeq, SerializeStruct};
use serde::{Serialize, Serializer};

use crate::line_diff_origins;
use crate::{DiffBody, DiffLine, Encoding, Entry, Hunk, LineDiff, NotText};

impl Serialize for LineDiff<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let origins = line_diff_origins::capture(self).map_err(S::Error::custom)?;
        let mut out = serializer.serialize_struct("LineDiff", 9)?;
        out.serialize_field("path", self.path())?;
        match self.kind() {
            Some(kind) => out.serialize_field("change", &kind)?,
            None => out.serialize_field("change", "unchanged")?,
        }
        out.serialize_field("differs", &self.has_differences())?;
        out.serialize_field("content", content(self.body()))?;
        out.serialize_field(
            "old",
            &self.old_entry().map(|e| Side(e, self.old_encoding())),
        )?;
        out.serialize_field(
            "new",
            &self.new_entry().map(|e| Side(e, self.new_encoding())),
        )?;
        out.serialize_field("hunks", &Hunks(self.body()))?;
        out.serialize_field("fieldOrigins", &origins.field_origins)?;
        out.serialize_field("provenance", &origins.provenance)?;
        out.end()
    }
}

const fn content(body: &DiffBody) -> &'static str {
    match body {
        DiffBody::Text(_) => "text",
        DiffBody::NotText(NotText::Binary) => "binary",
        DiffBody::NotText(NotText::TooLarge) => "too_large",
    }
}

struct Side<'a>(&'a Entry, Option<Encoding>);

impl Serialize for Side<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut out = serializer.serialize_struct("Side", 3)?;
        out.serialize_field("size", &self.0.uncompressed_size())?;
        let checksum = self.0.checksum();
        match checksum.value() {
            Some(value) => out.serialize_field(checksum.name(), &value)?,
            None => out.serialize_field(checksum.name(), &checksum.hex())?,
        }
        out.serialize_field("encoding", &self.1.map(Encoding::as_str))?;
        out.end()
    }
}

struct Hunks<'a>(&'a DiffBody);

impl Serialize for Hunks<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            DiffBody::Text(text) => serializer.collect_seq(text.hunks()),
            DiffBody::NotText(_) => serializer.serialize_seq(Some(0))?.end(),
        }
    }
}

impl Serialize for Hunk<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut out = serializer.serialize_struct("Hunk", 5)?;
        out.serialize_field("old_start", &self.old_start())?;
        out.serialize_field("old_lines", &self.old_lines().len())?;
        out.serialize_field("new_start", &self.new_start())?;
        out.serialize_field("new_lines", &self.new_lines().len())?;
        out.serialize_field("lines", &Lines(*self))?;
        out.end()
    }
}

struct Lines<'a>(Hunk<'a>);

impl Serialize for Lines<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.0.lines())
    }
}

impl Serialize for DiffLine<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut out = serializer.serialize_struct("DiffLine", 2)?;
        out.serialize_field("kind", &self.kind())?;
        out.serialize_field("text", &self.text())?;
        out.end()
    }
}

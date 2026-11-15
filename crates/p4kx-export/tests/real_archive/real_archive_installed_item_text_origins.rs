// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{EntryOrigin, Language, SourceEncryption};
use serde_json::{Value, json};

use super::TestResult;

#[derive(Clone, Copy)]
pub(super) enum Lookup {
    Skipped,
    Missing,
    Resolved,
}

pub(super) struct TextOrigins<'catalog> {
    uses: &'catalog [Value],
    sources: &'catalog [Value],
    dataforge: &'catalog EntryOrigin,
    record_index: u32,
}

impl<'catalog> TextOrigins<'catalog> {
    pub(super) fn new(
        catalog: &'catalog Value,
        dataforge: &'catalog EntryOrigin,
        record_index: u32,
    ) -> TestResult<Self> {
        Ok(Self {
            uses: rows(catalog, "/provenance/uses")?,
            sources: rows(catalog, "/provenance/sources")?,
            dataforge,
            record_index,
        })
    }

    pub(super) fn compare(
        &self,
        carrier: &Value,
        locale: &EntryOrigin,
        language: Language,
        outcome: Lookup,
    ) -> TestResult<()> {
        let references = rows(carrier, "/sourceUsesRefs")?;
        let mut has_owner = false;
        let mut roles = LookupRoles::default();
        for reference in references {
            let used = referenced(self.uses, reference)?;
            if used.pointer("/location/type").and_then(Value::as_str) == Some("localizationLookup")
            {
                self.verify_locale_use(used, references, locale, language)?;
                roles.record(used.get("role").and_then(Value::as_str))?;
            } else {
                self.key_selector(used)?;
                has_owner |= record_index(used) == Some(u64::from(self.record_index));
            }
        }
        if !has_owner || !roles.match_outcome(outcome) {
            return Err(format!("{language}: missing exact Item or locale lookup proof").into());
        }
        let marker = if matches!(outcome, Lookup::Skipped) {
            encryption_marker(self.dataforge.encryption())
        } else {
            combined_marker(self.dataforge.encryption(), locale.encryption())
        };
        if carrier.get("containsEncryptedSource") != Some(&json!(marker))
            || carrier.get("sourceSetComplete").and_then(Value::as_bool) != Some(true)
        {
            return Err(format!(
                "{language}: direct text provenance disagrees with the physical Entries"
            )
            .into());
        }
        Ok(())
    }

    fn verify_locale_use(
        &self,
        used: &Value,
        references: &[Value],
        locale: &EntryOrigin,
        language: Language,
    ) -> TestResult<()> {
        if used.pointer("/location/language").and_then(Value::as_str) != Some(language.code()) {
            return Err("a lookup cites a different language".into());
        }
        self.physical_source(used, locale)?;
        let key = used
            .pointer("/location/selector")
            .ok_or("the original lookup key selector")?;
        if !references.contains(key) {
            return Err("the lookup key is not one of this form's original uses".into());
        }
        self.key_selector(referenced(self.uses, key)?)
    }

    fn key_selector(&self, used: &Value) -> TestResult<()> {
        if used.get("role").and_then(Value::as_str) != Some("selector")
            || record_index(used).is_none()
        {
            return Err(
                "the text's key must come from a physical DataForge Record selector".into(),
            );
        }
        self.physical_source(used, self.dataforge)
    }

    fn physical_source(&self, used: &Value, original: &EntryOrigin) -> TestResult<()> {
        let source = referenced(
            self.sources,
            used.get("source").ok_or("physical source reference")?,
        )?;
        let entry = original.entry();
        if used.get("scope").and_then(Value::as_str) != Some("payload")
            || source.get("kind").and_then(Value::as_str) != Some("archiveEntry")
            || source.get("entryIndex").and_then(Value::as_u64) != Some(u64::from(entry.ordinal()))
            || source.pointer("/path/value").and_then(Value::as_str) != Some(entry.path())
            || source.get("storedEncryption").and_then(Value::as_str)
                != Some(encryption_state(entry.encryption()))
        {
            return Err("the text use differs from the observed physical Archive Entry".into());
        }
        Ok(())
    }
}

#[derive(Default)]
struct LookupRoles {
    selector: bool,
    payload: bool,
    conversion: bool,
}

impl LookupRoles {
    fn record(&mut self, role: Option<&str>) -> TestResult<()> {
        match role {
            Some("selector") => self.selector = true,
            Some("payload") => self.payload = true,
            Some("conversion") => self.conversion = true,
            _ => return Err("unexpected original locale contributor role".into()),
        }
        Ok(())
    }

    const fn match_outcome(&self, outcome: Lookup) -> bool {
        match outcome {
            Lookup::Skipped => !self.selector && !self.payload && !self.conversion,
            Lookup::Missing => self.selector && !self.payload && !self.conversion,
            Lookup::Resolved => self.selector && self.payload,
        }
    }
}

fn record_index(used: &Value) -> Option<u64> {
    match used.pointer("/location/type").and_then(Value::as_str) {
        Some("dataForge") => used.pointer("/location/recordIndex")?.as_u64(),
        Some("subresource")
            if used.pointer("/location/namespace")?.as_str()? == "dataforgeValue" =>
        {
            used.pointer("/location/indices/0")?.as_str()?.parse().ok()
        }
        _ => None,
    }
}

fn referenced<'a>(rows: &'a [Value], wanted: &Value) -> TestResult<&'a Value> {
    rows.iter()
        .find(|row| row.get("id") == Some(wanted))
        .ok_or_else(|| "unresolved text origin reference".into())
}

fn rows<'a>(value: &'a Value, path: &str) -> TestResult<&'a [Value]> {
    value
        .pointer(path)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "missing text provenance row array".into())
}

const fn encryption_marker(state: SourceEncryption) -> Option<bool> {
    match state {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    }
}

const fn combined_marker(left: SourceEncryption, right: SourceEncryption) -> Option<bool> {
    match (encryption_marker(left), encryption_marker(right)) {
        (Some(true), _) | (_, Some(true)) => Some(true),
        (Some(false), Some(false)) => Some(false),
        _ => None,
    }
}

const fn encryption_state(state: SourceEncryption) -> &'static str {
    match state {
        SourceEncryption::Encrypted => "encrypted",
        SourceEncryption::Unencrypted => "unencrypted",
        _ => "unknown",
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::error::Error;

use p4kx_export::Kind;
use serde_json::{Map, Value};

use crate::catalog_v3_consumer;
use crate::real_archive_contract_templates::mission_graph::{self as graph, Family};
use crate::real_archive_text_oracle::Oracle;
use crate::real_archive_texts::{Coverage, keyed_texts};

type TestResult<T> = Result<T, Box<dyn Error>>;

/// Canonical protection domain of an owning text reference.
#[derive(Clone, Copy)]
pub(crate) enum TextDomain {
    PublicName,
    GameText,
}

/// Whether a present projection permits a null text reference.
#[derive(Clone, Copy)]
pub(crate) enum TextReferenceShape {
    Required,
    Nullable,
}

const FIELDS: [(&str, TextDomain); 2] = [
    ("name", TextDomain::PublicName),
    ("description", TextDomain::GameText),
];

const REPUTATION_DISPLAY_FIELDS: [(&str, TextDomain); 1] = [("displayName", TextDomain::GameText)];
const STARMAP_NAME: [(&str, TextDomain); 1] = [("name", TextDomain::GameText)];

const PRESENTATION_FIELDS: [(&str, TextDomain); 3] = [
    ("name", TextDomain::PublicName),
    ("description", TextDomain::GameText),
    ("displayType", TextDomain::GameText),
];

/// A validated borrowed target retaining its original language carriers.
pub(crate) struct TextTarget<'catalog> {
    source: &'catalog Value,
    key: &'catalog str,
    translations: &'catalog Map<String, Value>,
}

impl<'catalog> TextTarget<'catalog> {
    /// Borrows the original canonical row after source-state validation.
    pub(crate) const fn source(&self) -> &'catalog Value {
        self.source
    }

    /// Borrows one required validated language payload, including an explicit source gap.
    pub(crate) fn shown(&self, language: &str) -> TestResult<&'catalog Value> {
        self.form(language, "displayText")
    }

    fn raw(&self, language: &str) -> TestResult<&'catalog Value> {
        self.form(language, "sourceText")
    }

    fn form(&self, language: &str, field: &str) -> TestResult<&'catalog Value> {
        let forms = self
            .translations
            .get(language)
            .ok_or_else(|| format!("required canonical language {language}"))?;
        language_payload(forms.get(field).ok_or("required direct text form")?)
    }
}

/// Canonical domain lookup without copied text payloads.
pub(crate) struct TextIndex<'catalog> {
    names: HashMap<&'catalog str, TextTarget<'catalog>>,
    protected: HashMap<&'catalog str, TextTarget<'catalog>>,
}

impl<'catalog> TextIndex<'catalog> {
    /// Indexes and validates both canonical text domains.
    pub(crate) fn new(catalog: &'catalog Value) -> TestResult<Self> {
        Ok(Self {
            names: table(catalog, "publicNames")?,
            protected: table(catalog, "gameTexts")?,
        })
    }

    /// Resolves an actual domain target while preserving source absence and nullable choices.
    pub(crate) fn reference(
        &self,
        carrier: &Value,
        domain: TextDomain,
        shape: TextReferenceShape,
    ) -> TestResult<Option<&TextTarget<'catalog>>> {
        let Some(value) = catalog_v3_consumer::source_value(carrier)? else {
            return Ok(None);
        };
        if value.is_null() && matches!(shape, TextReferenceShape::Nullable) {
            return Ok(None);
        }
        let id = value.as_str().ok_or("canonical text reference string")?;
        let index = match domain {
            TextDomain::PublicName => &self.names,
            TextDomain::GameText => &self.protected,
        };
        index
            .get(id)
            .map(Some)
            .ok_or_else(|| "present text reference has no domain target".into())
    }

    fn manufacturer(&self, row: &Value, field: &str) -> TestResult<Option<&TextTarget<'catalog>>> {
        let domain = match field {
            "name" => TextDomain::PublicName,
            "description" => TextDomain::GameText,
            _ => return Err("unknown Manufacturer text domain".into()),
        };
        let Some(localization) = catalog_v3_consumer::source_value(
            row.get("localization").ok_or("Localization carrier")?,
        )?
        else {
            return Ok(None);
        };
        let localization = localization
            .as_object()
            .ok_or("present owned Localization")?;
        self.reference(
            localization.get(field).ok_or("localized field carrier")?,
            domain,
            TextReferenceShape::Required,
        )
    }
}

fn table<'catalog>(
    catalog: &'catalog Value,
    domain: &str,
) -> TestResult<HashMap<&'catalog str, TextTarget<'catalog>>> {
    let rows = catalog
        .get("texts")
        .and_then(|texts| texts.get(domain))
        .and_then(Value::as_array)
        .ok_or("required canonical text table")?;
    let mut index = HashMap::new();
    for row in rows {
        let id = catalog_v3_consumer::present(row.get("id").ok_or("canonical text id carrier")?)?
            .as_str()
            .ok_or("canonical text id")?;
        let key =
            catalog_v3_consumer::present(row.get("key").ok_or("canonical text key carrier")?)?
                .as_str()
                .ok_or("canonical text key")?;
        let translations = catalog_v3_consumer::present(
            row.get("translations")
                .ok_or("canonical translations carrier")?,
        )?
        .as_object()
        .ok_or("canonical language map")?;
        for forms in translations.values() {
            validate_forms(forms, key)?;
        }
        let target = TextTarget {
            source: row,
            key,
            translations,
        };
        if index.insert(id, target).is_some() {
            return Err("duplicate canonical text target".into());
        }
    }
    Ok(index)
}

fn validate_forms(forms: &Value, key: &str) -> TestResult<()> {
    for field in ["sourceText", "displayText"] {
        let value = language_payload(forms.get(field).ok_or("required direct text form")?)?;
        if p4kx_extraction::is_placeholder_key(key) && !value.is_null() {
            return Err("placeholder cannot retain a winning source or display text".into());
        }
    }
    Ok(())
}

fn language_payload(carrier: &Value) -> TestResult<&Value> {
    let value = catalog_v3_consumer::source_value(carrier)?;
    match (carrier.get("state").and_then(Value::as_str), value) {
        (Some("present"), Some(value)) if value.is_string() => Ok(value),
        (Some("absent"), None) => carrier
            .get("value")
            .ok_or_else(|| "nullable translation".into()),
        _ => Err("malformed canonical language carrier".into()),
    }
}

/// Resolves a Manufacturer reference inside its actual canonical text domain.
pub(crate) fn manufacturer_text<'catalog>(
    catalog: &'catalog Value,
    row: &Value,
    field: &str,
) -> TestResult<Option<&'catalog Value>> {
    Ok(TextIndex::new(catalog)?
        .manufacturer(row, field)?
        .map(TextTarget::source))
}

struct CoverageCounters<'oracle> {
    english: Coverage,
    german: Coverage,
    english_oracle: &'oracle Oracle,
    german_oracle: &'oracle Oracle,
}

impl<'oracle> CoverageCounters<'oracle> {
    fn new(english: &'oracle Oracle, german: &'oracle Oracle) -> Self {
        Self {
            english: Coverage::default(),
            german: Coverage::default(),
            english_oracle: english,
            german_oracle: german,
        }
    }

    fn count(&mut self, target: Option<&TextTarget<'_>>) -> TestResult<()> {
        let Some(target) = target else {
            return Ok(());
        };
        if p4kx_extraction::is_placeholder_key(target.key) {
            return Ok(());
        }
        self.english.count_forms(
            self.english_oracle,
            target.key,
            target.raw("en")?,
            target.shown("en")?,
        );
        self.german.count_forms(
            self.german_oracle,
            target.key,
            target.raw("de")?,
            target.shown("de")?,
        );
        Ok(())
    }

    fn finish(self) -> (Coverage, Coverage) {
        (self.english, self.german)
    }
}

pub(crate) fn kind_coverage(
    catalog: &Value,
    kind: Kind,
    fields: &[String],
    english_oracle: &Oracle,
    german_oracle: &Oracle,
) -> TestResult<(Coverage, Coverage)> {
    let mission_family = match kind {
        Kind::ContractTemplates => Some(Family::Template),
        Kind::MissionBrokerEntries => Some(Family::Broker),
        _ => None,
    };
    if let Some(family) = mission_family {
        return mission_coverage(catalog, family, english_oracle, german_oracle);
    }
    if matches!(
        kind,
        Kind::Manufacturers
            | Kind::Commodities
            | Kind::ResourceTypes
            | Kind::Blueprints
            | Kind::ReputationScopes
            | Kind::ReputationStandings
            | Kind::StarmapObjects
    ) {
        return canonical_coverage(catalog, kind, english_oracle, german_oracle);
    }
    let (mut english, mut german) = (Coverage::default(), Coverage::default());
    let mut found = Vec::new();
    for row in catalog_v3_consumer::rows(catalog, kind)? {
        keyed_texts(row, fields, &mut found);
    }
    for (key, text) in found {
        english.count(english_oracle, key, text.get("en"));
        german.count(german_oracle, key, text.get("de"));
    }
    Ok((english, german))
}

fn mission_coverage(
    catalog: &Value,
    family: Family,
    english_oracle: &Oracle,
    german_oracle: &Oracle,
) -> TestResult<(Coverage, Coverage)> {
    let mut counters = CoverageCounters::new(english_oracle, german_oracle);
    let index = TextIndex::new(catalog)?;
    for mission in graph::contexts(catalog, family)? {
        for lookup in graph::array(mission.projection, "localizations")? {
            let source = lookup.get("source").ok_or("mission localization source")?;
            let key = mission_text_key(mission.follow(source)?)?;
            let target = index.reference(
                lookup
                    .get("text")
                    .ok_or("mission localization text carrier")?,
                TextDomain::GameText,
                TextReferenceShape::Required,
            )?;
            match (graph::text(lookup, "/interpretation")?, key, target) {
                ("lookup", Some(key), Some(target)) if key == target.key => {
                    counters.count(Some(target))?;
                }
                (
                    "empty" | "placeholder" | "notKey" | "null" | "wrongShape" | "unreadable",
                    None,
                    None,
                ) => {}
                _ => {
                    return Err("mission localization disagrees with original key or text".into());
                }
            }
        }
    }
    Ok(counters.finish())
}

fn mission_text_key(node: &Value) -> TestResult<Option<&str>> {
    let carrier = node.get("value").ok_or("original mission text carrier")?;
    let Some(payload) = catalog_v3_consumer::source_value(carrier)? else {
        return Ok(None);
    };
    if !matches!(graph::text(payload, "/type")?, "string" | "locale") {
        return Ok(None);
    }
    let key = graph::text(payload, "/value/sourceText")?;
    Ok((key.starts_with('@') && !p4kx_extraction::is_placeholder_key(key)).then_some(key))
}

fn canonical_coverage(
    catalog: &Value,
    kind: Kind,
    english_oracle: &Oracle,
    german_oracle: &Oracle,
) -> TestResult<(Coverage, Coverage)> {
    let mut counters = CoverageCounters::new(english_oracle, german_oracle);
    let index = TextIndex::new(catalog)?;
    if kind == Kind::Blueprints {
        let blueprints = crate::real_archive_blueprints::Blueprints::read(catalog)?;
        for blueprint in blueprints.rows() {
            counters.count(index.reference(
                blueprint.presentation.get("name").ok_or("Blueprint name")?,
                TextDomain::GameText,
                TextReferenceShape::Nullable,
            )?)?;
        }
        return Ok(counters.finish());
    }
    let rows = if kind == Kind::Commodities {
        catalog_v3_consumer::presentations(catalog)?
    } else {
        catalog_v3_consumer::rows(catalog, kind)?
    };
    for row in rows {
        if kind == Kind::Manufacturers {
            for (field, _) in FIELDS {
                counters.count(index.manufacturer(row, field)?)?;
            }
            continue;
        }
        let (fields, shape) = match kind {
            Kind::ReputationScopes | Kind::ReputationStandings => (
                REPUTATION_DISPLAY_FIELDS.as_slice(),
                TextReferenceShape::Required,
            ),
            Kind::Commodities => (PRESENTATION_FIELDS.as_slice(), TextReferenceShape::Nullable),
            Kind::StarmapObjects => (STARMAP_NAME.as_slice(), TextReferenceShape::Required),
            _ => (FIELDS.as_slice(), TextReferenceShape::Required),
        };
        for &(field, domain) in fields {
            counters.count(index.reference(
                row.get(field).ok_or("required canonical text reference")?,
                domain,
                shape,
            )?)?;
        }
    }
    Ok(counters.finish())
}

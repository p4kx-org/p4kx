// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::ptr;

use p4kx_extraction::{Archive, DataForgeEntry, Language, SourceUse, SourceUseRole, SourceValue};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct};

use super::{lookup_use, project};
use crate::build::original_input::test_support::TestResult;
use crate::catalog_origins::{CatalogLocaleInput, CatalogOrigins};
use crate::catalog_text_contract::TextForms;
use crate::texts::{TextSources, Texts};

const LOOKUP: [SourceUseRole; 1] = [SourceUseRole::Selector];
const READ: [SourceUseRole; 2] = [SourceUseRole::Selector, SourceUseRole::Payload];
const CONVERT: [SourceUseRole; 3] = [
    SourceUseRole::Selector,
    SourceUseRole::Payload,
    SourceUseRole::Conversion,
];

struct Case {
    language: Language,
    key: &'static str,
    looked_up: bool,
    text: Option<(&'static str, &'static str)>,
    source_roles: &'static [SourceUseRole],
    display_roles: &'static [SourceUseRole],
}

const CASES: [Case; 5] = [
    Case {
        language: Language::English,
        key: "@present",
        looked_up: true,
        text: Some(("Raw\\nLine", "Raw\nLine")),
        source_roles: &READ,
        display_roles: &CONVERT,
    },
    Case {
        language: Language::German,
        key: "@present",
        looked_up: true,
        text: Some(("Raw\\nLine", "Raw\nLine")),
        source_roles: &CONVERT,
        display_roles: &CONVERT,
    },
    Case {
        language: Language::English,
        key: "@empty",
        looked_up: true,
        text: Some(("", "")),
        source_roles: &READ,
        display_roles: &CONVERT,
    },
    Case {
        language: Language::German,
        key: "@missing",
        looked_up: true,
        text: None,
        source_roles: &LOOKUP,
        display_roles: &LOOKUP,
    },
    Case {
        language: Language::German,
        key: "@LOC_EMPTY",
        looked_up: false,
        text: None,
        source_roles: &[],
        display_roles: &[],
    },
];

#[test]
fn equal_locale_forms_share_references_and_keep_original_wire_bytes() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("text-forms.p4k");
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("FixtureTexts"))
        .record(TestRecord::new(
            1,
            "FixtureTexts",
            "TextFixture",
            "fixture/texts.xml",
        ))
        .build();
    let ini = b"present=Raw\\nLine\r\nempty=\r\n";
    let mut bom = b"\xEF\xBB\xBF".to_vec();
    bom.extend_from_slice(ini);
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database))
        .entry(TestEntry::stored(
            Language::English.global_ini_path(),
            ini.as_slice(),
        ))
        .entry(TestEntry::stored(Language::German.global_ini_path(), bom).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let entry = DataForgeEntry::read(&archive)?;
    let sources = TextSources::read(&archive)?;
    let origins = CatalogOrigins::new(&archive, &entry, &sources)?;
    let texts = Texts::parse(&sources)?;
    let record = origins.record(0);
    let selectors = [
        record.observe("first", SourceUseRole::Selector)?,
        record.observe("second", SourceUseRole::Selector)?,
    ];
    for case in &CASES {
        verify(&origins, &texts, &selectors, case)?;
    }
    Ok(())
}

fn verify(
    origins: &CatalogOrigins,
    texts: &Texts<'_>,
    selectors: &[SourceUse],
    case: &Case,
) -> TestResult {
    let input = origins
        .locale(case.language)
        .ok_or("original locale Entry")?;
    let selected = if case.looked_up {
        texts.project(case.language, case.key)
    } else {
        None
    };
    let actual = project(
        &origins.uses,
        input,
        case.language,
        selectors,
        selected,
        case.looked_up,
    )?;
    let source_uses = expected_uses(input, case.language, selectors, case.source_roles)?;
    let display_uses = expected_uses(input, case.language, selectors, case.display_roles)?;
    let (source, display) = case.text.unzip();
    let value = |text: Option<&'static str>| {
        text.map_or(SourceValue::Absent, |text| {
            SourceValue::Present(Cow::Borrowed(text))
        })
    };
    let expected = TextForms {
        source_text: origins.uses.value(value(source), &source_uses, true),
        display_text: origins.uses.value(value(display), &display_uses, true),
    };
    assert_eq!(
        serde_json::to_vec(&actual.forms)?,
        serde_json::to_vec(&expected)?,
        "exact forms, flags and source IDs for {}",
        case.key
    );
    assert!(
        actual
            .uses
            .iter()
            .map(SourceUse::reference)
            .eq(display_uses.iter().map(SourceUse::reference)),
        "aggregate retains identical original display use order for {}",
        case.key
    );
    let source_refs = actual.forms.source_text.origin().source_uses_refs();
    let display_refs = actual.forms.display_text.origin().source_uses_refs();
    assert_eq!(
        ptr::eq(source_refs, display_refs),
        case.source_roles == case.display_roles,
        "only identical authored and locale contributions share storage"
    );
    Ok(())
}

fn expected_uses(
    input: &CatalogLocaleInput,
    language: Language,
    selectors: &[SourceUse],
    roles: &[SourceUseRole],
) -> TestResult<Vec<SourceUse>> {
    let mut uses = selectors.to_vec();
    for selector in selectors {
        for &role in roles
            .iter()
            .filter(|&&role| role != SourceUseRole::Conversion)
        {
            uses.push(lookup_use(input, language, selector, role)?);
        }
    }
    if roles.contains(&SourceUseRole::Conversion) {
        for selector in selectors {
            uses.push(lookup_use(
                input,
                language,
                selector,
                SourceUseRole::Conversion,
            )?);
        }
    }
    Ok(uses)
}

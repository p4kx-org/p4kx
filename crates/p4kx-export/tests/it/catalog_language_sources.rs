// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{IniEncoding, Language, LocalizationEntry};
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use crate::health_origin_assertions as proof;
use crate::text_forms_fixture::{self as fixture, Origin, TestResult};

#[test]
fn language_metadata_remains_complete_without_any_localized_catalog_rows() -> TestResult {
    let database = DataForgeBuilder::new().version(8);
    let (_folder, archive) = fixture::open(
        &database,
        [
            fixture::locale(
                Language::English,
                b"raw=\xff\r\nmalformed line\n".to_vec(),
                Origin::Plain,
            ),
            fixture::locale(
                Language::German,
                fixture::encoded("raw=Quelle\n", IniEncoding::Utf16Le)?,
                Origin::Encrypted,
            ),
            fixture::locale(Language::Japanese, b"raw=?\n".to_vec(), Origin::Unknown),
        ],
    )?;
    let (_, catalog) = fixture::export(&archive)?;
    assert_eq!(
        proof::at(&catalog, "/texts/languagePolicy")?,
        &json!({"selection":"availableSupported", "fallback":"none"}),
        "the producer declares a single language policy independent of selected keys",
    );
    for table in ["/texts/publicNames", "/texts/gameTexts"] {
        assert!(
            proof::rows(&catalog, table)?.is_empty(),
            "no authored text row was needed"
        );
    }
    let languages = proof::at(&catalog, "/texts/languageSources")?;
    assert_eq!(
        languages.as_object().ok_or("language source map")?.len(),
        Language::ALL.len(),
        "one actual state per supported language, including absent languages",
    );
    for (language, encoding, lossy, skipped, encrypted) in [
        (Language::English, "utf8", true, 1, Some(false)),
        (Language::German, "utf16Le", false, 0, Some(true)),
        (Language::Japanese, "utf8", false, 0, None),
    ] {
        let entry = LocalizationEntry::read(&archive, language)?;
        let source = proof::cell(languages, language.code(), "present")?;
        let row = proof::at(source, "/value")?;
        let marker = proof::at(source, "/containsEncryptedSource")?;
        assert_eq!(
            marker,
            &json!(encrypted),
            "the outer carrier includes payload diagnostics"
        );
        proof::present(row, "path", &json!(entry.path()), Some(false))?;
        let encoding = proof::present(row, "encoding", &json!(encoding), encrypted)?;
        proof::present(row, "lossyDecoding", &json!(lossy), encrypted)?;
        proof::present(row, "skippedMalformedLines", &json!(skipped), encrypted)?;
        assert_encoding_input(&catalog, &entry, encoding)?;
    }
    assert_absence(&catalog, languages, Language::French)?;
    proof::closure(&catalog)
}

fn assert_encoding_input(
    catalog: &Value,
    original: &LocalizationEntry,
    carrier: &Value,
) -> TestResult {
    let sites = proof::uses(catalog, carrier)?;
    let [site] = sites.as_slice() else {
        return Err("one direct encoding observation".into());
    };
    assert_eq!(
        proof::at(site, "/scope")?,
        "payload",
        "encoding derives from the original payload"
    );
    assert_eq!(
        proof::at(site, "/role")?,
        "conversion",
        "encoding is a decoding observation"
    );
    assert_eq!(
        proof::at(site, "/location/namespace")?,
        "globalIni",
        "the original INI scope"
    );
    let source = proof::source(catalog, site)?;
    let ordinal = original
        .origin()
        .ok_or("original observed Entry")?
        .entry()
        .ordinal();
    assert_eq!(
        proof::at(source, "/entryIndex")?,
        &json!(ordinal),
        "the exact physical language Entry is bound"
    );
    Ok(())
}

fn assert_absence(catalog: &Value, languages: &Value, language: Language) -> TestResult {
    let absent = proof::cell(languages, language.code(), "absent")?;
    assert_eq!(
        proof::at(absent, "/sourceSetComplete")?,
        &json!(true),
        "the root directory was fully surveyed"
    );
    let selected = proof::uses(catalog, absent)?;
    let [site] = selected.as_slice() else {
        return Err("one compact absence survey".into());
    };
    assert_eq!(
        proof::at(site, "/location/type")?,
        "archiveDirectoryPaths",
        "absence cites the physical path scan"
    );
    assert_eq!(
        proof::at(proof::source(catalog, site)?, "/kind")?,
        "archiveDirectory",
        "the survey is independent of locale payloads"
    );
    Ok(())
}

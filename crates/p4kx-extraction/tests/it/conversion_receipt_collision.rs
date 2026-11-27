// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::num::NonZeroUsize;

use p4kx_extraction::{
    Checksum, ConflictPolicy, ExtractOptions, ExtractionError, ExtractionReport, extract,
};
use p4kx_testkit::TestEntry;
use sha2::{Digest as _, Sha256};

use super::{SHIP_XML, TestResult, fixture, little, read};

const XML: &str = r"Data\foo.xml";
const NATIVE: &str = r"Data\foo.xml.p4kx-origin.jsonl";
const NATIVE_BYTES: &[u8] = b"{\"ordinary\":\"native entry\"}\n";

#[test]
fn selected_native_entry_owns_the_cryxml_companion_path_in_both_orders() -> TestResult {
    for native_first in [false, true] {
        let fixture = fixture(vec![
            TestEntry::stored(XML, little()),
            TestEntry::stored(NATIVE, NATIVE_BYTES),
        ])?;
        let find = |path| {
            fixture
                .archive
                .entries()
                .iter()
                .find(|entry| entry.path() == path)
        };
        let xml = find(XML).ok_or("the original CryXml Entry")?;
        let native = find(NATIVE).ok_or("the native sibling Entry")?;
        let selected = if native_first {
            [native, xml]
        } else {
            [xml, native]
        };
        let options = ExtractOptions {
            convert: true,
            policy: ConflictPolicy::Overwrite,
            threads: Some(NonZeroUsize::MIN),
            ..ExtractOptions::default()
        };

        let report = extract(&fixture.archive, &selected, &fixture.target, &options);

        assert_eq!(
            read(&fixture.target, "Data/foo.xml.p4kx-origin.jsonl")?,
            NATIVE_BYTES,
            "the selected native sibling owns the path in order {native_first}"
        );
        let rendered = read(&fixture.target, "Data/foo.xml")?;
        assert_eq!(
            rendered,
            SHIP_XML.as_bytes(),
            "the CryXml is still rendered"
        );
        check_artifact(&report, &rendered)?;
        let native_path = std::path::absolute(&fixture.target)?
            .join("Data")
            .join("foo.xml.p4kx-origin.jsonl");
        check_collision(&report, &native_path)?;
    }
    Ok(())
}

fn check_artifact(report: &ExtractionReport, rendered: &[u8]) -> TestResult {
    assert_eq!(
        (report.extracted(), report.converted()),
        (2, Some(1)),
        "both payloads were committed and the XML was converted"
    );
    let [artifact] = report.cryxml_artifacts() else {
        return Err("one converted XML artifact expected".into());
    };
    assert_eq!(artifact.path, "Data/foo.xml", "the rendered artifact");
    assert!(
        artifact.origin_receipt.is_none(),
        "the companion is reserved"
    );
    assert_eq!(
        artifact.digest.byte_length(),
        u64::try_from(rendered.len())?,
        "the digest-only artifact states the committed length"
    );
    let sha256 = Checksum::Sha256(Sha256::digest(rendered).into()).hex();
    assert_eq!(
        artifact.digest.sha256(),
        sha256,
        "the digest-only artifact states the committed hash"
    );
    Ok(())
}

fn check_collision(report: &ExtractionReport, native_path: &std::path::Path) -> TestResult {
    let [failure] = report.failures() else {
        return Err(format!("one CryXml companion collision expected: {report:?}").into());
    };
    assert_eq!(
        failure.path(),
        XML,
        "the failure belongs to the CryXml Entry"
    );
    let ExtractionError::Write { path, source } = failure.error() else {
        return Err("the failure must address the companion write".into());
    };
    assert_eq!(path, native_path, "the occupied native companion path");
    let conflict = source
        .get_ref()
        .and_then(|cause| cause.downcast_ref::<ExtractionError>());
    assert!(
        matches!(conflict, Some(ExtractionError::CaseCollision { earlier }) if earlier == NATIVE),
        "the companion refusal identifies the native Entry: {failure:?}"
    );
    Ok(())
}

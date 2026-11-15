// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::ops::Range;
use std::path::Path;

use p4kx_extraction::{OriginDigestWriter, Soundbank};
use p4kx_testkit::test_key;
use serde_json::{Value, json};

use super::TestResult;

#[path = "wwise_txtp_origin_fixture.rs"]
mod fixture;

use self::fixture::{BLEND, BLEND_BANK, DELAY, Fixture, MAIN_BANK, MODE_OFFSET, SEQUENCE};

const EVENT: &str = "Play_Origin";
const TXTP: &str = "Play_Origin.txtp";
const RECEIPT: &str = "Play_Origin.txtp.p4kx-origin.jsonl";
const LOOPS_PATH: &str = "/approximations/0";
const TRANSITION_PATH: &str = "/approximations/1";
const BLEND_PATH: &str = "/approximations/2";
const TRANSITION_CODE: &str = "omit-container-transitions";
const BLEND_CODE: &str = "omit-blend-curves";

#[test]
fn txtp_diagnostics_bind_exact_text_and_bnks_with_independent_markers() -> TestResult {
    let fixture = fixture::create()?;
    let report = extract(&fixture.archive, &fixture.out, "overwrite")?;
    let file = txtp_file(&report)?;
    assert_eq!(
        file["approximations"],
        json!(["omit-container-loops", TRANSITION_CODE, BLEND_CODE]),
        "one output contains the three different contributor states"
    );
    check_unknown(field(file, LOOPS_PATH)?);

    let sequence =
        hirc_extent(&fixture.main_bank, SEQUENCE)?.start + size_of::<u32>() + MODE_OFFSET;
    let blend_end = hirc_extent(&fixture.blend_bank, BLEND)?.end;
    let blend = blend_end - size_of::<u8>() - 4 * size_of::<u32>();
    check_word(
        file,
        TRANSITION_PATH,
        MAIN_BANK,
        false,
        "randomSequenceTransitionMode",
        sequence..sequence + size_of::<u8>(),
    )?;
    check_word(
        file,
        BLEND_PATH,
        BLEND_BANK,
        true,
        "blendPointCount",
        blend..blend + size_of::<u32>(),
    )?;
    assert_eq!(
        fixture.main_bank.get(sequence).copied(),
        Some(DELAY),
        "the reported mode byte is the authored Delay"
    );
    assert_eq!(
        fixture.blend_bank.get(blend..blend + size_of::<u32>()),
        Some(1_u32.to_le_bytes().as_slice()),
        "the reported position is the original nonempty point count"
    );
    check_companion(&fixture, file)
}

#[test]
fn txtp_kept_identical_bytes_do_not_receive_current_archive_provenance() -> TestResult {
    let fixture = fixture::create()?;
    extract(&fixture.archive, &fixture.out, "overwrite")?;
    let artifact = fixture.out.join(TXTP);
    let receipt = fixture.out.join(RECEIPT);
    let old_bytes = fs::read(&artifact)?;
    let old_receipt = fs::read(&receipt)?;
    let report = extract(&fixture.alternate, &fixture.out, "if-changed")?;
    let file = txtp_file(&report)?;
    assert_eq!(file["kept"], true, "the equal text is retained");
    assert_eq!(
        file.get("originReceipt"),
        Some(&Value::Null),
        "retaining bytes cannot rebind the prior receipt"
    );
    for path in [LOOPS_PATH, TRANSITION_PATH, BLEND_PATH] {
        check_unknown(field(file, path)?);
    }
    assert_eq!(fs::read(&artifact)?, old_bytes, "TXTP is unchanged");
    assert_eq!(
        fs::read(&receipt)?,
        old_receipt,
        "old evidence is unchanged"
    );

    let report = extract(&fixture.alternate, &fixture.out, "overwrite")?;
    let file = txtp_file(&report)?;
    assert_eq!(
        field(file, TRANSITION_PATH)?["containsEncryptedSource"],
        true,
        "the same mode now comes from an encrypted bank which supplied the replacement"
    );
    assert_eq!(
        field(file, BLEND_PATH)?["containsEncryptedSource"],
        false,
        "the same blend now comes from a plain bank which supplied the replacement"
    );
    assert_eq!(
        fs::read(&artifact)?,
        old_bytes,
        "replaced TXTP bytes are equal"
    );
    assert_ne!(
        fs::read(&receipt)?,
        old_receipt,
        "replacement binds new sources"
    );
    Ok(())
}

fn check_unknown(origin: &Value) {
    assert_eq!(
        origin,
        &json!({
            "containsEncryptedSource": null,
            "sourceUsesRefs": [],
            "sourceSetComplete": false
        }),
        "no bound source proof remains explicitly unknown"
    );
}

fn check_word(
    file: &Value,
    path: &str,
    bank: &str,
    encrypted: bool,
    name: &str,
    range: Range<usize>,
) -> TestResult {
    let origin = field(file, path)?;
    assert_eq!(
        origin["containsEncryptedSource"], encrypted,
        "the code's actual source sets its immediate marker"
    );
    assert_eq!(
        origin["sourceSetComplete"], true,
        "both actual uses are known"
    );
    let refs = origin["sourceUsesRefs"]
        .as_array()
        .ok_or("source use refs")?;
    assert_eq!(
        refs.len(),
        2,
        "only the control field and layout selector contribute"
    );
    let observations = file
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    let sources = file
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("physical sources")?;
    let mut conversions = 0;
    let mut selectors = 0;
    for reference in refs {
        let actual = observations
            .iter()
            .find(|observation| observation.get("id") == Some(reference))
            .ok_or("actual use")?;
        check_bank(actual, sources, bank)?;
        let field_name = actual.pointer("/location/field").and_then(Value::as_str);
        match actual.get("role").and_then(Value::as_str) {
            Some("conversion") => {
                conversions += 1;
                assert_eq!(field_name, Some(name), "the diagnosed field");
                assert_eq!(
                    actual.pointer("/location/indices"),
                    Some(&json!([range.start.to_string(), range.end.to_string()])),
                    "exact original {name} bytes"
                );
            }
            Some("selector") => {
                selectors += 1;
                assert_eq!(field_name, Some("version"), "the checked layout selector");
            }
            _ => return Err("unexpected source-use role".into()),
        }
    }
    assert_eq!(
        (conversions, selectors),
        (1, 1),
        "both roles are present once"
    );
    Ok(())
}

fn check_bank(actual: &Value, sources: &[Value], bank: &str) -> TestResult {
    let source = sources
        .iter()
        .find(|source| source.get("id") == actual.get("source"))
        .ok_or("physical bank")?;
    assert_eq!(
        source.pointer("/path/value").and_then(Value::as_str),
        Some(bank),
        "the actual bank"
    );
    assert_eq!(
        actual.get("scope").and_then(Value::as_str),
        Some("payload"),
        "decoded bank bytes"
    );
    Ok(())
}

fn check_companion(fixture: &Fixture, file: &Value) -> TestResult {
    let receipt = file["originReceipt"].as_str().ok_or("receipt name")?;
    assert_eq!(receipt, RECEIPT, "portable canonical TXTP companion name");
    let contents = fs::read_to_string(fixture.out.join(receipt))?;
    let mut rows = contents.lines().map(serde_json::from_str::<Value>);
    let header = rows.next().ok_or("receipt header")??;
    let bytes = fs::read(fixture.out.join(TXTP))?;
    let mut digest = OriginDigestWriter::new(io::sink());
    digest.write_all(&bytes)?;
    let digest = digest.finish()?;
    assert_eq!(
        header
            .pointer("/binding/path/value")
            .and_then(Value::as_str),
        Some(TXTP),
        "bound artifact name"
    );
    assert_eq!(
        header.pointer("/binding/sha256").and_then(Value::as_str),
        Some(digest.sha256()),
        "actual TXTP hash"
    );
    assert_eq!(
        header
            .pointer("/binding/byteLength")
            .and_then(Value::as_str),
        Some(bytes.len().to_string().as_str()),
        "actual TXTP size"
    );
    let rows = rows.collect::<Result<Vec<_>, _>>()?;
    for (path, code) in [
        (LOOPS_PATH, "omit-container-loops"),
        (TRANSITION_PATH, TRANSITION_CODE),
        (BLEND_PATH, BLEND_CODE),
    ] {
        let row = rows
            .iter()
            .find(|row| row.pointer("/address/field").and_then(Value::as_str) == Some(path))
            .ok_or("addressed TXTP diagnostic")?;
        check_address(row, &bytes, code, field(file, path)?)?;
    }
    Ok(())
}

fn check_address(row: &Value, bytes: &[u8], code: &str, origin: &Value) -> TestResult {
    let address = &row["address"];
    assert_eq!(
        address["type"], "textBytes",
        "address uses the actual text span"
    );
    let start = address["start"]
        .as_str()
        .ok_or("start byte")?
        .parse::<usize>()?;
    let end = address["endExclusive"]
        .as_str()
        .ok_or("end byte")?
        .parse::<usize>()?;
    assert_eq!(
        bytes.get(start..end),
        Some(code.as_bytes()),
        "exact code bytes"
    );
    for key in [
        "containsEncryptedSource",
        "sourceSetComplete",
        "sourceUsesRefs",
    ] {
        assert_eq!(&row[key], &origin[key], "report and sidecar share {key}");
    }
    Ok(())
}

fn extract(archive: &Path, out: &Path, conflict: &str) -> Result<Value, Box<dyn Error>> {
    let result = p4kx_testkit::p4kx()?
        .arg("--json")
        .arg("--key")
        .arg(test_key().to_hex())
        .args(["wwise", "extract"])
        .arg(archive)
        .arg(EVENT)
        .arg("--out")
        .arg(out)
        .args(["--format", "wav", "--txtp", "--conflict", conflict])
        .output()?;
    assert_eq!(
        result.status.code(),
        Some(0),
        "synthetic export: {result:?}"
    );
    Ok(serde_json::from_slice(&result.stdout)?)
}

fn txtp_file(report: &Value) -> Result<&Value, Box<dyn Error>> {
    report
        .pointer("/txtp/files/0")
        .ok_or_else(|| "TXTP file report".into())
}

fn field<'a>(file: &'a Value, path: &str) -> Result<&'a Value, Box<dyn Error>> {
    file["fieldOrigins"]
        .get(path)
        .ok_or_else(|| format!("origin of {path}").into())
}

fn hirc_extent(bytes: &[u8], id: u32) -> Result<Range<usize>, Box<dyn Error>> {
    let bank = Soundbank::parse(bytes)?;
    let object = bank
        .objects()
        .iter()
        .find(|object| object.id == id)
        .ok_or("synthetic HIRC object")?;
    Ok(object.id_byte_range().start..object.byte_range().end)
}

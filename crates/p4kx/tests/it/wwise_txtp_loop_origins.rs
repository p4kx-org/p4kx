// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::ops::Range;
use std::path::Path;

use p4kx_testkit::{test_key, wwise_short_id};
use serde_json::{Value, json};

use super::TestResult;
use super::txtp_origin_assertions::{check_address, check_unknown};

#[path = "wwise_txtp_loop_origin_fixture.rs"]
mod fixture;

use self::fixture::{BANK, EMBEDDED_WEM, Fixture, LOOSE, LOOSE_WEM};

const EVENT: &str = "Play_Loop_Origin";
const TXTP: &str = "Play_Loop_Origin.txtp";
const RECEIPT: &str = "Play_Loop_Origin.txtp.p4kx-origin.jsonl";
const EMBEDDED_START: &str = "/commands/0/loop/start";
const EMBEDDED_END: &str = "/commands/0/loop/end";
const LOOSE_START: &str = "/commands/1/loop/start";
const LOOSE_END: &str = "/commands/1/loop/end";
const LOOP_PATHS: [&str; 4] = [EMBEDDED_START, EMBEDDED_END, LOOSE_START, LOOSE_END];

#[test]
fn txtp_loop_markers_bind_exact_tokens_to_the_actual_loose_and_embedded_inputs() -> TestResult {
    let fixture = fixture::create()?;
    for (format, extension) in [("wav", "wav"), ("raw", "wem")] {
        let out = fixture.target.join(format);
        let report = export(&fixture.archive, &out, format, "overwrite")?;
        let file = txtp_file(&report)?;
        check_commands(file);
        let info = Output::read(&out, file)?;
        assert_eq!(info.text, expected_text(extension), "exact TXTP text");
        info.check_loop_fields(file)?;
        check_embedded_sources(file, &fixture)?;
        check_loose_sources(file, &fixture)?;
    }
    Ok(())
}

#[test]
fn txtp_written_over_retained_audio_keeps_loop_source_unknown() -> TestResult {
    let fixture = fixture::create()?;
    let out = &fixture.target;
    export(&fixture.archive, out, "wav", "overwrite")?;
    let initial = fs::read(out.join(TXTP))?;
    fs::remove_file(out.join(RECEIPT))?;
    fs::remove_file(out.join(TXTP))?;

    let report = export(&fixture.alternate, out, "wav", "if-changed")?;
    assert_eq!(
        report.get("kept").and_then(Value::as_u64),
        Some(2),
        "both audio payloads were kept"
    );
    let file = txtp_file(&report)?;
    assert_eq!(
        file.get("kept").and_then(Value::as_bool),
        Some(false),
        "the TXTP and its receipt were written again"
    );
    check_commands(file);
    let info = Output::read(out, file)?;
    assert_eq!(
        info.text.as_bytes(),
        initial,
        "the TXTP bytes are unchanged"
    );
    info.check_loop_fields(file)?;
    assert_eq!(
        origin(file, "/approximations/0")?
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(false),
        "the independently rendered diagnostic has the new plain bank proof"
    );
    for path in LOOP_PATHS {
        check_unknown(origin(file, path)?);
    }
    Ok(())
}

fn export(
    archive: &Path,
    out: &Path,
    format: &str,
    conflict: &str,
) -> Result<Value, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?
        .arg("--json")
        .arg("--key")
        .arg(test_key().to_hex())
        .args(["wwise", "extract"])
        .arg(archive)
        .arg(EVENT)
        .arg("--out")
        .arg(out)
        .args(["--format", format, "--txtp", "--conflict", conflict])
        .output()?;
    assert_eq!(output.status.code(), Some(0), "exit code: {output:?}");
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn txtp_file(report: &Value) -> Result<&Value, Box<dyn Error>> {
    report
        .pointer("/txtp/files/0")
        .ok_or_else(|| "TXTP report".into())
}

fn check_commands(file: &Value) {
    assert_eq!(
        file.get("commands"),
        Some(&json!([{"loop": {"start": 12, "end": 64}}, {"loop": {"start": 9, "end": 192}}])),
        "each rendered sound carries its start and exclusive end separately"
    );
}

fn expected_text(extension: &str) -> String {
    format!(
        "# P4KX: event {EVENT} of trigger {EVENT}\n\
         # approximation: omit-container-loops\n\
         \x20 {EVENT}_{EMBEDDED_WEM}.{extension} #I 12 64\n\
         \x20 {EVENT}_{LOOSE_WEM}.{extension} #I 9 192\n\
         # event {}\n\
         group = -L2 #@layer-v\n",
        wwise_short_id(EVENT)
    )
}

struct Output {
    text: String,
    fields: Vec<Value>,
}

impl Output {
    fn read(out: &Path, file: &Value) -> Result<Self, Box<dyn Error>> {
        assert_eq!(
            file.get("originReceipt").and_then(Value::as_str),
            Some(RECEIPT),
            "canonical sibling receipt"
        );
        let fields = fs::read_to_string(out.join(RECEIPT))?
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<Vec<_>, _>>()?;
        let text = fs::read_to_string(out.join(TXTP))?;
        Ok(Self { text, fields })
    }

    fn check_loop_fields(&self, file: &Value) -> TestResult {
        for ExpectedSpan {
            path,
            token,
            offset,
        } in expected_spans(&self.text)?
        {
            let row = self
                .fields
                .iter()
                .find(|row| row.pointer("/address/field").and_then(Value::as_str) == Some(path))
                .ok_or_else(|| format!("receipt text address for {path}"))?;
            let actual = check_address(row, self.text.as_bytes(), token, origin(file, path)?)?;
            assert_eq!(actual, offset, "{path} covers the actual command token");
        }
        Ok(())
    }
}

struct ExpectedSpan {
    path: &'static str,
    token: &'static str,
    offset: Range<usize>,
}

fn expected_spans(text: &str) -> Result<[ExpectedSpan; 4], Box<dyn Error>> {
    let first = text.find(" #I 12 64\n").ok_or("embedded loop command")? + " #I ".len();
    let second = text.find(" #I 9 192\n").ok_or("loose loop command")? + " #I ".len();
    Ok([
        ExpectedSpan {
            path: EMBEDDED_START,
            token: "12",
            offset: first..first + 2,
        },
        ExpectedSpan {
            path: EMBEDDED_END,
            token: "64",
            offset: first + 3..first + 5,
        },
        ExpectedSpan {
            path: LOOSE_START,
            token: "9",
            offset: second..second + 1,
        },
        ExpectedSpan {
            path: LOOSE_END,
            token: "192",
            offset: second + 2..second + 5,
        },
    ])
}

fn check_embedded_sources(file: &Value, fixture: &Fixture) -> TestResult {
    let start = contributors(file, EMBEDDED_START, BANK, true)?;
    let end = contributors(file, EMBEDDED_END, BANK, true)?;
    let selected = [1, 1, fixture.embedded_smpl, 1];
    expect_site(&start, "wemLoop", "start", "payload", &selected);
    expect_site(&end, "wemLoop", "lastFrame", "conversion", &selected);
    for refs in [&start, &end] {
        for field in ["mediaId", "dataOffset", "dataLength"] {
            expect_site(refs, "bnkDidx", field, "selector", &[fixture.didx_triplet]);
        }
        for field in ["tag", "bodyLength"] {
            expect_site(refs, "bnkSection", field, "selector", &[fixture.bank_data]);
        }
    }
    Ok(())
}

fn check_loose_sources(file: &Value, fixture: &Fixture) -> TestResult {
    let start = contributors(file, LOOSE_START, LOOSE, false)?;
    let end = contributors(file, LOOSE_END, LOOSE, false)?;
    let selected = [fixture.loose_smpl, 0];
    expect_site(&start, "wemLoop", "start", "payload", &selected);
    expect_site(&end, "wemLoop", "lastFrame", "selector", &selected);
    for observation in start.iter().chain(&end) {
        let namespace = observation
            .pointer("/location/namespace")
            .and_then(Value::as_str);
        assert!(
            !matches!(namespace, Some("bnkDidx" | "bnkSection")),
            "a loose WEM has no embedded bank routing inputs"
        );
    }
    expect_site(
        &end,
        "wemChunk",
        "bodyLength",
        "conversion",
        &[fixture.loose_data],
    );
    Ok(())
}

fn expect_site(uses: &[&Value], namespace: &str, field: &str, role: &str, indices: &[usize]) {
    let indices = json!(indices.iter().map(ToString::to_string).collect::<Vec<_>>());
    assert!(
        uses.iter().any(|observation| {
            observation
                .pointer("/location/namespace")
                .and_then(Value::as_str)
                == Some(namespace)
                && observation
                    .pointer("/location/field")
                    .and_then(Value::as_str)
                    == Some(field)
                && observation.pointer("/location/indices") == Some(&indices)
                && observation.get("role").and_then(Value::as_str) == Some(role)
        }),
        "missing original {namespace}/{field} at {indices} as {role}: {uses:?}"
    );
}

fn origin<'a>(file: &'a Value, path: &str) -> Result<&'a Value, Box<dyn Error>> {
    file.get("fieldOrigins")
        .and_then(|fields| fields.get(path))
        .ok_or_else(|| format!("origin for {path}").into())
}

fn contributors<'a>(
    file: &'a Value,
    path: &str,
    physical_path: &str,
    encrypted: bool,
) -> Result<Vec<&'a Value>, Box<dyn Error>> {
    let field = origin(file, path)?;
    assert_eq!(
        field
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(encrypted),
        "{path} marks its own source's encryption state"
    );
    assert_eq!(
        field.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "{path} has complete source evidence"
    );
    let references = field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("loop member's source uses")?;
    let uses = file
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    let sources = file
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("physical sources")?;
    references
        .iter()
        .map(|reference| {
            let observation = uses
                .iter()
                .find(|observation| observation.get("id") == Some(reference))
                .ok_or("an actual field use")?;
            let input = sources
                .iter()
                .find(|input| input.get("id") == observation.get("source"))
                .ok_or("physical input")?;
            assert_eq!(
                input.pointer("/path/value").and_then(Value::as_str),
                Some(physical_path),
                "each coordinate belongs to the actual input for {path}"
            );
            Ok(observation)
        })
        .collect()
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_media::{ExportOptions, MediaKind, MediaRecord};
use p4kx_testkit::TestEntry;
use serde_json::Value;

use super::support::{self, TestResult};

#[test]
fn mannequin_roles_retain_original_alternatives_transitions_and_attributes() -> TestResult {
    let bytes = br#"<AnimDB xmlns:fixture="urn:fixture">
      <FragmentList><FixtureGroup>
        <Fragment Tags="A" OptionWeight="0.75" fixture:weight="unknown">
          <AnimLayer><Blend Duration="0.10"/><Animation name="same"/></AnimLayer>
          <ProcLayer><Blend ExitTime="0.23"/><Procedural type="synthetic"><Params opacity="0.1"/></Procedural></ProcLayer>
        </Fragment>
        <Fragment Tags="A" OptionWeight="1"><AnimLayer><Animation name="same"/></AnimLayer></Fragment>
      </FixtureGroup></FragmentList>
      <FragmentBlendList><Blend from="A" to="B"><OptionA selectTime="0.1">
        <Fragment TransitionTags="B"><AnimLayer><Blend StartTime="-1"/></AnimLayer></Fragment>
      </OptionA></Blend></FragmentBlendList>
      <SubADBs><SubADB File="Animations/graph.adb" Tags="A"/></SubADBs>
    </AnimDB>"#;
    let rows = export([TestEntry::stored("Data/Animations/graph.adb", bytes)])?;
    assert_eq!(
        fields(&rows, "option-weight"),
        ["0.75", "1"],
        "ordered weighted alternatives"
    );
    assert_eq!(
        fields(&rows, "animation-name"),
        ["same", "same"],
        "duplicate clips survive"
    );
    assert_eq!(fields(&rows, "duration"), ["0.10"], "original precision");
    assert_eq!(fields(&rows, "from"), ["A"], "transition selector");
    assert_eq!(fields(&rows, "start-time"), ["-1"], "no timing conversion");
    assert_eq!(
        fields(&rows, "unknown"),
        ["unknown"],
        "namespace has its own original field"
    );
    assert!(
        role(&rows, "transition-option").is_some(),
        "source option label stays addressable"
    );
    assert!(
        role(&rows, "procedural").is_some(),
        "original procedural root role"
    );
    assert!(
        rows.iter().any(|row| row
            .data
            .pointer("/payload/qualified_name")
            .and_then(Value::as_str)
            == Some("opacity")),
        "arbitrary procedural subtree remains in the raw ordered row stream"
    );
    Ok(())
}

#[test]
fn blend_space_roles_preserve_examples_faces_and_no_caf_inference() -> TestResult {
    let rows = export([
        TestEntry::stored("Data/Animations/fixture.bspace", br#"<ParaGroup version="1"><Dimensions>
          <Param Name="Speed" Min="-1" Max="8" Cells="9"/>
          </Dimensions><ExampleList><Example AName="a" Alias="Same" SetPara0="1.00"/>
          <Example AName="b" Alias="Same" SetPara0="1.00"/>
          </ExampleList><ExamplePseudo><Pseudo Alias="virtual" p0="0" w0=".5"/></ExamplePseudo>
          <Blendable><Face p0="0" p1="1"/></Blendable>
          <AdditionalExtraction><Param Name="Amount" JointName="test"/></AdditionalExtraction></ParaGroup>"#),
        TestEntry::stored("Data/Animations/fixture.comb", br#"<CombinedBlendSpace><Dimensions>
          <Param Name="Speed" WeightBlendSpace="1"/></Dimensions>
          <BlendSpaces><BlendSpace AName="Animations/fixture.bspace" fixedPara="2"/></BlendSpaces>
          </CombinedBlendSpace>"#),
    ])?;
    assert_eq!(
        fields(&rows, "alias"),
        ["Same", "Same", "virtual"],
        "repeated aliases retain order"
    );
    assert_eq!(
        fields(&rows, "parameter-assignment"),
        ["1.00", "1.00"],
        "lexical parameter values"
    );
    assert_eq!(fields(&rows, "face-point"), ["0", "1"], "ordered indices");
    assert_eq!(
        fields(&rows, "example-weight"),
        [".5"],
        "pseudo example weight"
    );
    assert_eq!(
        fields(&rows, "fixed-parameter"),
        ["2"],
        "combined space fixed parameter"
    );
    assert_blend_space_edges(&rows)
}

fn assert_blend_space_edges(rows: &[MediaRecord]) -> TestResult {
    let face = role(rows, "face").ok_or("face row")?;
    let parent = role(rows, "faces").ok_or("faces row")?;
    let parent_id = parent
        .subresource
        .indices
        .first()
        .ok_or("faces source identity")?
        .to_string();
    assert_eq!(
        face.data
            .pointer("/payload/location/parent")
            .and_then(Value::as_str),
        Some(parent_id.as_str()),
        "structural edge uses the source node index"
    );
    let links: Vec<_> = rows
        .iter()
        .filter(|row| row.data.get("record_type").and_then(Value::as_str) == Some("link"))
        .collect();
    let [space] = links.as_slice() else {
        return Err("one physical file reference expected".into());
    };
    assert_eq!(
        space
            .data
            .pointer("/payload/reference/resolution/status")
            .and_then(Value::as_str),
        Some("resolved"),
        "combined space joins the physical blend space"
    );
    Ok(())
}

fn fields<'a>(rows: &'a [MediaRecord], field: &str) -> Vec<&'a str> {
    rows.iter()
        .filter(|row| row.data.pointer("/payload/field").and_then(Value::as_str) == Some(field))
        .filter_map(|row| {
            row.data
                .pointer("/payload/declaration/value")
                .and_then(Value::as_str)
        })
        .collect()
}

fn role<'a>(rows: &'a [MediaRecord], role: &str) -> Option<&'a MediaRecord> {
    rows.iter()
        .find(|row| row.data.pointer("/payload/role").and_then(Value::as_str) == Some(role))
}

fn export(
    entries: impl IntoIterator<Item = TestEntry>,
) -> Result<Vec<MediaRecord>, Box<dyn std::error::Error>> {
    let scratch = tempfile::tempdir()?;
    let archive = support::archive_with(scratch.path(), entries)?;
    let output = scratch.path().join("media");
    let options = ExportOptions {
        kinds: vec![MediaKind::AnimationDefinition],
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "animation-definition.jsonl")?;
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in &rows {
        validator
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
    }
    Ok(rows)
}

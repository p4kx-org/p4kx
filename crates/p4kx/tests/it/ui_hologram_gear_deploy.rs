// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::fs;

use p4kx_testkit::{
    CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, TestAnimation, TestDba, TestElement,
    TestEntity, TestEntry, TestModel, TestMtl, TestMtlMaterial, TestSkeleton, TestTrack,
    entity_schema, landing_gear_entries, landing_system_record, test_key,
};
use serde_json::{Value, json};

use super::super::hologram_for;
use super::{TestResult, receipt, skip, source_uses, write_archive};

const ENTITY: &str = "Hologram_Gear_Deploy";
const GEAR_CDF: &str = "Objects/Test/deploy.cdf";
const GEAR_SKIN: &str = "Objects/Test/deploy.skin";
const PARAMS: &str = "Data\\Objects\\Test\\deploy.chrparams";
const ROTATION: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

#[derive(Clone, Copy)]
enum Route {
    Caf,
    Dba,
}

impl Route {
    const fn folder(self) -> &'static str {
        match self {
            Self::Caf => "Animations/a",
            Self::Dba => "Animations/z",
        }
    }

    const fn database(self) -> &'static str {
        match self {
            Self::Caf => "Animations/z/gear.dba",
            Self::Dba => "Animations/a/gear.dba",
        }
    }

    const fn clip_ordinal(self) -> &'static str {
        match self {
            Self::Caf => "0",
            Self::Dba => "2",
        }
    }

    const fn entries(self) -> (&'static str, &'static str) {
        match self {
            Self::Caf => ("gear_extend.caf", "gear.dba"),
            Self::Dba => ("gear.dba", "gear_extend.caf"),
        }
    }
}

fn parameter_document(route: Route) -> TestElement {
    let animation = |name, path| {
        TestElement::new("Animation")
            .attribute("name", name)
            .attribute("path", path)
    };
    let list = TestElement::new("AnimationList")
        .child(animation("#filepath", "Animations/unused"))
        .child(animation("$TracksDatabase", "Animations/unused.dba"))
        .child(animation("$TracksDatabase", route.database()))
        .child(animation("#filepath", route.folder()))
        .child(animation("gear_deploy", "loser.caf"))
        .child(animation("gear_extend_retract", "retract.caf"))
        .child(animation("gear_extend", "gear_extend.caf"))
        .child(animation("later_extend", "later.caf"));
    TestElement::new("Params").child(list)
}

fn parameters(route: Route) -> Vec<u8> {
    let document = parameter_document(route);
    match route {
        Route::Caf => document.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little),
        Route::Dba => format!("\u{feff}{}", document.to_xml()).into_bytes(),
    }
}

fn definition(hidden: bool) -> String {
    let material = if hidden {
        "Objects/Test/hidden.mtl"
    } else {
        "Objects/Test/paint.mtl"
    };
    format!(
        r#"<CharacterDefinition>
    <Model File="Objects/Test/deploy.chr"/>
    <AttachmentList>
        <Attachment Type="CA_SKIN" Binding="{GEAR_SKIN}" Material="{material}"/>
    </AttachmentList>
</CharacterDefinition>"#
    )
}

fn database() -> Vec<u8> {
    let entity = TestEntity::new(1, ENTITY, "entities/deploy.xml")
        .geometry("Objects/Test/ship.cgf", None)
        .vehicle("", "")
        .landing_system(0x10);
    entity_schema(DataForgeBuilder::new())
        .record(entity.record())
        .record(landing_system_record(
            0x10,
            "Gear_Deploy_System",
            &[("hardpoint_gear_front", GEAR_CDF, 1.0)],
        ))
        .build()
}

fn animation_entries(route: Route, early: f32, height: f32) -> [TestEntry; 2] {
    let clip = || {
        let tip = TestSkeleton::controller_id(1);
        TestAnimation::new("gear_extend.caf", 30)
            .track(TestTrack::new(tip))
            .track(
                TestTrack::new(tip)
                    .rotations(&[(1, ROTATION), (30, ROTATION)])
                    .positions(&[(0, [early, 0.0, 0.0]), (30, [0.0, 0.0, height])]),
            )
            .track(TestTrack::new(tip).positions(&[(0, [99.0; 3])]))
            .track(TestTrack::new(77).positions(&[(0, [78.0; 3])]))
    };
    let skipped = TestAnimation::new("corrupt.caf", 30).track(
        TestTrack::new(1)
            .rotation_code(0x99)
            .rotations(&[(0, ROTATION)]),
    );
    let pack = TestDba::new()
        .animation(skipped)
        .animation(clip().additive())
        .animation(clip());
    let caf = TestEntry::stored(
        format!("Data/{}/gear_extend.caf", route.folder()).replace('/', "\\"),
        clip().caf(),
    );
    let dba = TestEntry::stored(
        format!("Data/{}", route.database()).replace('/', "\\"),
        pack.bytes(),
    );
    match route {
        Route::Caf => [caf.encrypted(), dba],
        Route::Dba => [caf, dba.encrypted()],
    }
}

fn fixture(route: Route, hidden: bool, early: f32, height: f32) -> Vec<TestEntry> {
    let mut entries: Vec<_> = landing_gear_entries()
        .into_iter()
        .filter(|entry| entry.path().ends_with("ship.cgf") || entry.path().ends_with("ship.cgfm"))
        .collect();
    let skin = TestModel::two_bone_skin();
    let library = |shader| {
        let material = || TestMtlMaterial::new("body").shader(shader);
        TestMtl::multi(&[material(), material(), material()]).cryxml()
    };
    entries.extend([
        TestEntry::stored("Data\\Game2.dcb", database()),
        TestEntry::stored(
            "Data\\Objects\\Test\\deploy.cdf",
            definition(hidden).into_bytes(),
        ),
        TestEntry::stored("Data\\Objects\\Test\\deploy.chr", skin.cgf()),
        TestEntry::stored("Data\\Objects\\Test\\deploy.skin", skin.cgf()),
        TestEntry::stored("Data\\Objects\\Test\\deploy.skinm", skin.cgfm()),
        TestEntry::stored("Data\\Objects\\Test\\hidden.mtl", library("NoDraw")),
        TestEntry::stored("Data\\Objects\\Test\\paint.mtl", library("Illum")),
        TestEntry::stored(PARAMS, parameters(route)),
    ]);
    entries.extend(animation_entries(route, early, height));
    entries
}

struct Observation {
    image: Vec<u8>,
    rows: Vec<Value>,
    report: Value,
}

impl Observation {
    fn uses(&self, file: &str) -> Vec<&Value> {
        source_uses(&self.rows, &self.report, file)
    }
}

fn observed(
    route: Route,
    hidden: bool,
    early: f32,
    height: f32,
    name: &str,
) -> Result<Option<Observation>, Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), fixture(route, hidden, early, height))?;
    let output = dir.path().join("ship.png");
    let key = test_key().to_hex();
    let run = hologram_for(
        &archive,
        &output,
        ENTITY,
        &["--size", "96", "--key", &key],
        true,
    )?;
    if skip(name, &run) {
        return Ok(None);
    }
    assert_eq!(run.code, Some(0), "{}", run.stderr);
    let report: Value = serde_json::from_str(&run.stdout)?;
    Ok(Some(Observation {
        image: fs::read(&output)?,
        rows: receipt(&output, &report)?,
        report,
    }))
}

#[test]
fn only_the_winning_physical_animation_and_last_used_keys_supply_the_pose() -> TestResult {
    let name = "only_the_winning_physical_animation_and_last_used_keys_supply_the_pose";
    let Some(caf) = observed(Route::Caf, false, 0.0, 4.0, name)? else {
        return Ok(());
    };
    let Some(dba) = observed(Route::Dba, false, 0.0, 4.0, name)? else {
        return Ok(());
    };
    assert_eq!(
        caf.image, dba.image,
        "the same decoded keys make the same PNG"
    );
    for (route, run) in [(Route::Caf, caf), (Route::Dba, dba)] {
        assert_animation(route, &run);
        assert_parameters(route, &run)?;
    }
    Ok(())
}

fn assert_animation(route: Route, run: &Observation) {
    let (selected, unselected) = route.entries();
    let uses = run.uses(selected);
    assert!(
        run.uses(unselected).is_empty(),
        "the competing clip is never used"
    );
    assert_eq!(
        run.report
            .pointer("/artifactOrigin/containsEncryptedSource"),
        Some(&json!(true)),
        "only the selected clip is encrypted"
    );
    let clip = route.clip_ordinal();
    let conversions: Vec<_> = uses
        .iter()
        .filter(|source| source.get("role").and_then(Value::as_str) == Some("conversion"))
        .collect();
    assert_eq!(conversions.len(), 2, "only one controller's two channels");
    for field in ["rotation", "translation"] {
        assert!(
            conversions.iter().any(|site| site.get("location")
                == Some(&json!({
                    "type": "subresource",
                    "namespace": "gearDeployKey",
                    "indices": [clip, "1", "1"],
                    "field": field,
                }))),
            "last physical {field} key of the admitted controller"
        );
    }
    assert!(
        uses.iter().any(|site| site.get("location")
            == Some(&json!({
                "type": "subresource", "namespace": "gearDeployController",
                "indices": [clip, "1"], "field": "controllerId",
            }))),
        "the original binding selector"
    );
}

fn assert_parameters(route: Route, run: &Observation) -> TestResult {
    let params = run.uses("deploy.chrparams");
    let fields: BTreeSet<_> = params
        .iter()
        .filter_map(|site| site.pointer("/location/field")?.as_str())
        .collect();
    let expected = match route {
        Route::Caf => ["clipName", "clipPath", "folderName", "folderPath"],
        Route::Dba => ["clipName", "clipPath", "databaseName", "databasePath"],
    };
    assert_eq!(
        fields,
        BTreeSet::from(expected),
        "only the used parameter route"
    );
    for site in params {
        let location = site.get("location").ok_or("parameter site")?;
        let field = location
            .get("field")
            .and_then(Value::as_str)
            .ok_or("parameter field")?;
        let indices = match route {
            Route::Caf => cryxml_indices(field)?,
            Route::Dba => text_indices(field, route)?,
        };
        assert_eq!(location.get("indices"), Some(&indices), "original {field}");
    }
    let location = json!({
        "type": "subresource", "namespace": "cdfModelTextBytes",
        "indices": attribute_bytes(&definition(false), "File", "Objects/Test/deploy.chr")?,
        "field": "File",
    });
    assert!(
        run.uses("deploy.cdf")
            .iter()
            .any(|site| site.get("location") == Some(&location)),
        "the original CDF File selected this pose's skeleton parameters"
    );
    Ok(())
}

fn cryxml_indices(field: &str) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(match field {
        "clipName" => json!(["8", "12"]),
        "clipPath" => json!(["8", "13"]),
        "folderName" => json!(["5", "6"]),
        "folderPath" => json!(["5", "7"]),
        _ => return Err("unexpected CryXml selector".into()),
    })
}

fn text_indices(field: &str, route: Route) -> Result<Value, Box<dyn std::error::Error>> {
    let (attribute, value) = match field {
        "clipName" => ("name", "gear_extend"),
        "clipPath" => ("path", "gear_extend.caf"),
        "databaseName" => ("name", "$TracksDatabase"),
        "databasePath" => ("path", route.database()),
        _ => return Err("unexpected text selector".into()),
    };
    attribute_bytes(&String::from_utf8(parameters(route))?, attribute, value)
}

fn attribute_bytes(
    source: &str,
    attribute: &str,
    value: &str,
) -> Result<Value, Box<dyn std::error::Error>> {
    let exact = format!("{attribute}=\"{value}\"");
    let start = source.rfind(&exact).ok_or("original selector bytes")?;
    Ok(json!([
        start.to_string(),
        (start + exact.len()).to_string()
    ]))
}

#[test]
fn only_rendered_pose_keys_change_the_image_and_their_source_marker() -> TestResult {
    let name = "only_rendered_pose_keys_change_the_image_and_their_source_marker";
    let Some(first) = observed(Route::Dba, false, 0.0, 1.0, name)? else {
        return Ok(());
    };
    let Some(early) = observed(Route::Dba, false, 9.0, 1.0, name)? else {
        return Ok(());
    };
    let Some(last) = observed(Route::Dba, false, 0.0, 4.0, name)? else {
        return Ok(());
    };
    let Some(hidden) = observed(Route::Dba, true, 0.0, 4.0, name)? else {
        return Ok(());
    };
    assert_eq!(first.image, early.image, "an earlier key is not applied");
    assert_ne!(
        first.image, last.image,
        "the last used key supplies rendered positions"
    );
    for file in ["gear.dba", "gear_extend.caf", "deploy.chrparams"] {
        assert!(hidden.uses(file).is_empty(), "no consumed pose from {file}");
    }
    assert!(
        !hidden.uses("deploy.cdf").iter().any(|site| {
            site.pointer("/location/field").and_then(Value::as_str) == Some("File")
        }),
        "the CDF skeleton selector cannot claim the suppressed pose"
    );
    assert_eq!(
        hidden
            .report
            .pointer("/artifactOrigin/containsEncryptedSource"),
        Some(&Value::Null),
        "a fully suppressed pose cannot mark the PNG encrypted-derived"
    );
    Ok(())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::TestEntry;
use serde_json::Value;

use super::TestResult;

pub(super) fn entries(root_links: &str) -> [TestEntry; 3] {
    let controller = br#"<ControllerDef><Fragments filename="Animations/Test/fragments.xml"/>
        <FragmentDefs><Walk/><Walk/><Idle scopes="Body+Head"/></FragmentDefs>
        <ScopeDefs><Head/><Body/></ScopeDefs></ControllerDef>"#;
    let database = format!(
        r#"<AnimDB FragDef="animations\test\fragments.xml"><FragmentList>
        <Walk><Fragment Tags="one"/><Fragment Tags="two"/></Walk>
        <Idle><Fragment FragTags="rest"/></Idle>
        <Missing><Fragment/></Missing>
    </FragmentList>{root_links}</AnimDB>"#
    );
    let fragment_definitions = br"<FragmentDefinition><Walk/><Idle/></FragmentDefinition>";
    [
        TestEntry::stored("Data/Animations/test/controller.xml", controller),
        TestEntry::stored("Data/Animations/test/player.adb", database.into_bytes()),
        TestEntry::stored("Data/Animations/test/fragments.xml", fragment_definitions).encrypted(),
    ]
}

pub(super) fn verify(document: &Value) -> TestResult {
    let motion = document
        .pointer("/motion/components/value/0/authoredOptions")
        .ok_or("authored options")?;
    assert_eq!(
        motion
            .pointer("/sharedFragmentFile/value/type")
            .and_then(Value::as_str),
        Some("present"),
        "both selectors designate a single original physical document",
    );
    let options = motion
        .get("options")
        .and_then(Value::as_array)
        .ok_or("option rows")?;
    let [one, two, idle, unmatched] = options.as_slice() else {
        return Err("expected all four original options".into());
    };
    repeated_options(one, two);
    exact_scope(idle)?;
    assert_eq!(
        unmatched
            .pointer("/controllerFragments/value")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0),
        "source option without a same-name controller declaration remains visible",
    );
    Ok(())
}

fn repeated_options(one: &Value, two: &Value) {
    for (option, tag) in [(one, "one"), (two, "two")] {
        assert_eq!(
            option.pointer("/tags/value").and_then(Value::as_str),
            Some(tag),
            "original option order and tag spelling",
        );
        assert_eq!(
            option
                .pointer("/controllerFragments/value")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(2),
            "duplicate controller names are retained in source order",
        );
        assert_eq!(
            option.pointer("/scopes/state").and_then(Value::as_str),
            Some("unsupported"),
            "competing controller fragments do not choose a scope",
        );
    }
}

fn exact_scope(idle: &Value) -> TestResult {
    assert_eq!(
        idle.pointer("/fragmentTags/value").and_then(Value::as_str),
        Some("rest"),
        "original fragment tags remain separate from option tags",
    );
    let scopes = idle
        .pointer("/scopes/value")
        .and_then(Value::as_array)
        .ok_or("scope targets")?;
    let [body, head] = scopes.as_slice() else {
        return Err("expected two explicit scope matches".into());
    };
    let start = |value: &Value| {
        value
            .pointer("/value/candidates/0/start")
            .and_then(Value::as_str)
            .and_then(|start| start.parse::<usize>().ok())
    };
    assert!(
        start(body)
            .zip(start(head))
            .is_some_and(|(body, head)| body > head),
        "scope targets follow the order of the original selector, independent of definition order",
    );
    assert_eq!(
        idle.pointer("/scopes/containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "the physical shared definition and actor evidence are retained",
    );
    Ok(())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;
use crate::template_canonical_graph::Template;
use crate::template_fixture_context as context;
use crate::template_fixture_fields as fields;
use crate::template_fixture_tags as tag_paths;
use crate::template_fixture_text as texts;

pub(crate) fn string(catalog: &Value, graph: &Template<'_>, value: &Value) -> TestResult {
    let options = fields::elements(graph, graph.field(value, "options")?)?;
    assert_eq!(options.len(), 1, "one original StringHash option");
    let option = *options.first().ok_or("actual original StringHash option")?;
    texts::localized(
        catalog,
        graph,
        graph.field(option, "textId")?,
        "@place_name",
        Some("Hijacked Ship"),
        None,
    )?;
    assert_eq!(
        fields::text(graph.field(option, "value")?)?,
        "calm",
        "the authored String"
    );
    fields::numeric_fields(graph, option, &[("weighting", json!(3.0))])?;
    fields::scalar_fields(
        graph,
        option,
        &[("DEBUG_forceChooseThisOption", "boolean", json!(false))],
    )?;
    let dependencies = fields::elements(graph, graph.field(option, "dependentProperties")?)?;
    assert_eq!(dependencies.len(), 1, "the original dependency remains");
    let dependency = *dependencies.first().ok_or("actual dependency")?;
    let caller = graph.field(dependency, "property")?;
    context::property_target(graph, caller, "Home_BP")?;
    let binding = context::context(graph, "propertyBindings", "caller", caller)?;
    assert_eq!(
        raw::present(
            binding
                .get("missionPropertyStatus")
                .ok_or("strict property status")?
        )?,
        &json!("resolved"),
        "the established same-owner property status"
    );
    assert!(
        raw::elements(graph.field(dependency, "values")?)?.is_empty(),
        "original option selection empty"
    );
    Ok(())
}

pub(crate) fn tags(graph: &Template<'_>, value: &Value) -> TestResult {
    let positive = fields::elements(graph, graph.field(graph.field(value, "tags")?, "tags")?)?;
    assert_eq!(positive.len(), 1, "the original positive tag list");
    let tag = *positive.first().ok_or("actual positive tag")?;
    assert_eq!(
        fields::scalar(tag, "recordReference")?.get("status"),
        Some(&json!("present")),
        "the actual original raw tag identity remains present"
    );
    tag_paths::path(graph, tag, Some("Kill"))?;
    assert!(
        raw::elements(graph.field(graph.field(value, "negativeTags")?, "tags")?)?.is_empty(),
        "the real negative list remains empty"
    );
    assert!(
        raw::elements(graph.field(value, "options")?)?.is_empty(),
        "the real option list remains empty"
    );
    Ok(())
}

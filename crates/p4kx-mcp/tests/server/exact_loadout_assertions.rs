// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{Attachment, EntityNode};
use serde_json::{Value, json};

use crate::exact_loadout_fixture::Texts;
use crate::support::{Server, TestResult};

pub(crate) fn exact(output: &Value, node: &EntityNode, texts: &Texts) -> TestResult {
    for (path, expected) in [
        ("/entity/class_name", node.class_name()),
        ("/entity/name", node.name()),
        ("/entity/record_path", node.record_path()),
        ("/ports/0/path", texts.port.as_str()),
        ("/ports/0/name", texts.port.as_str()),
        ("/ports/0/definition/display_name", texts.display.as_str()),
        ("/ports/0/definition/flags", texts.flags.as_str()),
        ("/ports/0/definition/helper", texts.helper.as_str()),
        ("/ports/1/unresolved/class_name", texts.missing.as_str()),
    ] {
        assert_eq!(
            output.pointer(path),
            Some(&json!(expected)),
            "complete original datum {path}"
        );
    }
    let Attachment::Entity(child) = node
        .ports()
        .first()
        .ok_or("original first port")?
        .attachment()
    else {
        return Err("actual child".into());
    };
    assert_eq!(
        output.pointer("/ports/0/entity/class_name"),
        Some(&json!(child.class_name())),
        "the attached Entity's full identity"
    );
    let (name, subtypes) = texts.types.first().ok_or("original first type")?;
    assert_eq!(
        output.pointer("/ports/0/definition/types/0/name"),
        Some(&json!(name)),
        "full type"
    );
    assert_eq!(
        output.pointer("/ports/0/definition/types/0/sub_types"),
        Some(&json!(subtypes)),
        "full subtypes"
    );
    assert_eq!(
        output.pointer("/ports/1/unresolved/reason"),
        Some(&json!(format!(
            "no Entity has the class name {}",
            texts.missing
        ))),
        "the actual Display reason remains complete"
    );
    assert_eq!(
        output.get("total"),
        Some(&json!(2)),
        "no source port was dropped"
    );
    assert_eq!(
        output.pointer("/counts/unresolved"),
        Some(&json!(1)),
        "original failure count"
    );
    Ok(())
}

pub(crate) fn roundtrip(server: &mut Server, output: &Value, node: &EntityNode) -> TestResult {
    for path in ["/entity/name", "/entity/class_name", "/entity/record_path"] {
        let identifier = output
            .pointer(path)
            .and_then(Value::as_str)
            .ok_or("returned identifier")?;
        assert!(
            identifier.len() <= 1024,
            "this original identifier is within the existing input cap"
        );
        let again = server.ok("entity_loadout", &json!({"entity":identifier,"limit":1}))?;
        assert_eq!(
            again.pointer("/entity/guid"),
            Some(&json!(node.guid().to_string())),
            "the full returned {path} selects the same actual Record"
        );
    }
    Ok(())
}

pub(crate) fn too_large(server: &mut Server, arguments: &Value) -> TestResult {
    let error = server.error("entity_loadout", arguments)?;
    assert!(
        error.contains("response cap of 262144 bytes"),
        "explicit existing budget: {error}"
    );
    assert!(
        error.len() < 1024,
        "failure metadata does not clone the enormous source identity"
    );
    Ok(())
}

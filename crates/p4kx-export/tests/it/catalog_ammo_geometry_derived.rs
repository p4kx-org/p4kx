// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use serde_json::Value;

use super::{MODEL_TAG, check_reference, database};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[test]
fn ammo_geometry_accepts_all_known_original_model_tag_derivatives() -> TestResult {
    for (source_type, output_type) in [
        ("SGeometryActorModelTag", "actor"),
        ("GeometryPropModelTag", "prop"),
        ("GeometryAsteroidModelTag", "asteroid"),
    ] {
        let (_folder, archive) = common::open(&database(source_type), DcbOrigin::Encrypted)?;
        let built = build_catalog(&archive, &common::options())?;
        let catalog: Value = serde_json::from_slice(built.json())?;
        let tag = proof::at(
            &catalog,
            "/definitions/ammoDefinitions/rows/0/geometryResourceParams/value/modelTag/value",
        )?;
        assert_eq!(
            proof::at(tag, "/type")?,
            output_type,
            "original model tag subtype {source_type} is represented"
        );
        check_reference(proof::at(tag, "/tag/value")?, MODEL_TAG)?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

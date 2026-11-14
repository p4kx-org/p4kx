// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForge;
use p4kx_testkit::DataForgeBuilder;
use serde_json::json;

use crate::commodity_v3_fixture::TestResult;
use crate::quality_v3_fixture_schema as schema;
use crate::quality_v3_fixture_values as fixture;
use crate::quality_v3_origin_support::{self as origins, Domain, Use};
use crate::quality_v3_support as support;

#[test]
fn quality_v3_inverse_excludes_other_guid_payload_but_retains_its_identity_selector() -> TestResult
{
    let database = database();
    assert_original_identities(&database)?;
    let (_, catalog) = support::diagnostic(&database)?;
    let winner = support::row(&catalog, support::INVERSE, 1)?;
    let members = support::at(winner, "/resourceTypeGuids")?;
    assert_eq!(
        support::at(members, "/value/0/value")?,
        &json!(support::guid(fixture::RESOURCE_A)),
        "the selected group's actual inverse is unchanged by an earlier different GUID"
    );
    assert_eq!(
        support::at(members, "/value")?.as_array().map(Vec::len),
        Some(1),
        "the relation contains exactly its actual matching resource"
    );
    support::marker(members, Some(false))?;
    assert_eq!(
        support::at(members, "/sourceSetComplete")?,
        &json!(true),
        "the complete contributor set is narrower than all eagerly evaluated candidates"
    );
    let used = origins::uses(&catalog, members)?;
    assert!(
        used.contains(&Use::new(Domain::Header, 0, "guid", "selector")),
        "the earlier candidate's actual original GUID establishes inequality"
    );
    assert!(
        used.contains(&Use::new(Domain::Header, 1, "guid", "selector")),
        "the winning original GUID selects the inverse group"
    );
    assert!(
        used.contains(&Use::new(
            Domain::Body,
            1,
            "qualityQuantization.bands[0].mappedValue",
            "selector"
        )),
        "the actual winning band's eligibility still contributes"
    );
    assert_no_other_guid_payload(&used);
    Ok(())
}

fn database() -> DataForgeBuilder {
    schema::schema()
        .record(fixture::quantization(
            0x53,
            "Different",
            "different",
            fixture::bands(vec![fixture::band(0, 99, 77)]),
        ))
        .record(fixture::quantization(
            fixture::QUANTIZATION,
            "Winner",
            "winner",
            fixture::bands(vec![fixture::band(0, 99, 20)]),
        ))
        .record(fixture::resource(
            fixture::RESOURCE_A,
            "resource_a",
            vec![fixture::property(Some(fixture::QUANTIZATION))],
        ))
}

fn assert_original_identities(database: &DataForgeBuilder) -> TestResult {
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    let records = forge.records().take(2).collect::<Vec<_>>();
    let [earlier, winner] = records.as_slice() else {
        return Err("two original quantizers".into());
    };
    assert_eq!(
        (earlier.index(), winner.index()),
        (0, 1),
        "the prefix uses actual original Record order"
    );
    assert_eq!(
        earlier.guid().to_string(),
        support::guid(0x53),
        "the prior GUID comes from its original header"
    );
    assert_eq!(
        winner.guid().to_string(),
        support::guid(fixture::QUANTIZATION),
        "the matching GUID comes from a different original header"
    );
    assert_ne!(
        earlier.guid(),
        winner.guid(),
        "the two candidate identities are physically and nominally distinct"
    );
    Ok(())
}

fn assert_no_other_guid_payload(used: &std::collections::BTreeSet<Use>) {
    for role in ["payload", "selector", "conversion"] {
        for field in [
            "qualityQuantization",
            "qualityQuantization.$type",
            "qualityQuantization.bands",
            "qualityQuantization.bands[0]",
            "qualityQuantization.bands[0].$type",
            "qualityQuantization.bands[0].start",
            "qualityQuantization.bands[0].end",
            "qualityQuantization.bands[0].mappedValue",
        ] {
            assert!(
                !used.contains(&Use::new(Domain::Body, 0, field, role)),
                "a different-GUID candidate's unrelated band payload cannot contribute to this group's first winner"
            );
        }
    }
}

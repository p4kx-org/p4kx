// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The ship store slug entries of the sample catalog.

use serde_json::json;

pub(super) fn ship_store_pledges() -> serde_json::Value {
    json!({
        "snapshot": {
            "source": "https://robertsspaceindustries.com/ship-matrix/index",
            "captureDate": "2026-10-28",
            "sourceRevision": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "reviewedDate": "2026-11-24",
            "revision": 1,
            "scope": "reviewedCandidates"
        },
        "rows": [{
            "rsiId": 3,
            "slug": "Fixture-Concept",
            "storeName": "Fixture Concept",
            "manufacturerCode": "TEST",
            "productionStatus": "in-concept"
        }]
    })
}

pub(super) fn ship_store_slugs() -> serde_json::Value {
    json!([{
        "slug": "Aurora-Mk-I-ES",
        "rsiId": 1,
        "storeName": "Aurora Mk I ES",
        "className": "RSI_Aurora_GS_ES",
        "shipGuid": "cfc0122d-5275-415a-a656-3dcb3346feb5",
        "retired": false
    }, {
        "slug": "Retired-Ship",
        "rsiId": 2,
        "storeName": "Retired Ship",
        "className": null,
        "shipGuid": null,
        "retired": true
    }])
}

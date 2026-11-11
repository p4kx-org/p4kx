// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, ValueError};
use serde_json::{Value, json};

pub(super) struct ErrorCase {
    pub(super) error: ValueError,
    pub(super) kind: &'static str,
    pub(super) parameters: Value,
    pub(super) controls: Value,
}

pub(super) fn cases() -> Vec<ErrorCase> {
    let mut cases = scalar_cases();
    cases.extend(pointer_cases());
    cases.extend(array_policy_cases());
    cases.extend(validation_cases());
    cases
}

fn scalar_cases() -> Vec<ErrorCase> {
    let guid = Guid::from_stored([0x17; 16]);
    vec![
        ErrorCase {
            error: ValueError::InvalidString {
                table: "unfamiliar-table",
                offset: u32::MAX,
            },
            kind: "invalidString",
            parameters: json!({"offset":4_294_967_295_u32}),
            controls: json!({"table":"unfamiliar-table"}),
        },
        ErrorCase {
            error: ValueError::UnknownEnum { index: u16::MAX },
            kind: "unknownEnum",
            parameters: json!({"index":65535}),
            controls: json!({}),
        },
        ErrorCase {
            error: ValueError::UnknownStruct { index: 37 },
            kind: "unknownStruct",
            parameters: json!({"index":37}),
            controls: json!({}),
        },
        ErrorCase {
            error: ValueError::InvalidBool { byte: 0x81 },
            kind: "invalidBool",
            parameters: json!({"byte":129}),
            controls: json!({}),
        },
        ErrorCase {
            error: ValueError::DanglingReference { guid },
            kind: "danglingReference",
            parameters: json!({"guid":"17171717-1717-1717-1717-171717171717"}),
            controls: json!({}),
        },
    ]
}

fn pointer_cases() -> Vec<ErrorCase> {
    vec![
        ErrorCase {
            error: ValueError::InvalidPointer {
                struct_index: 77,
                instance_index: 99,
            },
            kind: "invalidPointer",
            parameters: json!({"structIndex":77,"instanceIndex":99}),
            controls: json!({}),
        },
        ErrorCase {
            error: ValueError::OwnedTwice {
                struct_index: 23,
                instance_index: 29,
            },
            kind: "ownedTwice",
            parameters: json!({"structIndex":23,"instanceIndex":29}),
            controls: json!({}),
        },
        ErrorCase {
            error: ValueError::OutsideRecord {
                struct_index: 41,
                instance_index: 43,
            },
            kind: "outsideRecord",
            parameters: json!({"structIndex":41,"instanceIndex":43}),
            controls: json!({}),
        },
    ]
}

fn array_policy_cases() -> Vec<ErrorCase> {
    vec![
        ErrorCase {
            error: ValueError::ArrayOutOfRange {
                first: 13,
                count: 17,
                len: u64::MAX,
                pool: "unfamiliar-pool",
            },
            kind: "arrayOutOfRange",
            parameters: json!({"first":13,"count":17,"len":"18446744073709551615"}),
            controls: json!({"pool":"unfamiliar-pool"}),
        },
        ErrorCase {
            error: ValueError::InlineClassCycle { struct_index: 31 },
            kind: "inlineClassCycle",
            parameters: json!({"structIndex":31}),
            controls: json!({}),
        },
        ErrorCase {
            error: ValueError::PathTooWide { limit: 73 },
            kind: "pathTooWide",
            parameters: json!({}),
            controls: json!({"limit":"73"}),
        },
    ]
}

fn validation_cases() -> Vec<ErrorCase> {
    let guid = Guid::from_stored([0x17; 16]);
    vec![
        ErrorCase {
            error: ValueError::UnexpectedPointerType {
                struct_index: 47,
                declared: 61,
            },
            kind: "unexpectedPointerType",
            parameters: json!({"structIndex":47,"declared":61}),
            controls: json!({}),
        },
        ErrorCase {
            error: ValueError::UnexpectedReferenceType {
                guid,
                struct_index: 67,
                declared: 71,
            },
            kind: "unexpectedReferenceType",
            parameters: json!({"guid":"17171717-1717-1717-1717-171717171717","structIndex":67,"declared":71}),
            controls: json!({}),
        },
        ErrorCase {
            error: ValueError::InstanceTooShort {
                struct_index: 79,
                size: 4_294_967_296,
                needed: u64::MAX,
            },
            kind: "instanceTooShort",
            parameters: json!({"structIndex":79,"size":"4294967296","needed":"18446744073709551615"}),
            controls: json!({}),
        },
    ]
}

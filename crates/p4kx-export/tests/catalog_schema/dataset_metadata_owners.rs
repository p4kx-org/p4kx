// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{Group, Owner, Shape};

pub(super) const GROUPS: &[Group] = &[
    Group {
        owner: Owner::DocumentMetadata,
        prefix: "",
        shape: Shape::Object,
        members: &["completeness", "exportHealth", "provenance"],
    },
    Group {
        owner: Owner::DocumentMetadata,
        prefix: "",
        shape: Shape::Scalar,
        members: &["schemaVersion"],
    },
    Group {
        owner: Owner::DocumentMetadata,
        prefix: "/source",
        shape: Shape::Scalar,
        members: &[
            "archiveKey",
            "archiveModified",
            "archiveSize",
            "channel",
            "encryptedEntriesUnread",
            "exportedAt",
            "toolVersion",
        ],
    },
    Group {
        owner: Owner::DocumentMetadata,
        prefix: "/source/build",
        shape: Shape::Scalar,
        members: &[
            "branch",
            "buildId",
            "changeNumber",
            "publicPatch",
            "version",
        ],
    },
];

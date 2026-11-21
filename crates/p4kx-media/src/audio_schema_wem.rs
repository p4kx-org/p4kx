// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::audio_schema_status::StatusRule;
use crate::audio_wem_annotation::FAILURE_NAMESPACE as ANNOTATION_FAILURE;
use crate::audio_wem_cue::{CUE_NAMESPACE, POINT_NAMESPACE, TAIL_NAMESPACE};
use crate::audio_wem_diagnostic::{FAILURE_NAMESPACE as WEM_FAILURE, OPAQUE_NAMESPACE};
use crate::audio_wem_list::{ITEM_NAMESPACE, LIST_NAMESPACE};
use crate::audio_wem_projection::{CHUNK_NAMESPACE, CONVERSION_NAMESPACE, WEM_NAMESPACE};

use super::{ANNOTATION_PAYLOAD, Binding, WEM_PAYLOAD};

pub(super) const fn bindings() -> [Binding; 5] {
    [
        Binding::exact(WEM_PAYLOAD, WEM_NAMESPACE, "wem", 0, StatusRule::Complete),
        Binding::exact(
            WEM_PAYLOAD,
            CONVERSION_NAMESPACE,
            "wem-conversion",
            0,
            StatusRule::Conversion,
        ),
        Binding::exact(
            WEM_PAYLOAD,
            CHUNK_NAMESPACE,
            "wem-chunk",
            1,
            StatusRule::Complete,
        ),
        Binding::exact(
            WEM_PAYLOAD,
            OPAQUE_NAMESPACE,
            "wem-opaque",
            0,
            StatusRule::Complete,
        ),
        Binding::exact(
            WEM_PAYLOAD,
            WEM_FAILURE,
            "wem-failure",
            1,
            StatusRule::Failed,
        ),
    ]
}

pub(super) const fn annotation_bindings() -> [Binding; 6] {
    [
        Binding::exact(
            ANNOTATION_PAYLOAD,
            CUE_NAMESPACE,
            "wem-cue",
            1,
            StatusRule::Complete,
        ),
        Binding::exact(
            ANNOTATION_PAYLOAD,
            POINT_NAMESPACE,
            "wem-cue-point",
            2,
            StatusRule::Complete,
        ),
        Binding::exact(
            ANNOTATION_PAYLOAD,
            TAIL_NAMESPACE,
            "wem-cue-tail",
            1,
            StatusRule::Complete,
        ),
        Binding::exact(
            ANNOTATION_PAYLOAD,
            LIST_NAMESPACE,
            "wem-list",
            1,
            StatusRule::Complete,
        ),
        Binding::exact(
            ANNOTATION_PAYLOAD,
            ITEM_NAMESPACE,
            "wem-list-item",
            2,
            StatusRule::ShortListItem,
        ),
        Binding::exact(
            ANNOTATION_PAYLOAD,
            ANNOTATION_FAILURE,
            "wem-annotation-failure",
            1,
            StatusRule::Failed,
        ),
    ]
}

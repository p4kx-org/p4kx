// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::audio_schema_status::StatusRule;
use crate::audio_wem_diagnostic::{FAILURE_NAMESPACE as WEM_FAILURE, OPAQUE_NAMESPACE};
use crate::audio_wem_projection::{CHUNK_NAMESPACE, CONVERSION_NAMESPACE, WEM_NAMESPACE};

use super::{Binding, WEM_PAYLOAD};

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

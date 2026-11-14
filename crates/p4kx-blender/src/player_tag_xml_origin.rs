// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    SourceScope, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue,
};

use crate::soc_scene::SocSceneError;
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};

pub(in super::super) fn node(
    origin: &XmlOrigin<'_>,
    position: SocXmlPosition,
) -> Result<(SourcedValue<SocXmlPosition>, SourceUse), SocSceneError> {
    let used = origin.at(&position, "$element", SourceUseRole::Payload)?;
    let value = origin.sources.value(
        SourceValue::Present(position),
        std::slice::from_ref(&used),
        true,
    );
    Ok((value, used))
}

pub(in super::super) fn origin<'a>(sources: &'a SourceUses, scope: &SourceScope) -> XmlOrigin<'a> {
    XmlOrigin {
        sources,
        scope: scope.clone(),
        chunk: None,
        archive_member: None,
    }
}

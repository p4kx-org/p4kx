// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::archive_animation_path;
use p4kx_extraction::cryxml::SourceElement;

pub(super) fn targets(root: SourceElement<'_>) -> Vec<String> {
    if !is(root, "Params") {
        return Vec::new();
    }
    root.elements()
        .filter(|element| is(*element, "AnimationList"))
        .flat_map(SourceElement::elements)
        .filter(|element| is(*element, "Animation"))
        .filter_map(|element| {
            let name = unique_attribute(element, "name")?;
            let path = unique_attribute(element, "path")?;
            if !name.eq_ignore_ascii_case("$AnimEventDatabase")
                || !path
                    .rsplit_once('.')
                    .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("animevents"))
            {
                return None;
            }
            archive_animation_path(path)
        })
        .collect()
}

fn unique_attribute<'source>(
    element: SourceElement<'source>,
    wanted: &str,
) -> Option<&'source str> {
    let mut attributes = element.attributes().filter(|attribute| {
        attribute.namespace().is_none() && attribute.name().eq_ignore_ascii_case(wanted)
    });
    let attribute = attributes.next()?;
    attributes.next().is_none().then(|| attribute.value())
}

fn is(element: SourceElement<'_>, name: &str) -> bool {
    element.namespace().is_none() && element.name().eq_ignore_ascii_case(name)
}

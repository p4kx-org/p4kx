// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::SourceElement;
use serde_json::{Value, json};

use crate::real_archive_item_durability::{rows, text};

use super::TestResult;
use super::proof::XmlProof;
use super::scopes;

#[derive(Debug, Default)]
pub(super) struct Coverage {
    pub(super) branches: usize,
    pub(super) selectors: usize,
    pub(super) no_group: usize,
    pub(super) unresolved_scope: usize,
    pub(super) matched_scope: usize,
    pub(super) options: usize,
    pub(super) blend_out: usize,
    pub(super) blends: usize,
    pub(super) animations: usize,
    pub(super) speed: usize,
    pub(super) flags: usize,
    pub(super) proc_layers: usize,
    pub(super) candidates: usize,
}

pub(super) struct OptionRow<'source, 'catalog> {
    pub(super) group: &'source str,
    pub(super) original: SourceElement<'source>,
    pub(super) row: &'catalog Value,
}

pub(super) fn checked<'source, 'catalog>(
    root: SourceElement<'source>,
    catalog: &'catalog Value,
    proof: XmlProof<'_, '_>,
    selectors: &[&str],
    counts: &mut Coverage,
) -> TestResult<Vec<OptionRow<'source, 'catalog>>> {
    let mut rows = rows(catalog, "/options")?.iter();
    let mut selected = Vec::new();
    for (section_index, section) in root.elements().enumerate() {
        if section.name() != "FragmentList" || section.namespace().is_some() {
            continue;
        }
        for (group_index, group) in section.elements().enumerate() {
            if group.namespace().is_some() {
                continue;
            }
            let wanted = selectors
                .iter()
                .any(|name| group.name().eq_ignore_ascii_case(name));
            for (option_index, option) in group.elements().enumerate() {
                if option.name() != "Fragment" || option.namespace().is_some() {
                    continue;
                }
                let row = rows.next().ok_or("Catalog option is missing")?;
                if wanted {
                    let ordinals = [section_index, group_index, option_index];
                    check_option(proof, row, group, option, ordinals, counts)?;
                    selected.push(OptionRow {
                        group: group.name(),
                        original: option,
                        row,
                    });
                }
            }
        }
    }
    assert!(
        rows.next().is_none(),
        "every option row belongs to an original occurrence"
    );
    Ok(selected)
}

fn check_option(
    proof: XmlProof<'_, '_>,
    row: &Value,
    group: SourceElement<'_>,
    option: SourceElement<'_>,
    ordinals: [usize; 3],
    counts: &mut Coverage,
) -> TestResult {
    assert!(
        row.pointer("/ordinals/value") == Some(&json!(ordinals)),
        "original list, group and option ordinals",
    );
    proof.position(&row["position"], option)?;
    proof.name(&row["fragment"], group)?;
    proof.attributes(row, option)?;
    counts.options += 1;
    counts.blend_out += usize::from(scopes::source_attribute(option, "BlendOutDuration").is_some());
    check_elements(proof, row, option, counts)
}

fn check_elements(
    proof: XmlProof<'_, '_>,
    row: &Value,
    owner: SourceElement<'_>,
    counts: &mut Coverage,
) -> TestResult {
    let mut pending = vec![(None, owner.elements().enumerate())];
    for (index, row) in rows(row, "/elements")?.iter().enumerate() {
        let (parent, ordinal, node) = loop {
            let Some((parent, siblings)) = pending.last_mut() else {
                return Err("extra XML child in option row".into());
            };
            if let Some((ordinal, node)) = siblings.next() {
                break (*parent, ordinal, node);
            }
            pending.pop();
        };
        assert!(
            row.pointer("/ordinal/value") == Some(&json!(ordinal))
                && row.pointer("/parent/value") == Some(&json!(parent)),
            "exact option child and parent positions",
        );
        proof.position(&row["position"], node)?;
        proof.name(&row["name"], node)?;
        proof.attributes(row, node)?;
        match text(row, "/name/value")? {
            "Blend" => counts.blends += 1,
            "Animation" => {
                counts.animations += 1;
                counts.speed += usize::from(scopes::source_attribute(node, "speed").is_some());
                counts.flags += usize::from(scopes::source_attribute(node, "flags").is_some());
            }
            "ProcLayer" => counts.proc_layers += 1,
            _ => {}
        }
        pending.push((Some(index), node.elements().enumerate()));
    }
    assert!(
        pending
            .iter_mut()
            .all(|(_, siblings)| siblings.next().is_none()),
        "every original child and lexical leaf is retained",
    );
    Ok(())
}

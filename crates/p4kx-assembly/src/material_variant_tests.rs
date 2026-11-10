// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::thread;

use crate::MaterialVariant;

fn node(tags: &str, material: Option<&str>, children: Vec<MaterialVariant>) -> MaterialVariant {
    MaterialVariant {
        tags: tags.split_whitespace().map(str::to_owned).collect(),
        material: material.map(str::to_owned),
        children,
    }
}

#[test]
fn resolution_preserves_branch_order_tag_gating_and_parent_fallback() {
    let variants = vec![
        node(
            "a b",
            Some("first"),
            vec![
                node(
                    "",
                    Some("tagless"),
                    vec![node("a", Some("hidden"), Vec::new())],
                ),
                node("other", Some("excluded"), Vec::new()),
                node("b", None, vec![node("a", Some("deep"), Vec::new())]),
            ],
        ),
        node("a", Some("later"), Vec::new()),
    ];
    assert_eq!(
        MaterialVariant::resolve(&variants, &["A".to_owned(), "B".to_owned()]),
        Some("deep")
    );
    assert_eq!(
        MaterialVariant::resolve(&variants, &["a".to_owned()]),
        Some("later")
    );
    assert_eq!(MaterialVariant::resolve(&variants, &[]), None);
    let fallback = vec![node("a", Some("parent"), vec![node("b", None, Vec::new())])];
    assert_eq!(
        MaterialVariant::resolve(&fallback, &["a".to_owned(), "b".to_owned()]),
        Some("parent")
    );
}

#[test]
fn a_matching_materialless_branch_does_not_hide_a_later_material() {
    let variants = vec![
        node("a", None, Vec::new()),
        node("a", Some("later"), Vec::new()),
    ];
    assert_eq!(
        MaterialVariant::resolve(&variants, &["a".to_owned()]),
        Some("later")
    );
}

#[test]
fn complete_material_tree_operations_do_not_recurse_on_the_thread_stack()
-> Result<(), Box<dyn Error>> {
    thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            let levels = 8192;
            let mut original = node("match", Some("tail"), Vec::new());
            for _ in 0..levels {
                original = node("match", None, vec![original]);
            }
            assert_eq!(
                MaterialVariant::resolve(std::slice::from_ref(&original), &["MATCH".to_owned()]),
                Some("tail")
            );
            let mut clone = original.clone();
            assert_eq!(original, clone);
            let text = format!("{original:#?}");
            assert_eq!(text.matches("MaterialNode {").count(), levels + 1);
            assert!(text.contains("\"tail\""));
            let mut leaf = &mut clone;
            while !leaf.children.is_empty() {
                leaf = &mut leaf.children[0];
            }
            leaf.material = Some("changed".to_owned());
            assert_ne!(original, clone);
            drop(clone);
            drop(original);
        })?
        .join()
        .map_err(|_| "material tree operations panicked")?;
    Ok(())
}

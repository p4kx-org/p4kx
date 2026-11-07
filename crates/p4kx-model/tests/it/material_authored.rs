// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_model::{
    AuthoredAttributeKind, AuthoredContent, MaterialEntryDisposition, MaterialLibrary, RawElement,
};
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement};

type TestResult = Result<(), Box<dyn Error>>;

fn child<'element>(
    element: &'element RawElement,
    name: &str,
) -> Result<&'element RawElement, Box<dyn Error>> {
    element
        .children
        .iter()
        .find(|child| child.name == name)
        .ok_or_else(|| "authored child missing".into())
}

#[test]
fn multi_root_and_unknown_slots_survive_without_inheriting_root_parameters() -> TestResult {
    let bytes = br#"<Material MtlFlags="-1" RootOnly="keep"><PublicParams Gloss="001.50"/><SubMaterials><Material Name="Hull"/><UnknownSlot State="retained"/></SubMaterials><VertexDeform Type="wave"><Unknown Value="NaN"/></VertexDeform></Material>"#;
    let library = MaterialLibrary::parse("synthetic/root", bytes)?;
    let root = library.authored.root.as_ref().ok_or("authored root")?;
    assert_eq!(
        root.attributes,
        [
            ("MtlFlags".into(), "-1".into()),
            ("RootOnly".into(), "keep".into())
        ],
        "root attributes remain separate"
    );
    assert_eq!(
        child(root, "PublicParams")?.attributes,
        [("Gloss".into(), "001.50".into())],
        "authored numeric lexeme stays text"
    );
    assert_eq!(
        library.materials.len(),
        2,
        "unknown slot retains its original material index"
    );
    assert_eq!(
        library.materials.first().ok_or("surface")?.flags.0,
        0,
        "root flags are not inherited into children"
    );
    assert_eq!(
        library.unknown_entries, 1,
        "unknown submaterial disposition is counted"
    );
    let unknown = library.authored.entries.get(1).ok_or("unknown position")?;
    assert_eq!(unknown.ordinal, 1, "original ordinal is not compacted");
    assert_eq!(
        unknown.disposition,
        MaterialEntryDisposition::EngineDefault,
        "existing render fallback is explicit"
    );
    assert_eq!(
        child(child(root, "SubMaterials")?, "UnknownSlot")?
            .attributes
            .first()
            .map(|pair| pair.1.as_str()),
        Some("retained"),
        "unknown slot source is preserved"
    );
    Ok(())
}

#[test]
fn qualified_names_namespace_declarations_and_case_distinct_attributes_keep_order() -> TestResult {
    let bytes = br#"<m:Material Case="one" xmlns:m="urn:material" m:Case="A&amp;B" case="two" xmlns:n="urn:other"><n:Unknown n:Flag="keep"/></m:Material>"#;
    let library = MaterialLibrary::parse("synthetic/namespaces", bytes)?;
    let root = library.authored.root.as_ref().ok_or("root")?;
    let names: Vec<_> = root
        .authored
        .attributes
        .iter()
        .map(|attribute| attribute.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["Case", "xmlns:m", "m:Case", "case", "xmlns:n"],
        "namespace declarations share original attribute order"
    );
    assert_eq!(
        root.authored.qualified_name, "m:Material",
        "original prefix survives"
    );
    assert_eq!(
        root.authored.namespace.as_deref(),
        Some("urn:material"),
        "namespace meaning comes from the parser"
    );
    let attribute = root
        .authored
        .attributes
        .get(2)
        .ok_or("qualified attribute")?;
    assert_eq!(
        attribute.value, "A&B",
        "XML semantic value is kept separately"
    );
    assert_eq!(
        attribute.lexeme.as_deref(),
        Some("A&amp;B"),
        "authored entity spelling remains unchanged"
    );
    assert!(
        matches!(&attribute.kind, AuthoredAttributeKind::Attribute { namespace: Some(value) } if value == "urn:material"),
        "qualified attribute namespace is explicit"
    );
    assert_eq!(
        child(root, "Unknown")?.authored.qualified_name,
        "n:Unknown",
        "unknown QName survives"
    );
    Ok(())
}

#[test]
fn coalesced_text_cdata_comments_and_pi_keep_source_order_and_endtag_like_data() -> TestResult {
    let bytes = br"<Material><Unknown>head<![CDATA[</Unknown><fake/>]]>&amp;<!--comment--><?probe </Unknown> ?><Child/>tail<![CDATA[</Material>]]></Unknown></Material>";
    let library = MaterialLibrary::parse("synthetic/mixed", bytes)?;
    let root = library.authored.root.as_ref().ok_or("root")?;
    let unknown = child(root, "Unknown")?;
    let [
        AuthoredContent::Text { value, lexeme },
        AuthoredContent::Comment { value: comment },
        AuthoredContent::ProcessingInstruction { target, .. },
        AuthoredContent::Element { ordinal },
        AuthoredContent::Text {
            value: tail,
            lexeme: tail_lexeme,
        },
    ] = unknown.authored.content.as_slice()
    else {
        return Err("ordered mixed content shape".into());
    };
    assert_eq!(
        value, "head</Unknown><fake/>&",
        "coalesced text is not truncated to its first chunk"
    );
    assert_eq!(
        lexeme, "head<![CDATA[</Unknown><fake/>]]>&amp;",
        "lexeme extends to the next actual direct node"
    );
    assert_eq!(comment, "comment", "comment remains in place");
    assert_eq!(target, "probe", "processing instruction is retained");
    assert_eq!(
        *ordinal, 0,
        "child reference uses its original element ordinal"
    );
    assert_eq!(
        tail, "tail</Material>",
        "CDATA endtag-like data is ordinary content"
    );
    assert_eq!(
        tail_lexeme, "tail<![CDATA[</Material>]]>",
        "last text ends at the proven actual parent endtag"
    );
    Ok(())
}

#[test]
fn plain_and_binary_keep_root_public_params_layers_and_texmod_declarations() -> TestResult {
    let root = declaration_fixture();
    let inputs = [
        root.to_xml().into_bytes(),
        root.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little),
    ];
    for input in inputs {
        let library = MaterialLibrary::parse("synthetic/declarations", &input)?;
        assert!(
            library.authored.losses.is_empty(),
            "valid encodings do not imply repaired/lost source syntax"
        );
        let surface = library.materials.first().ok_or("surface")?;
        assert_eq!(
            surface.public_params,
            [
                ("Case".into(), "NaN".into()),
                ("case".into(), "Infinity".into())
            ],
            "nonfinite authored parameter tokens remain text"
        );
        assert_eq!(
            surface
                .layers
                .first()
                .and_then(|layer| layer.attribute("Submtl")),
            Some("7"),
            "layer Submtl is not inferred or dropped"
        );
        let modifier = surface
            .textures
            .first()
            .and_then(|slot| slot.children.first())
            .ok_or("modifier")?;
        assert_eq!(
            modifier.attributes.first().map(|pair| pair.1.as_str()),
            Some("2.50"),
            "non-default modifier keeps its original number spelling"
        );
        assert_eq!(
            surface.textures.first().map(|slot| slot.file.as_str()),
            Some("$RenderToTexture"),
            "engine placeholder is source data"
        );
    }
    Ok(())
}

fn declaration_fixture() -> TestElement {
    TestElement::new("Material")
        .attribute("MtlFlags", "-1")
        .child(TestElement::new("PublicParams").attribute("Root", "001.50"))
        .child(
            TestElement::new("SubMaterials").child(
                TestElement::new("Material")
                    .attribute("Name", "Hull")
                    .child(
                        TestElement::new("PublicParams")
                            .attribute("Case", "NaN")
                            .attribute("case", "Infinity"),
                    )
                    .child(
                        TestElement::new("MatLayers").child(
                            TestElement::new("Layer")
                                .attribute("Path", "synthetic/layer.mtl")
                                .attribute("Submtl", "7"),
                        ),
                    )
                    .child(
                        TestElement::new("Textures").child(
                            TestElement::new("Texture")
                                .attribute("Map", "TexSlot1")
                                .attribute("File", "$RenderToTexture")
                                .child(
                                    TestElement::new("TexMod")
                                        .attribute("TileU", "2.50")
                                        .content("modifier text"),
                                ),
                        ),
                    ),
            ),
        )
}

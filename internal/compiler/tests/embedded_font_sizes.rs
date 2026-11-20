// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![cfg(all(feature = "renderer-software", not(target_arch = "wasm32")))]

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::embedded_resources::{BitmapFont, EmbeddedResourcesKind};
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::{CompilerConfiguration, EmbedResourcesKind, compile_syntax_node};

fn compile_fonts(body: &str, scale: f32) -> Vec<BitmapFont> {
    compile_fonts_with_scale(body, Some(scale))
}

fn compile_fonts_with_scale(body: &str, scale: Option<f32>) -> Vec<BitmapFont> {
    let font_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/screenshots/fonts/NotoSans-Regular.ttf");
    let source = format!(
        r#"import "{}";
        export component TestCase inherits Window {{
            default-font-family: "Noto Sans";
            {body}
        }}"#,
        font_path.display()
    );
    let mut diagnostics = BuildDiagnostics::default();
    let node = i_slint_compiler::parser::parse(source, None, &mut diagnostics);
    let mut config = CompilerConfiguration::new(OutputFormat::Llr);
    config.embed_resources = EmbedResourcesKind::EmbedTextures;
    config.const_scale_factor = scale;
    let (doc, diagnostics, _) = spin_on::spin_on(compile_syntax_node(node, diagnostics, config));
    assert!(!diagnostics.has_errors(), "{:?}", diagnostics.to_string_vec());
    let fonts: Vec<_> = doc
        .embedded_file_resources
        .borrow()
        .iter()
        .filter_map(|resource| match &resource.kind {
            EmbeddedResourcesKind::BitmapFontData(font) => Some(font.clone()),
            _ => None,
        })
        .collect();
    assert!(!fonts.is_empty());
    fonts
}

fn sizes(fonts: &[BitmapFont]) -> Vec<i16> {
    let mut result: Vec<_> =
        fonts.iter().flat_map(|font| font.glyphs.iter().map(|g| g.pixel_size)).collect();
    result.sort();
    result.dedup();
    result
}

#[test]
fn fixed_sizes_omit_unused_default_and_preserve_glyphs() {
    let fixed = compile_fonts(
        r#"default-font-size: 14px; Text { text: "Meter 123"; font-size: 14px; }"#,
        1.,
    );
    let dynamic = compile_fonts(
        r#"
        default-font-size: 14px;
        in-out property <length> size: 14px;
        Text { text: "Meter 123"; font-size: root.size; }
    "#,
        1.,
    );
    assert_eq!(sizes(&fixed), [14]);
    assert_eq!(sizes(&dynamic), [12, 14]);
    for font in &fixed {
        let reference = dynamic
            .iter()
            .find(|f| {
                f.family_name == font.family_name
                    && f.weight == font.weight
                    && f.italic == font.italic
            })
            .unwrap();
        let a = &font.glyphs.iter().find(|g| g.pixel_size == 14).unwrap().glyph_data;
        let b = &reference.glyphs.iter().find(|g| g.pixel_size == 14).unwrap().glyph_data;
        assert_eq!(a.len(), b.len());
        assert_eq!(font.character_map.len(), reference.character_map.len());
        for (entry, reference_entry) in font.character_map.iter().zip(&reference.character_map) {
            assert_eq!(entry.code_point, reference_entry.code_point);
            let a = &a[entry.glyph_index as usize];
            let b = &b[reference_entry.glyph_index as usize];
            assert_eq!(
                (a.x, a.y, a.width, a.height, a.x_advance, &a.data),
                (b.x, b.y, b.width, b.height, b.x_advance, &b.data)
            );
        }
    }
}

#[test]
fn mutable_literal_font_size_keeps_default() {
    let fonts = compile_fonts(
        r#"
        default-font-size: 14px;
        t := Text { text: "Meter"; font-size: 14px; }
        callback change-size();
        change-size => { t.font-size = 12px; }
    "#,
        1.,
    );
    assert_eq!(sizes(&fonts), [12, 14]);
}

#[test]
fn unspecified_window_default_keeps_default() {
    assert_eq!(sizes(&compile_fonts(r#"Text { text: "Meter"; font-size: 14px; }"#, 1.)), [12, 14]);
}

#[test]
fn runtime_scale_factor_keeps_default() {
    assert_eq!(
        sizes(&compile_fonts_with_scale(
            r#"default-font-size: 14px; Text { text: "Meter"; font-size: 14px; }"#,
            None,
        )),
        [12, 14]
    );
}

#[test]
fn zero_font_size_keeps_default() {
    assert!(
        sizes(&compile_fonts(
            r#"default-font-size: 14px; Text { text: "Meter"; font-size: 0px; }"#,
            1.
        ))
        .contains(&12)
    );
}

#[test]
fn explicit_default_size_and_scale_are_preserved() {
    assert_eq!(
        sizes(&compile_fonts(
            r#"default-font-size: 14px; Text { text: "Meter"; font-size: 12px; }"#,
            1.
        )),
        [12, 14]
    );
    assert_eq!(
        sizes(&compile_fonts(
            r#"default-font-size: 14px; Text { text: "Meter"; font-size: 14px; }"#,
            2.
        )),
        [28]
    );
}

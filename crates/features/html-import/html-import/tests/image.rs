//! Images: the `url()` the export writes (an asset id, with the size, position and repeat it writes) is the model's
//! `image`; every other `url()` is a placeholder, kept as text under `assets` and never fetched.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "paint_common/mod.rs"]
mod common;
use common::*;
use meldframe_protocol::{ImageFill, ImageFit};

const ID: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

fn img(base: &str, size: &str) -> Read {
    read(&format!(
        "background-image:url({base}{ID});background-size:{size};background-position:center;background-repeat:no-repeat"
    ))
}

fn fit(r: &Read) -> ImageFit {
    r.paint.image.as_ref().expect("an image").fit
}

#[test]
fn the_export_form_is_an_image_with_its_fit() {
    for (size, want) in [
        ("cover", ImageFit::Cover),
        ("contain", ImageFit::Contain),
        ("100% 100%", ImageFit::Fill),
        ("COVER", ImageFit::Cover),
    ] {
        let r = img("assets/", size);
        assert_eq!(
            r.paint.image,
            Some(ImageFill {
                asset_id: ID.into(),
                fit: want
            })
        );
        assert!(
            r.paint.assets.is_empty() && r.report.entries.is_empty(),
            "{size}"
        );
    }
}

#[test]
fn the_base_of_the_url_is_whatever_ends_before_the_id() {
    for base in [
        "assets/",
        "https://cdn.example/x_y/",
        "assets/a\\20 b\\26 c\\27 d\\22 e/",
        "a\\5c b\\3b c\\7b d\\7d /",
        "",
        "'assets/",
    ] {
        let q = if base.starts_with('\'') { "'" } else { "" };
        let r = read(&format!(
            "background-image:url({base}{ID}{q});background-size:cover;background-position:center;background-repeat:no-repeat"
        ));
        assert_eq!(fit(&r), ImageFit::Cover, "{base}");
    }
    // Quotes around the whole url are read too.
    let r = read(&format!(
        "background-image:url('assets/{ID}');background-size:cover;background-position:center;background-repeat:no-repeat"
    ));
    assert_eq!(fit(&r), ImageFit::Cover);
}

#[test]
fn an_image_under_a_gradient_is_both_with_a_size_for_each_layer() {
    let r = read(&format!(
        "background-image:url(assets/{ID}),linear-gradient(90deg,#ff0000 0%,#0000ff 100%);background-size:contain,100% 100%;background-position:center;background-repeat:no-repeat"
    ));
    assert_eq!(fit(&r), ImageFit::Contain);
    assert!(r.paint.gradient.is_some());
    assert!(r.report.entries.is_empty(), "{:?}", r.notes());
}

#[test]
fn a_size_position_or_repeat_that_is_not_the_exports_leaves_the_image_a_placeholder() {
    let base = format!("background-image:url(assets/{ID});");
    for (extra, code) in [
        (
            "background-position:center;background-repeat:no-repeat",
            "background_size",
        ),
        (
            "background-size:50%;background-position:center;background-repeat:no-repeat",
            "background_size",
        ),
        (
            "background-size:auto;background-position:center;background-repeat:no-repeat",
            "background_size",
        ),
        (
            "background-size:cover;background-repeat:no-repeat",
            "background_position",
        ),
        (
            "background-size:cover;background-position:top left;background-repeat:no-repeat",
            "background_position",
        ),
        (
            "background-size:cover;background-position:center",
            "background_repeat",
        ),
        (
            "background-size:cover;background-position:center;background-repeat:repeat",
            "background_repeat",
        ),
    ] {
        let r = read(&format!("{base}{extra}"));
        assert!(r.paint.image.is_none(), "{extra}");
        assert_eq!(r.notes(), [format!("refused:{code}")], "{extra}");
        assert_eq!(r.paint.assets.len(), 1, "{extra}");
        assert!(r.paint.assets[0].src.ends_with(ID));
    }
    for pos in ["center center", "50% 50%"] {
        let r = read(&format!(
            "{base}background-size:cover;background-position:{pos};background-repeat:no-repeat"
        ));
        assert!(r.paint.image.is_some(), "{pos}");
    }
}

#[test]
fn any_other_url_is_a_placeholder_and_is_listed_not_fetched() {
    for (src, code) in [
        ("photo.png", "background_url"),
        ("https://example.com/a.jpg", "background_url"),
        ("javascript:alert(1)", "background_url"),
        (
            "assets/ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789",
            "background_url",
        ),
        (&ID[..63], "background_url"),
        (
            "assets/g23456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef01",
            "background_url",
        ),
        ("logo.svg", "image_svg"),
        ("logo.SVG?x=1#a", "image_svg"),
    ] {
        let r = read(&format!(
            "background-image:url({src});background-size:cover"
        ));
        assert!(r.paint.image.is_none(), "{src}");
        assert_eq!(r.notes(), [format!("refused:{code}")], "{src}");
        assert_eq!(r.paint.assets.len(), 1, "{src}");
        assert_eq!(r.paint.assets[0].src, src);
    }
}

#[test]
fn a_data_url_keeps_its_media_type_and_never_its_payload() {
    let r = read("background-image:url(data:image/png;base64,AAAAAAAAAAAAAAAAAAAA)");
    assert!(r.paint.image.is_none());
    assert_eq!(r.notes(), ["refused:image_data_uri"]);
    assert_eq!(r.paint.assets[0].src, "data:image/png;base64");
    let r = read("background-image:url('DATA:image/svg+xml;utf8,<svg/>')");
    assert_eq!(r.paint.assets[0].src, "DATA:image/svg+xml;utf8");
}

#[test]
fn a_long_src_is_cut_and_cleaned() {
    let long = "a".repeat(400);
    let r = read(&format!("background-image:url({long}.png)"));
    assert_eq!(r.paint.assets[0].src.len(), 200);
    let r = read("background-image:url('a\u{202e}b\u{7}c.png')");
    assert_eq!(r.paint.assets[0].src, "abc.png");
}

#[test]
fn a_src_is_an_identifier_to_the_strict_profile_and_is_cut_between_characters() {
    // The old set kept these (U+2028, U+061C, U+2800, U+3164); a joiner has no emoji to protect in an address.
    let r =
        read("background-image:url('a\u{2028}b\u{61c}c\u{2800}d\u{3164}e\u{200d}f\u{fe0f}.png')");
    assert_eq!(r.paint.assets[0].src, "abcdef.png");
    let r = read("background-image:url('\u{1F468}\u{200D}\u{1F469}.png')");
    assert_eq!(r.paint.assets[0].src, "\u{1F468}\u{1F469}.png");
    // Four-byte characters at the cap: 200 characters, none cut in the middle.
    let r = read(&format!(
        "background-image:url('{}')",
        "\u{1F600}".repeat(220)
    ));
    let src = &r.paint.assets[0].src;
    assert_eq!(src.chars().count(), 200);
    assert_eq!(src.len(), 800);
    // A `data:` URL keeps its media type and nothing hidden in it.
    let r = read("background-image:url('data:image/\u{202e}png;base64,AAAA')");
    assert_eq!(r.paint.assets[0].src, "data:image/png;base64");
}

#[test]
fn a_second_image_is_refused_and_a_gradient_over_an_image_is_listed() {
    let r = read(&format!(
        "background-image:url(assets/{ID}),url(other.png);background-size:cover;background-position:center;background-repeat:no-repeat"
    ));
    assert!(r.paint.image.is_some());
    assert_eq!(r.notes(), ["refused:background_url"]);
    let r = read(&format!(
        "background-image:linear-gradient(red,blue),url(assets/{ID});background-size:100% 100%,cover;background-position:center;background-repeat:no-repeat"
    ));
    assert!(r.paint.image.is_some() && r.paint.gradient.is_some());
    assert_eq!(r.notes(), ["listed:background_layer_order"]);
    let id2 = ID.replace('a', "b");
    let r = read(&format!(
        "background-image:url(assets/{ID}),url(assets/{id2});background-size:cover;background-position:center;background-repeat:no-repeat"
    ));
    assert_eq!(r.paint.image.as_ref().unwrap().asset_id, ID);
    assert_eq!(r.notes(), ["refused:background_layer_extra"]);
    assert_eq!(r.paint.assets.len(), 1);
}

#[test]
fn an_empty_url_is_refused() {
    for url in ["url()", "url('')", "url( )"] {
        let r = read(&format!("background-image:{url}"));
        assert!(
            r.paint.image.is_none() && r.paint.assets.is_empty(),
            "{url}"
        );
        assert_eq!(r.notes(), ["refused:background_url"], "{url}");
    }
}

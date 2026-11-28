//! The capture request path through the harness (finding T13, decision 43): the app layer plans the drawing an agent asked for, the
//! shell hands back the size of the bytes it holds as `Action::Wire`, and an image over the 4 MiB cap is refused as data
//! (`Effect::FailCapture` and a drop of the bytes), never uploaded. The rule is the app's (`meldframe-app-export-tests`,
//! `capture_request.rs`); this test is that the shell of a second front end meets it with nothing but actions and answers.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::files;
use meldframe_headless::run;

const CAPTURE: &str = include_str!("scripts/capture.script");

#[test]
fn an_image_at_the_cap_is_uploaded_and_one_byte_over_is_refused_as_data_and_never_delivered() {
    let r = run(CAPTURE, &files, true);
    assert_eq!(r.failure, None, "{}", r.out);
    // Data for the agent, not a refusal: no `Event::Refused` came out of either request.
    assert!(r.refusals.is_empty(), "{:?}", r.refusals);
    // The first request was drawn and delivered; the second was drawn, dropped and answered with the failure.
    assert!(
        r.out.contains("  effects: raster finishRaster\n"),
        "{}",
        r.out
    );
    assert!(
        r.out
            .contains("  effects: raster finishRaster failCapture\n"),
        "{}",
        r.out
    );
    assert_eq!(
        r.shell.expect("a file is open").delivered(),
        Some((300, 200))
    );
}

#[test]
fn an_image_over_the_cap_alone_is_never_delivered() {
    let over = CAPTURE.replace("bytes 4194304", "bytes 4194305");
    let r = run(&over, &files, false);
    // The first request is over the cap too, so the check that it was delivered fails, and nothing was ever delivered.
    assert!(
        r.failure.as_deref().is_some_and(|f| f.contains("deliver")),
        "{}",
        r.out
    );
    assert_eq!(r.shell.expect("a file is open").delivered(), None);
}

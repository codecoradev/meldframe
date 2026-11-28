//! The desktop shell on Rune's headless runtime: no window, no GPU, no socket. It is what C4 and C6 can run here. A frame of this
//! runtime does the CPU part of a frame (the view, the layout of the elements, the instances) and presents nothing, so a time
//! read from it is never the frame interval of a real window.

use meldframe_desktop::{State, WINDOW_SIZE, spec};
use meldframe_protocol::Node;
use rune::{Env, Runtime};
use std::cell::RefCell;

thread_local! {
    static FILE: RefCell<Vec<Node>> = const { RefCell::new(Vec::new()) };
}

fn init() -> State {
    FILE.with(|f| State::with_nodes(f.borrow().clone(), true))
}

/// The nodes `State::with_nodes` is given when a window of this thread is made (an app spec takes a plain function).
pub fn set_file(nodes: Vec<Node>) {
    FILE.with(|f| *f.borrow_mut() = nodes);
}

/// The shell on `nodes` in a headless runtime at scale 1, the first frame already run, with the bundled font only.
pub fn runtime(nodes: Vec<Node>) -> Runtime<State> {
    rune::text::set_system_fonts(false);
    set_file(nodes);
    let mut rt = rune::App::new(spec(init)).headless_runtime(WINDOW_SIZE, Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

/// Renders the shell on `nodes` to the PNG `out` with Rune's own headless renderer, on the adapter wgpu finds (the one
/// `VK_ICD_FILENAMES` names, for a software run), and returns the milliseconds it took, adapter start-up included.
pub fn snap(nodes: Vec<Node>, out: &std::path::Path) -> Result<f64, String> {
    rune::text::set_system_fonts(false);
    set_file(nodes);
    let (w, h) = WINDOW_SIZE;
    let argv = [
        "--snap".to_owned(),
        out.display().to_string(),
        "--size".to_owned(),
        format!("{}x{}", w as u32, h as u32),
        "--scale".to_owned(),
        "1".to_owned(),
    ];
    let t = std::time::Instant::now();
    rune::App::new(spec(init))
        .args(argv)
        .try_run()
        .map_err(|e| format!("{e:?}"))?;
    Ok(t.elapsed().as_secs_f64() * 1e3)
}

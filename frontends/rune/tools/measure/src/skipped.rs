//! The criteria this slice does not run, said plainly with what each one needs. None prints a number.

use crate::report::{Report, skipped};

/// C3 Paint: the oracle's probes against the painter's pixels.
pub fn c3() -> Report {
    skipped(
        "C3",
        "the painter draws no gradient, shadow or clip yet, and the oracle's probe points are in tools/fidelity-oracle/probes.mjs and not exported as data",
        &[
            "a painter for gradients, blurred shadows and a clip (E8.3.2)",
            "the probes of f07-decor and f19-gradient and the blur table written as JSON by the Linux oracle",
            "a readback of the headless render (the PNG of --snap) compared at those points",
        ],
    )
}

/// C7 Input method: a person on a Mac.
pub fn c7() -> Report {
    skipped(
        "C7",
        "needs macOS input methods (Japanese, Chinese, Korean, the emoji picker, dead keys) and a person to judge the candidate window",
        &[
            "a text field wired to the app layer's text edit (state.textEdit, T12) in the desktop shell; the skeleton has none",
            "a development server with a second credential for the node deleted by someone else (check K4)",
            "a person on a Mac with the five input sources of section 10",
        ],
    )
}

/// C8 Accessibility: the tree and VoiceOver.
pub fn c8() -> Report {
    skipped(
        "C8",
        "the skeleton has no toolbar, panels or node list to name, and VoiceOver is a person on a Mac",
        &[
            "the Layers and Properties panels and a node list in the desktop shell (E8.3.3)",
            "a dump of the AccessKit tree of the running window (the tree Rune gives accesskit_macos)",
            "VoiceOver on macOS and a person to copy its caption text",
        ],
    )
}

/// C9 Upstream defect: the first wheel after idle in a native window.
pub fn c9() -> Report {
    skipped(
        "C9",
        "needs a native window with a real event loop and wheel events, which a headless runtime does not have",
        &[
            "the desktop window on the 5,000 node file, in front and focused",
            "wheel events sent into the window's event loop after 5 s, 30 s and 60 s of rest, and a frame monitor that sees the presented frames",
        ],
    )
}

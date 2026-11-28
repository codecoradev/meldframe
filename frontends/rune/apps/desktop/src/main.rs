//! The entry point of the desktop app. `rune::run` opens the window, or, with Rune's tooling flags (`--snap`, `--tree`),
//! renders headless.
//!
//! Not verified: no macOS window was opened and no server was connected (this was written and tested on Linux with no
//! display). The app starts on an empty file and says it is not connected, because the socket and the token handling are not
//! built yet (E8.2.2, what remains).

fn main() {
    rune::run(meldframe_desktop::spec(meldframe_desktop::State::offline));
}

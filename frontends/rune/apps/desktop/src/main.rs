//! The entry point of the desktop app. `rune::run` opens the window, or, with Rune's tooling flags (`--snap`, `--tree`),
//! renders headless.
//!
//! It connects to the development server of `just dev` (`MELDFRAME_SERVER`, default `http://127.0.0.1:8080`) as the token in
//! `MELDFRAME_TOKEN` or in the git-ignored `.dev-token` says; the token is never an argument. Without one it opens on an empty
//! file and says why it is not connected.
//!
//! Not verified: no macOS window was opened (this was written and tested on Linux with no display).

fn main() {
    rune::run(meldframe_desktop::spec(
        meldframe_desktop::State::from_environment,
    ));
}

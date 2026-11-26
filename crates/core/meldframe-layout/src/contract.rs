//! The text the fidelity contract does not promise (`docs/fidelity-contract.md`, section 4.3): the one rule that the
//! export and the HTML import both warn with, here because both depend on the layout core and neither on the other.

/// What `text_outside_contract` says to a person: the warning the export and the HTML import give for such a text.
pub const TEXT_OUTSIDE_CONTRACT_MESSAGE: &str = "the text has a tab, a URL or a script (CJK) that the browser breaks differently from the canvas";

/// A tab, a URL or CJK text: a browser and the canvas break it in different places, so a box that holds it is only as
/// exact as the contract says (`text_outside_contract`).
pub fn text_outside_contract(t: &str) -> bool {
    t.contains('\t')
        || t.contains("://")
        || t.chars().any(|c| {
            ('\u{3000}'..='\u{9fff}').contains(&c) || ('\u{ac00}'..='\u{d7af}').contains(&c)
        })
}

#[cfg(test)]
mod tests {
    use super::text_outside_contract;

    #[test]
    fn a_tab_a_url_and_cjk_text_are_outside_the_contract_and_plain_text_is_not() {
        for t in [
            "a\tb",
            "see https://x.test/a",
            "\u{4f60}\u{597d}",
            "\u{d55c}\u{ae00}",
        ] {
            assert!(text_outside_contract(t), "{t:?}");
        }
        for t in ["", "Hello world", "100% / 2", "caf\u{e9}"] {
            assert!(!text_outside_contract(t), "{t:?}");
        }
    }
}

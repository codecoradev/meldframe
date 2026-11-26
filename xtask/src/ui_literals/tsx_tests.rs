//! The `.tsx` side of `ui-literals`: JSX style objects, Tailwind arbitrary values in class strings and JSX comments.

use super::Literal::*;
use super::*;

fn lines(path: &str, src: &str) -> Vec<usize> {
    check_file(path, src).iter().map(|f| f.line).collect()
}

fn kinds(path: &str, src: &str) -> Vec<Literal> {
    check_file(path, src)
        .into_iter()
        .map(|f| f.literal)
        .collect()
}
fn tsx(src: &str) -> Vec<Literal> {
    kinds("a.tsx", src)
}

#[test]
fn tsx_style_objects_colours_font_sizes_and_durations() {
    assert_eq!(tsx("<p style={{ color: '#fff' }}>x</p>"), [Colour]);
    assert_eq!(
        tsx("<p style={{ background: \"rgb(0 0 0)\" }}>x</p>"),
        [Colour]
    );
    assert_eq!(tsx("<p style={{ fontSize: 13 }}>x</p>"), [FontSize]);
    assert_eq!(tsx("<p style={{ fontSize: '13px' }}>x</p>"), [FontSize]);
    assert_eq!(
        tsx("<p style={{ transitionDuration: 200 }}>x</p>"),
        [Duration]
    );
    assert_eq!(tsx("<p style={{ animationDelay: .5 }}>x</p>"), [Duration]);
    assert_eq!(
        tsx("<p style={{ transition: 'opacity 200ms' }}>x</p>"),
        [Duration]
    );
}

#[test]
fn tsx_style_objects_that_read_tokens_pass() {
    assert!(tsx("<p style={{ color: 'var(--mf-text)', fontSize: size }}>x</p>").is_empty());
    assert!(tsx("<p style={{ transitionDuration: tokens.fast, opacity: 0.5 }}>x</p>").is_empty());
    assert!(tsx("<p style={{ width: 240 }}>x</p>").is_empty()); // sizes are the strict script's
}

#[test]
fn tailwind_arbitrary_values_in_class_strings() {
    assert_eq!(tsx("<p className=\"text-[13px]\">x</p>"), [FontSize]);
    assert_eq!(
        tsx("<p className=\"md:text-[0.8rem] p-2\">x</p>"),
        [FontSize]
    );
    assert_eq!(
        tsx("<p className={`text-[length:13px] ${on}`}>x</p>"),
        [FontSize]
    );
    assert_eq!(tsx("<p className=\"bg-[#fff]\">x</p>"), [Colour]);
    assert_eq!(tsx("<p className=\"bg-[rgb(0,0,0)]\">x</p>"), [Colour]);
    assert_eq!(tsx("<p className=\"duration-[200ms]\">x</p>"), [Duration]);
    assert_eq!(
        tsx("<p className=\"transition-[opacity_200ms]\">x</p>"),
        [Duration]
    );
    assert_eq!(
        kinds("a.svelte", "<p class=\"text-[13px]\">x</p>"),
        [FontSize]
    );
}

#[test]
fn tailwind_classes_that_read_tokens_pass() {
    assert!(
        tsx("<p className=\"text-[var(--mf-font-size-md)] text-text bg-panel w-[var(--w)]\">x</p>")
            .is_empty()
    );
    assert_eq!(tsx("<p className=\"text-[#abc]\">x</p>"), [Colour]);
    assert!(tsx("<p className=\"context-menu delay-list text-xs\">x</p>").is_empty());
}

#[test]
fn tsx_comments_in_jsx_whole_line_trailing_and_block() {
    assert!(tsx("{/* style={{ color: '#fff' }} */}").is_empty());
    assert!(tsx("// <p style={{ color: '#fff' }} />").is_empty());
    assert!(tsx("const a = 1; // <p style={{ color: '#fff' }} />").is_empty());
    assert!(tsx("<p>x</p> {/* text-[13px] */}").is_empty());
    assert_eq!(
        lines(
            "a.tsx",
            "{/*\n  #fff\n  fontSize: 13\n*/}\n<p style={{ color: '#fff' }}>x</p>\n"
        ),
        [5]
    );
    assert_eq!(
        lines("a.tsx", "{/* note */}<p style={{ color: '#fff' }}>x</p>\n"),
        [1]
    );
}

#[test]
fn tsx_comment_markers_inside_strings_are_not_comments() {
    assert_eq!(tsx("const u = 'http://x'; const c = '#fff';"), [Colour]);
    assert_eq!(tsx("const u = \"a // b\"; const c = \"#fff\";"), [Colour]);
    assert_eq!(tsx("const u = `a // b`; const c = '#fff';"), [Colour]);
    assert_eq!(
        tsx("<a href=\"https://x.dev\" style={{ color: '#fff' }}>d</a>"),
        [Colour]
    );
    assert!(tsx("<a href=\"https://x.dev\">docs</a>").is_empty());
}

#[test]
fn a_trailing_slash_slash_is_a_comment_in_tsx_only() {
    assert_eq!(lines("a.css", "a { color: #fff } // x\n"), [1]);
    assert_eq!(lines("a.css", "a { margin: 0 } // #fff\n"), [1]);
    assert!(lines("a.tsx", "const a = 1; // #fff\n").is_empty());
}

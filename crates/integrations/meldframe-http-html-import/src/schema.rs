//! The tool's name, the text a client reads and the JSON Schema of its arguments (`docs/import-html.md` section 4).

use html_import::map::MAX_INPUT_BYTES;
use meldframe_http::mcp::MAX_KEY;
use serde_json::{Value, json};

/// The name of the tool.
pub const TOOL: &str = "import_html";

/// The digest of the text is part of the key of every op (`{key}:{digest}:{i}`), and the key of a batch header is at
/// most [`MAX_KEY`] characters: the colon and the 12 digits leave this much for the caller's key.
pub const DIGEST_CHARS: usize = 12;

/// The longest idempotency key of this tool, in characters.
pub const MAX_IMPORT_KEY: usize = MAX_KEY - DIGEST_CHARS - 1;

/// The longest name of the root frame, in characters (the file's limit for a name).
pub const MAX_NAME: usize = 256;

/// The text a client reads.
pub const DESCRIPTION: &str = "Import HTML and CSS text that you read from the user's project as editable nodes of a file, so you can pull a section of existing code onto the canvas, bind variables to it, change it and export it again. Pass file, html (a fragment or a whole document) and, if you have more rules, css (read after the style blocks, so at equal specificity it wins). html and css together are at most 262144 bytes (256 KiB, counted in bytes, not characters). The text is only read: no script runs and nothing is fetched, so a <link>, an @import, a url() or a src is never followed. Linked style sheets are yours to resolve: read them and put their rules in css. The tool does not compute the page the way a browser does. It reads the CSS the author wrote (rules with type, class, id, attribute, universal and compound selectors and the descendant and child combinators, style attributes, custom properties and var(), px, rem, em and pt, hex, rgb, hsl, oklch and named colours, and a small fixed table of the browser's default styles: the size and weight of headings, the margins of p, ul, ol and blockquote, the 8 px margin of body) and it refuses what it cannot hold: @media, grid, transforms, line-height, font-family (Inter is the only font), min and max sizes, :hover and the other state selectors, and more. If you have a browser, the input it reads best is HTML whose styles are inline and already computed (px sizes, no percentages); that recipe is a suggestion, no script here does it and its fidelity has not been measured. The Tailwind export of this server (export_frame with format tailwind) imports back faithfully only together with the css it returns: send both, because its reset (box-sizing, margins) and its variables live in that css and the html alone has the browser's defaults under it. Other Tailwind classes (p-4, text-lg, bg-blue-500, md:flex) are listed as unresolved: send the compiled style sheet in css and they are read as rules. The root frame is the page (the body, or the elements of a fragment): named by name, else the title, else Imported HTML, as wide as the page's CSS says, else width (default 1440). It is a new top-level frame 40 px to the right of the rightmost one, or inside parentId (a frame of the file: E_PARENT_NOT_FOUND when it is not there, E_PARENT_NOT_FRAME when it is not a frame). Every node gets a new id (the data-node ids of an export are not reused, so importing an export into the file it came from is legal) and is named by its data-name, else its id, its first class or its tag. An img becomes a rect of its size with no fill and its src is listed under assets: nothing is fetched, so upload the image with upload_asset and set it with updateNode; svg, canvas, video and the like become an empty rect. The import only adds nodes, as one atomic call and one batch of at most 100 commands, all or none. HTML that makes more is refused as E_IMPORT_HTML_TOO_MANY: send one section at a time with parentId. There is no ifSeq, because adding a node cannot overwrite one. Every call needs an idempotencyKey (also accepted as idempotency_key, 1 to 87 characters here): send the same call again with the same key after a timeout and nothing is written twice (replayed is true); the same key with other text, name, width or parentId is a new call. Pass note as in apply_commands (at most 500 characters and 3 line breaks, no hidden characters); the answer says batch {n, note}. You cannot undo an import: removing a frame needs the owner's approval, so ask the person to revert the batch. The answer has root (the id of the root frame), commands, seq and replayed; nodes, one row for each node written with its id, name, type, source (the element it came from, as tag.class#id), approximated (what was carried in part) and unsupported (what was not read, with the code), the first 20 of each, with approximatedOmitted and unsupportedOmitted counting the rest; skipped (elements that make no node); assets; warnings (default layer names, layout findings such as overflow and fill_under_hug, and text_size_estimated: the server measures text with an estimate and not with the browser's font, so the sizes of text that hugs are not exact); and report (counts of what was repaired, cut, refused or listed, and at most 200 entries and 32 KiB of them, omitted counts the rest). A refused property is never silent: it is on its node under unsupported and in the report. dryRun true writes nothing and returns the same answer after checking the commands against the file, so you can read what would be refused first (the quota and the policy are not tried). strict true fails the call as E_IMPORT_HTML_UNSUPPORTED, with the refused properties listed, when anything was refused, and writes nothing; a class that no rule names is only listed, not refused. Other errors: E_IMPORT_HTML_TOO_LARGE (over 256 KiB, nothing is read), E_IMPORT_HTML_TOO_MANY_ELEMENTS (more than 5,000 elements: send one section at a time) and E_IMPORT_HTML_EMPTY (no element makes a node). A command that the file's checks refuse carries its E_ code, its index and source, the element it came from. Malformed HTML is not an error: the reader repairs it by fixed rules and the report says where.";

fn key(spelling: &str, other: &str) -> Value {
    json!({"type": "string", "minLength": 1, "maxLength": MAX_IMPORT_KEY,
        "description": format!("The idempotency key of the call (1 to {MAX_IMPORT_KEY} characters). Send {spelling} or {other}, not both.")})
}

/// The JSON Schema of the arguments.
pub fn input_schema() -> Value {
    json!({"type": "object", "required": ["file", "html"], "additionalProperties": false, "properties": {
        "file": {"type": "string", "minLength": 1, "maxLength": 128,
            "description": "The name of the file: 1 to 128 characters, no slashes or control characters. A name that does not exist yet is a new, empty file."},
        "html": {"type": "string", "maxLength": MAX_INPUT_BYTES,
            "description": "The HTML text to import: a fragment or a whole document. html and css together are at most 262144 bytes (256 KiB, counted in bytes, not characters)."},
        "css": {"type": "string", "maxLength": MAX_INPUT_BYTES,
            "description": "More rules, with the grammar of a style block, read after the style blocks of the html. Put the rules of linked style sheets here."},
        "parentId": {"type": "string", "minLength": 1, "maxLength": 64,
            "description": "The id of a frame of the file to add the page into. Left out: a new top-level frame to the right of the others."},
        "name": {"type": "string", "minLength": 1, "maxLength": MAX_NAME,
            "description": "The name of the root frame, at most 256 characters without control or hidden characters. Left out: the title of the page, else Imported HTML."},
        "width": {"type": "number", "exclusiveMinimum": 0, "maximum": 100_000, "default": 1440,
            "description": "The width of the root frame in px when the page's CSS gives none."},
        "idempotencyKey": key("idempotencyKey", "idempotency_key"),
        "idempotency_key": key("idempotency_key", "idempotencyKey"),
        "note": {"type": "string", "maxLength": 500,
            "description": "As in apply_commands: what this batch does and why, at most 500 characters and 3 line breaks, no hidden characters."},
        "dryRun": {"type": "boolean", "default": false,
            "description": "True: write nothing and return the same answer, after the commands are checked against the file."},
        "strict": {"type": "boolean", "default": false,
            "description": "True: fail the call as E_IMPORT_HTML_UNSUPPORTED, writing nothing, when any property or selector was refused."}
    }})
}

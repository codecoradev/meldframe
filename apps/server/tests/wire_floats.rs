//! The server reads a number as the client wrote it, to the last bit. A client keeps the command it sent and compares it
//! with the one the server gives back (its own op coming home): a number the server read one bit off would come back as
//! another command, and the client would take its file for diverged and load it again. `serde_json` reads floats this way
//! only with its `float_roundtrip` feature, which this crate asks for.

#[test]
fn a_float_written_by_a_client_is_read_back_exactly() {
    // A y that a drop at a zoom of 110% produced (`223.73056994818654` is its shortest form, one bit from `...657`).
    for text in [
        "223.73056994818654",
        "450.0518134715026",
        "0.1",
        "1e-7",
        "123456789.12345678",
    ] {
        let read: f64 = serde_json::from_str(text).unwrap();
        assert_eq!(read, text.parse::<f64>().unwrap(), "{text}");
        assert_eq!(
            serde_json::to_string(&read)
                .unwrap()
                .parse::<f64>()
                .unwrap(),
            read
        );
    }
}

#[test]
fn a_command_comes_back_as_it_was_sent() {
    let sent =
        r#"{"type":"updateNode","id":"c","patch":{"x":450.0518134715026,"y":223.73056994818654}}"#;
    let command: meldframe_protocol::Command = serde_json::from_str(sent).unwrap();
    assert_eq!(serde_json::to_string(&command).unwrap(), sent);
}

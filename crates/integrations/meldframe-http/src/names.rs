//! The names the adapter keys on: a client address, a file and a tool.

/// The key of a client address: an IPv4 address, or the /64 of an IPv6 one (a host can use any address of its
/// prefix); an IPv4-mapped IPv6 address is its IPv4. The rate limiter and the connection cap use it.
pub fn client_key(ip: std::net::IpAddr) -> String {
    use std::net::IpAddr;
    match ip {
        IpAddr::V4(v4) => v4.to_string(),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => v4.to_string(),
            None => {
                let s = v6.segments();
                format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
            }
        },
    }
}

/// A file name the server should see: the rule of `meldframe_text::file_name` (1 to 128 bytes, not `.` or `..`, not
/// only whitespace, no control or hidden character), and no slash or backslash.
pub fn valid_file(name: &str) -> bool {
    meldframe_text::file_name(name).is_ok() && !name.contains(['/', '\\'])
}

/// A tool name: 1 to 64 bytes of ASCII letters, digits, `_`, `-` and `.`.
pub(crate) fn valid_tool(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}

pub(crate) const BAD_NAME: &str = "a file name is 1 to 128 bytes without slashes, control or invisible formatting characters, and a tool name is 1 to 64 letters, digits, _ - or .";

use super::*;

#[test]
fn cleanup_starts_by_leaving_terminal_strings_and_synchronized_output() {
    assert!(VT_CLEANUP_SEQUENCE.starts_with(b"\x1b]\x1b\\\x1b[?2026l"));
}

#[test]
fn cleanup_covers_modern_input_and_report_modes() {
    for sequence in [
        b"?9;1000;1002;1003;1004;1005;1006;1015;1016;2004;2031;2033;2048;5522l".as_slice(),
        b"\x1b[<8u",
        b"\x1b[=0u",
        b"\x1b[>4;0m",
    ] {
        assert!(
            VT_CLEANUP_SEQUENCE
                .windows(sequence.len())
                .any(|window| window == sequence)
        );
    }
}

#[test]
fn fallback_leaves_terminal_strings_before_ris() {
    assert!(VT_FALLBACK_SEQUENCE.starts_with(b"\x1b]\x1b\\\x1bc"));
}

#[test]
fn unreadable_terminfo_has_a_precise_public_error() {
    assert!(matches!(
        map_terminfo_error(terminfo::Error::Unreadable),
        Error::TerminfoUnreadable
    ));
}

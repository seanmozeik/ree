use std::borrow::Cow;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;

use proptest::collection::vec;
use proptest::prelude::{ProptestConfig, any, proptest};

use super::*;

fn test_entry(string_offset: i16, table: &[u8]) -> Vec<u8> {
    let mut data = vec![0; 14 + table.len()];
    data[0..2].copy_from_slice(&MAGIC_STANDARD.to_le_bytes());
    data[8..10].copy_from_slice(&1_u16.to_le_bytes());
    let table_length = u16::try_from(table.len()).unwrap_or(u16::MAX);
    data[10..12].copy_from_slice(&table_length.to_le_bytes());
    data[12..14].copy_from_slice(&string_offset.to_le_bytes());
    data[14..].copy_from_slice(table);
    data
}

#[test]
fn terminal_names_cannot_traverse_paths() {
    assert!(is_valid_term_name(b"xterm-256color"));
    assert!(is_valid_term_name(b"screen.xterm-256color"));
    assert!(!is_valid_term_name(b""));
    assert!(!is_valid_term_name(b".."));
    assert!(!is_valid_term_name(b"../xterm"));
    assert!(!is_valid_term_name(b"xterm/name"));
    assert!(!is_valid_term_name(b"xterm name"));
}

#[test]
fn parser_accepts_standard_and_extended_headers() {
    let mut standard = [0_u8; 12];
    standard[0..2].copy_from_slice(&MAGIC_STANDARD.to_le_bytes());
    assert!(Entry::parse(&standard).is_ok());

    let mut extended = [0_u8; 12];
    extended[0..2].copy_from_slice(&MAGIC_EXTENDED.to_le_bytes());
    assert!(Entry::parse(&extended).is_ok());
}

#[test]
fn parser_rejects_a_truncated_declared_string_table() {
    let mut data = [0_u8; 12];
    data[0..2].copy_from_slice(&MAGIC_STANDARD.to_le_bytes());
    data[10..12].copy_from_slice(&1_u16.to_le_bytes());
    assert!(matches!(Entry::parse(&data), Err(Error::Truncated)));
}

#[test]
fn parser_aligns_the_combined_name_and_boolean_sections() -> Result<(), Error> {
    let mut data = vec![0_u8; 19];
    data[0..2].copy_from_slice(&MAGIC_STANDARD.to_le_bytes());
    data[2..4].copy_from_slice(&1_u16.to_le_bytes());
    data[4..6].copy_from_slice(&1_u16.to_le_bytes());
    data[8..10].copy_from_slice(&1_u16.to_le_bytes());
    data[10..12].copy_from_slice(&3_u16.to_le_bytes());
    data[16..].copy_from_slice(b"ok\0");

    let entry = Entry::parse(&data)?;

    assert_eq!(entry.string_raw(0)?, Some(b"ok".as_slice()));
    Ok(())
}

#[test]
fn extended_parser_uses_four_byte_numbers() -> Result<(), Error> {
    let mut data = vec![0_u8; 21];
    data[0..2].copy_from_slice(&MAGIC_EXTENDED.to_le_bytes());
    data[6..8].copy_from_slice(&1_u16.to_le_bytes());
    data[8..10].copy_from_slice(&1_u16.to_le_bytes());
    data[10..12].copy_from_slice(&3_u16.to_le_bytes());
    data[18..].copy_from_slice(b"ok\0");

    let entry = Entry::parse(&data)?;

    assert_eq!(entry.string_raw(0)?, Some(b"ok".as_slice()));
    Ok(())
}

#[test]
fn capability_rejects_an_offset_outside_the_string_table() -> Result<(), Error> {
    let data = test_entry(2, b"x\0");
    let entry = Entry::parse(&data)?;
    assert!(matches!(entry.string_raw(0), Err(Error::BadStringOffset)));
    Ok(())
}

#[test]
fn capability_requires_a_terminator_inside_the_string_table() -> Result<(), Error> {
    let data = test_entry(0, b"xy");
    let entry = Entry::parse(&data)?;
    assert!(matches!(
        entry.string_raw(0),
        Err(Error::UnterminatedString)
    ));
    Ok(())
}

#[test]
fn capability_returns_a_bounded_string() -> Result<(), Error> {
    let data = test_entry(0, b"ok\0");
    let entry = Entry::parse(&data)?;
    assert_eq!(entry.string_raw(0)?, Some(b"ok".as_slice()));
    Ok(())
}

#[test]
fn strips_terminfo_padding_markers() {
    assert_eq!(strip_padding(b"a$<10>b$<1.5*/>c").as_ref(), b"abc");
    assert_eq!(strip_padding(b"a$<2/*>b").as_ref(), b"ab");
    assert_eq!(
        strip_padding(b"a$<unterminated").as_ref(),
        b"a$<unterminated"
    );
    for invalid in [
        b"$<>".as_slice(),
        b"$<abc>",
        b"$<.5>",
        b"$<1.>",
        b"$<1.25>",
        b"$<1**>",
        b"$<1//>",
        b"$<1x>",
    ] {
        assert_eq!(strip_padding(invalid).as_ref(), invalid);
    }
    assert_eq!(strip_padding(b"$<outer$<2>>").as_ref(), b"$<outer$<2>>");
    assert!(matches!(strip_padding(b"plain"), Cow::Borrowed(_)));
}

#[test]
fn recognizes_known_vt_terminals() {
    for term in [
        b"xterm-256color".as_slice(),
        b"xterm-ghostty",
        b"ghostty",
        b"screen.xterm-256color",
        b"st-256color",
        b"vt100",
        b"foot+base",
    ] {
        assert!(is_vt_compatible(OsStr::from_bytes(term)), "{term:?}");
    }

    for term in [
        b"dumb".as_slice(),
        b"stupid",
        b"status",
        b"vtable",
        b"linuxbrew",
        b"xterm/unsafe",
    ] {
        assert!(!is_vt_compatible(OsStr::from_bytes(term)), "{term:?}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn arbitrary_entries_do_not_panic(data in vec(any::<u8>(), 0..=MAX_ENTRY_SIZE)) {
        if let Ok(entry) = Entry::parse(&data) {
            let _strings = [
                entry.string(48),
                entry.string(49),
                entry.string(50),
                entry.string(122),
                entry.string(123),
                entry.string(124),
                entry.string(270),
            ];
        }
    }
}

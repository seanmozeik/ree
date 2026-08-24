use std::borrow::Cow;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;

use thiserror::Error as ThisError;

const MAGIC_STANDARD: u16 = 0o0432;
const MAGIC_EXTENDED: u16 = 0o01036;
pub const MAX_ENTRY_SIZE: usize = 32_768;

const STRING_IS1: usize = 48;
const STRING_IS2: usize = 49;
const STRING_IS3: usize = 50;
const STRING_RS1: usize = 122;
const STRING_RS2: usize = 123;
const STRING_RS3: usize = 124;
const STRING_CLEAR_MARGINS: usize = 270;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ThisError)]
pub enum Error {
    #[error("TERM is not a valid terminal name")]
    InvalidTermName,
    #[error("no compiled terminfo entry was found")]
    NotFound,
    #[error("compiled terminfo entry is smaller than its header")]
    TooSmall,
    #[error("compiled terminfo entry has an unknown magic number")]
    BadMagic,
    #[error("compiled terminfo entry ends before a declared section")]
    Truncated,
    #[error("compiled terminfo string offset is outside its table")]
    BadStringOffset,
    #[error("compiled terminfo string is not NUL-terminated")]
    UnterminatedString,
    #[error("compiled terminfo entry is larger than 32 KiB")]
    TooLarge,
    #[error("compiled terminfo entry could not be read")]
    Unreadable,
}

#[derive(Debug)]
struct Entry<'data> {
    data: &'data [u8],
    string_offsets_start: usize,
    string_table_start: usize,
    string_table_end: usize,
    string_count: usize,
}

impl<'data> Entry<'data> {
    fn parse(data: &'data [u8]) -> Result<Self, Error> {
        if data.len() < 12 {
            return Err(Error::TooSmall);
        }

        let magic = read_u16(data, 0).ok_or(Error::TooSmall)?;
        if magic != MAGIC_STANDARD && magic != MAGIC_EXTENDED {
            return Err(Error::BadMagic);
        }

        let name_size = usize::from(read_u16(data, 2).ok_or(Error::TooSmall)?);
        let boolean_count = usize::from(read_u16(data, 4).ok_or(Error::TooSmall)?);
        let number_count = usize::from(read_u16(data, 6).ok_or(Error::TooSmall)?);
        let string_count = usize::from(read_u16(data, 8).ok_or(Error::TooSmall)?);
        let string_table_size = usize::from(read_u16(data, 10).ok_or(Error::TooSmall)?);
        let number_width = if magic == MAGIC_EXTENDED { 4 } else { 2 };

        let after_names = checked_add(12, name_size)?;
        let after_booleans = padded_even(checked_add(after_names, boolean_count)?)?;
        let string_offsets_start = checked_add(after_booleans, number_count * number_width)?;
        let string_table_start = checked_add(string_offsets_start, string_count * 2)?;
        let string_table_end = checked_add(string_table_start, string_table_size)?;

        if string_table_start > data.len() || string_table_end > data.len() {
            return Err(Error::Truncated);
        }

        Ok(Self {
            data,
            string_offsets_start,
            string_table_start,
            string_table_end,
            string_count,
        })
    }

    fn string_raw(&self, index: usize) -> Result<Option<&'data [u8]>, Error> {
        if index >= self.string_count {
            return Ok(None);
        }

        let offset_start = self.string_offsets_start + index * 2;
        let offset = read_i16(self.data, offset_start).ok_or(Error::Truncated)?;
        if offset < 0 {
            return Ok(None);
        }

        let relative = usize::try_from(offset).map_err(|_| Error::BadStringOffset)?;
        let table_length = self.string_table_end - self.string_table_start;
        if relative >= table_length {
            return Err(Error::BadStringOffset);
        }

        let start = self.string_table_start + relative;
        let rest = self
            .data
            .get(start..self.string_table_end)
            .ok_or(Error::BadStringOffset)?;
        let end = rest
            .iter()
            .position(|byte| *byte == 0)
            .ok_or(Error::UnterminatedString)?;
        Ok(rest.get(..end))
    }

    fn string(&self, index: usize) -> Result<Option<Cow<'data, [u8]>>, Error> {
        self.string_raw(index).map(|raw| raw.map(strip_padding))
    }
}

#[derive(Debug)]
pub struct ResetStrings<'data> {
    pub(crate) rs1: Option<Cow<'data, [u8]>>,
    pub(crate) rs2: Option<Cow<'data, [u8]>>,
    pub(crate) rs3: Option<Cow<'data, [u8]>>,
    pub(crate) is1: Option<Cow<'data, [u8]>>,
    pub(crate) is2: Option<Cow<'data, [u8]>>,
    pub(crate) is3: Option<Cow<'data, [u8]>>,
    pub(crate) clear_margins: Option<Cow<'data, [u8]>>,
}

pub fn load(term: &OsStr) -> Result<Vec<u8>, Error> {
    crate::terminfo_db::load(term)
}

pub fn reset_strings(data: &[u8]) -> Result<ResetStrings<'_>, Error> {
    let entry = Entry::parse(data)?;
    Ok(ResetStrings {
        rs1: entry.string(STRING_RS1)?,
        rs2: entry.string(STRING_RS2)?,
        rs3: entry.string(STRING_RS3)?,
        is1: entry.string(STRING_IS1)?,
        is2: entry.string(STRING_IS2)?,
        is3: entry.string(STRING_IS3)?,
        clear_margins: entry.string(STRING_CLEAR_MARGINS)?,
    })
}

pub fn is_vt_compatible(term: &OsStr) -> bool {
    const FAMILIES: &[&[u8]] = &[
        b"xterm",
        b"screen",
        b"tmux",
        b"rxvt",
        b"ansi",
        b"linux",
        b"cygwin",
        b"st",
        b"alacritty",
        b"kitty",
        b"wezterm",
        b"foot",
        b"ghostty",
    ];

    let term = term.as_bytes();
    is_valid_term_name(term)
        && (FAMILIES
            .iter()
            .any(|family| is_terminal_family(term, family))
            || term
                .strip_prefix(b"vt")
                .is_some_and(|suffix| suffix.first().is_some_and(u8::is_ascii_digit)))
}

pub fn validate(data: &[u8]) -> Result<(), Error> {
    Entry::parse(data).map(drop)
}

pub fn is_valid_term_name(term: &[u8]) -> bool {
    !term.is_empty()
        && term.len() <= 255
        && term != b"."
        && term != b".."
        && term
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_' | b'.' | b'+'))
}

fn strip_padding(raw: &[u8]) -> Cow<'_, [u8]> {
    if !raw.contains(&b'$') {
        return Cow::Borrowed(raw);
    }

    let mut output = Vec::with_capacity(raw.len());
    let mut index = 0;
    while index < raw.len() {
        if raw.get(index..index + 2) == Some(b"$<") {
            let body_start = index + 2;
            if let Some(close) = raw
                .get(body_start..)
                .and_then(|tail| tail.iter().position(|byte| *byte == b'>'))
            {
                let end = body_start + close;
                if is_padding_body(&raw[body_start..end]) {
                    index = end + 1;
                    continue;
                }

                output.extend_from_slice(&raw[index..=end]);
                index = end + 1;
                continue;
            }
        }

        if let Some(byte) = raw.get(index) {
            output.push(*byte);
        }
        index += 1;
    }

    Cow::Owned(output)
}

fn is_padding_body(body: &[u8]) -> bool {
    let digits = body.iter().take_while(|byte| byte.is_ascii_digit()).count();
    if digits == 0 {
        return false;
    }

    let mut index = digits;
    if body.get(index) == Some(&b'.') {
        index += 1;
        if !body.get(index).is_some_and(u8::is_ascii_digit) {
            return false;
        }
        index += 1;
        if body.get(index).is_some_and(u8::is_ascii_digit) {
            return false;
        }
    }

    let mut star = false;
    let mut slash = false;
    for byte in &body[index..] {
        match *byte {
            b'*' if !star => star = true,
            b'/' if !slash => slash = true,
            _ => return false,
        }
    }
    true
}

fn is_terminal_family(term: &[u8], family: &[u8]) -> bool {
    term == family
        || term.strip_prefix(family).is_some_and(|suffix| {
            suffix
                .first()
                .is_some_and(|byte| matches!(byte, b'-' | b'.' | b'+'))
        })
}

fn read_u16(data: &[u8], start: usize) -> Option<u16> {
    let bytes: [u8; 2] = data.get(start..start + 2)?.try_into().ok()?;
    Some(u16::from_le_bytes(bytes))
}

fn read_i16(data: &[u8], start: usize) -> Option<i16> {
    let bytes: [u8; 2] = data.get(start..start + 2)?.try_into().ok()?;
    Some(i16::from_le_bytes(bytes))
}

fn checked_add(left: usize, right: usize) -> Result<usize, Error> {
    left.checked_add(right).ok_or(Error::Truncated)
}

fn padded_even(value: usize) -> Result<usize, Error> {
    if value.is_multiple_of(2) {
        Ok(value)
    } else {
        checked_add(value, 1)
    }
}

#[cfg(test)]
#[path = "terminfo_tests.rs"]
mod tests;

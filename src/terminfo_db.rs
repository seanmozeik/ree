use std::env;
use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use fs_err::File;

use crate::terminfo::{self, Error, MAX_ENTRY_SIZE};

const SYSTEM_DIRECTORIES: &[&str] = &[
    "/usr/share/terminfo",
    "/usr/share/misc/terminfo",
    "/lib/terminfo",
    "/usr/local/share/terminfo",
];

pub fn load(term: &OsStr) -> Result<Vec<u8>, Error> {
    let term = term.as_bytes();
    if !terminfo::is_valid_term_name(term) {
        return Err(Error::InvalidTermName);
    }

    load_from_directories(term, search_directories())
}

fn search_directories() -> Vec<PathBuf> {
    let terminfo = env::var_os("TERMINFO");
    let home = env::var_os("HOME");
    let terminfo_dirs = env::var_os("TERMINFO_DIRS");
    search_directories_from(
        terminfo.as_deref(),
        home.as_deref(),
        terminfo_dirs.as_deref(),
    )
}

fn search_directories_from(
    terminfo: Option<&OsStr>,
    home: Option<&OsStr>,
    terminfo_dirs: Option<&OsStr>,
) -> Vec<PathBuf> {
    let mut directories = Vec::new();

    if let Some(directory) = terminfo.filter(|value| !value.is_empty()) {
        push_unique(&mut directories, PathBuf::from(directory));
    }

    if let Some(home) = home.filter(|value| !value.is_empty()) {
        push_unique(&mut directories, PathBuf::from(home).join(".terminfo"));
    }

    if let Some(paths) = terminfo_dirs {
        for path in paths.as_bytes().split(|byte| *byte == b':') {
            let directory = if path.is_empty() {
                PathBuf::from("/usr/share/terminfo")
            } else {
                PathBuf::from(OsString::from_vec(path.to_vec()))
            };
            push_unique(&mut directories, directory);
        }
    }

    for directory in SYSTEM_DIRECTORIES {
        push_unique(&mut directories, PathBuf::from(directory));
    }
    directories
}

fn push_unique(directories: &mut Vec<PathBuf>, directory: PathBuf) {
    if !directories.contains(&directory) {
        directories.push(directory);
    }
}

fn load_from_directories(
    term: &[u8],
    directories: impl IntoIterator<Item = PathBuf>,
) -> Result<Vec<u8>, Error> {
    let mut first_error = None;

    for directory in directories {
        match try_directory(&directory, term) {
            Ok(Some(data)) => return Ok(data),
            Ok(None) => {}
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }

    Err(first_error.unwrap_or(Error::NotFound))
}

fn try_directory(directory: &Path, term: &[u8]) -> Result<Option<Vec<u8>>, Error> {
    let Some(first) = term.first() else {
        return Ok(None);
    };
    let hex_subdirectory = format!("{first:02x}");
    let ascii_subdirectory = OsStr::from_bytes(&term[..1]);
    let name = OsStr::from_bytes(term);
    let mut first_error = None;

    for path in [
        directory.join(&hex_subdirectory).join(name),
        directory.join(ascii_subdirectory).join(name),
    ] {
        match read_entry(&path) {
            Ok(data) => match terminfo::validate(&data) {
                Ok(()) => return Ok(Some(data)),
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            },
            Err(Error::NotFound) => {}
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }

    first_error.map_or(Ok(None), Err)
}

fn read_entry(path: &Path) -> Result<Vec<u8>, Error> {
    let file = File::open(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::NotFound
        } else {
            Error::Unreadable
        }
    })?;
    let mut data = Vec::new();
    file.take((MAX_ENTRY_SIZE + 1) as u64)
        .read_to_end(&mut data)
        .map_err(|_| Error::Unreadable)?;

    if data.len() > MAX_ENTRY_SIZE {
        return Err(Error::TooLarge);
    }

    Ok(data)
}

#[cfg(test)]
#[path = "terminfo_db_tests.rs"]
mod tests;

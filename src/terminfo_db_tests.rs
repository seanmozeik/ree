use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use fs_err as fs;
use tempfile::tempdir;

use super::*;

fn empty_entry() -> Vec<u8> {
    let mut data = vec![0_u8; 12];
    data[0..2].copy_from_slice(&0o432_u16.to_le_bytes());
    data
}

fn write_candidate(
    directory: &Path,
    subdirectory: &str,
    term: &[u8],
    data: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = directory.join(subdirectory);
    fs::create_dir_all(&directory)?;
    fs::write(directory.join(OsStr::from_bytes(term)), data)?;
    Ok(())
}

#[test]
fn empty_primary_paths_do_not_search_the_working_directory() {
    let directories = search_directories_from(
        Some(OsStr::from_bytes(b"")),
        Some(OsStr::from_bytes(b"")),
        None,
    );

    assert!(directories.iter().all(|directory| directory.is_absolute()));
    assert!(!directories.contains(&PathBuf::new()));
    assert!(!directories.contains(&PathBuf::from(".terminfo")));
}

#[test]
fn empty_terminfo_dirs_components_select_the_system_default_once() {
    let directories = search_directories_from(None, None, Some(OsStr::from_bytes(b":/custom::")));

    assert_eq!(
        directories
            .iter()
            .filter(|directory| directory.as_path() == Path::new("/usr/share/terminfo"))
            .count(),
        1
    );
    assert!(directories.contains(&PathBuf::from("/custom")));
}

#[test]
fn a_valid_later_layout_wins_over_a_malformed_entry() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let term = b"xterm-ree-layout-test";
    write_candidate(directory.path(), "78", term, b"bad")?;
    let valid = empty_entry();
    write_candidate(directory.path(), "x", term, &valid)?;

    assert_eq!(try_directory(directory.path(), term)?, Some(valid));
    Ok(())
}

#[test]
fn a_valid_later_directory_wins_over_a_malformed_entry() -> Result<(), Box<dyn std::error::Error>> {
    let malformed = tempdir()?;
    let valid = tempdir()?;
    let term = b"xterm-ree-search-test";
    write_candidate(malformed.path(), "78", term, b"bad")?;
    let expected = empty_entry();
    write_candidate(valid.path(), "78", term, &expected)?;

    let loaded = load_from_directories(
        term,
        [malformed.path().to_path_buf(), valid.path().to_path_buf()],
    )?;

    assert_eq!(loaded, expected);
    Ok(())
}

#[test]
fn a_malformed_only_search_retains_its_error() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let term = b"xterm-ree-malformed-test";
    write_candidate(directory.path(), "78", term, b"bad")?;

    assert!(matches!(
        load_from_directories(term, [directory.path().to_path_buf()]),
        Err(Error::TooSmall)
    ));
    Ok(())
}

#[test]
fn bounds_compiled_entries() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let path = directory.path().join("entry");
    fs::write(&path, vec![0_u8; MAX_ENTRY_SIZE + 1])?;
    assert!(matches!(read_entry(&path), Err(Error::TooLarge)));
    Ok(())
}

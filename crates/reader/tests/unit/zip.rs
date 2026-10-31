use std::io::Write;

use reader::zip::{Archive, ZipError};

use crate::writer::{Packing, zip};

fn archive(bytes: &[u8]) -> Result<Archive, ZipError> {
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(bytes).unwrap();
    Archive::open(file)
}

#[test]
fn stored_and_deflated_entries_read_back_whole_or_by_their_start() {
    let long = "words ".repeat(5000);
    let opened = archive(&zip(&[
        ("plain.txt", Packing::Stored, b"stored bytes"),
        ("packed.txt", Packing::Deflated, long.as_bytes()),
    ]))
    .unwrap();
    assert_eq!(opened.read("plain.txt").unwrap(), b"stored bytes");
    assert_eq!(opened.read("packed.txt").unwrap(), long.as_bytes());
    assert_eq!(opened.head("plain.txt", 6).unwrap(), b"stored");
    assert_eq!(
        opened.head("packed.txt", 100).unwrap(),
        &long.as_bytes()[..100]
    );
    assert!(matches!(opened.read("absent"), Err(ZipError::Missing(_))));
}

#[test]
fn bytes_that_are_not_a_zip_are_refused() {
    assert!(matches!(
        archive(b"plain text, no archive"),
        Err(ZipError::NotZip)
    ));
}

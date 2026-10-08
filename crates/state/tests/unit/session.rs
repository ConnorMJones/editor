use std::path::Path;

use state::session;

#[test]
fn session_file_escapes_root() {
    assert_eq!(
        session::file(Path::new("/state"), Path::new("/home/me/50%/proj")),
        Path::new("/state/%2Fhome%2Fme%2F50%25%2Fproj.toml")
    );
}

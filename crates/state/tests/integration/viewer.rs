use std::fs;
use std::path::Path;
use std::time::Duration;

use model::notice::Notice;
use model::picture::{Bitmap, Encoding};
use model::sheet::{Fit, PageSize, SheetView, Zoom};
use state::project::session;
use state::render::markdown::LocalImage;
use state::render::typeset::Probe;
use state::view::tab::Tab;
use state::viewer::ImageTab;

use crate::harness::Harness;

fn write_png(path: &Path, width: u32, height: u32) {
    image::RgbImage::new(width, height).save(path).unwrap();
}

fn tab(h: &Harness) -> &ImageTab {
    match h.state.layout.focused().active() {
        Some(Tab::Image(tab)) => tab,
        Some(_) | None => panic!("an image is focused"),
    }
}

fn size(h: &Harness) -> PageSize {
    h.state.images[tab(h).file()].stack.sizes()[0]
}

fn sides(size: PageSize) -> (f32, f32) {
    (size.width.get(), size.height.get())
}

#[test]
fn an_image_opens_fitted_whole_at_its_pixel_size() {
    let dir = tempfile::tempdir().unwrap();
    write_png(&dir.path().join("photo.png"), 40, 20);
    let mut h = Harness::new(dir.path());
    h.keys(":o photo.png<ret>");
    assert_eq!(h.focused_tabs(), ["▣photo.png"]);
    assert_eq!(tab(&h).view.fit, Fit::Page);
    assert_eq!(sides(size(&h)), (40.0, 20.0));
}

#[test]
fn an_svg_shows_at_its_own_size() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("figure.svg"),
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="30" height="10"/>"#,
    )
    .unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o figure.svg<ret>");
    assert_eq!(sides(size(&h)), (30.0, 10.0));
}

#[test]
fn view_keys_zoom_and_step_the_fit() {
    let dir = tempfile::tempdir().unwrap();
    write_png(&dir.path().join("photo.png"), 40, 20);
    let mut h = Harness::new(dir.path());
    h.keys(":o photo.png<ret>+");
    assert!(tab(&h).view.zoom > Zoom::ONE);
    h.keys("=zf");
    assert_eq!(tab(&h).view.zoom, Zoom::ONE);
    assert_eq!(tab(&h).view.fit, Fit::Width);
}

#[test]
fn an_image_cannot_be_written() {
    let dir = tempfile::tempdir().unwrap();
    write_png(&dir.path().join("photo.png"), 40, 20);
    let mut h = Harness::new(dir.path());
    h.keys(":o photo.png<ret>:w<ret>");
    assert!(matches!(h.state.notice, Some(Notice::ReadOnly)));
    h.state.notice = None;
    h.run(|state, out| state::apply::apply(state, model::command::Command::Undo, out));
    assert!(
        matches!(h.state.notice, Some(Notice::ReadOnly)),
        "nor undone"
    );
}

#[test]
fn a_file_that_is_not_the_image_it_names_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("broken.png"), "not a png").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o broken.png<ret>");
    assert!(matches!(h.state.notice, Some(Notice::Unreadable { .. })));
    assert!(h.focused_tabs().is_empty());
}

#[test]
fn an_image_changed_on_disk_reloads_at_its_new_size() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().canonicalize().unwrap().join("photo.png");
    write_png(&path, 40, 20);
    let mut h = Harness::new(dir.path());
    h.keys(":o photo.png<ret>");
    // A later mtime than the one read, so the reload sees a change.
    std::thread::sleep(Duration::from_millis(20));
    write_png(&path, 10, 30);
    h.changed(vec![path]);
    assert_eq!(sides(size(&h)), (10.0, 30.0));
}

#[test]
fn a_session_restores_an_image_with_its_view() {
    let dir = tempfile::tempdir().unwrap();
    write_png(&dir.path().join("photo.png"), 40, 20);
    let mut before = Harness::new(dir.path());
    before.keys(":o photo.png<ret>zf++");
    let kept: SheetView = tab(&before).view;
    let text = toml::to_string(&session::snapshot(&before.state)).unwrap();

    let mut after = Harness::new(dir.path());
    let saved: session::Session = toml::from_str(&text).unwrap();
    after
        .run(|state, out| session::restore(state, saved, out))
        .unwrap();
    assert_eq!(after.focused_tabs(), ["▣photo.png"]);
    assert_eq!(tab(&after).view, kept);
}

/// The size the focused rendering's image was read at.
fn shown_size(h: &Harness, path: &Path) -> Option<(f32, f32)> {
    let image = LocalImage {
        path: path.into(),
        encoding: Encoding::Bitmap(Bitmap::Png),
    };
    match h.state.figures.image(&image) {
        Some(Probe::Read(probed)) => Some(sides(probed.size)),
        Some(Probe::Pending | Probe::Failed(_)) | None => None,
    }
}

#[test]
fn a_note_reads_its_image_sizes_and_reads_them_again_on_change() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().canonicalize().unwrap().join("photo.png");
    write_png(&path, 40, 20);
    fs::write(dir.path().join("note.md"), "# Photo\n\n![](photo.png)\n").unwrap();
    let mut h = Harness::new(dir.path());
    h.keys(":o note.md<ret><space>V");
    assert_eq!(shown_size(&h, &path), Some((40.0, 20.0)));
    std::thread::sleep(Duration::from_millis(20));
    write_png(&path, 10, 30);
    h.changed(vec![path.clone()]);
    assert_eq!(shown_size(&h, &path), Some((10.0, 30.0)));
}

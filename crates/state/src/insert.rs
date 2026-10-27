//! Images going into the focused tab, however they arrive (a drop, the picker, `:image`, a
//! paste): a drawing embeds the image's bytes, and a note links an image file kept beside it.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use helix_core::{Selection, Transaction};
use ink::doc::{Board, PageLayer, Paged};
use model::notice::Notice;
use model::picture::{Bitmap, Encoding};

use crate::Effect;
use crate::State;
use crate::apply::edit;
use crate::drawing::InkTabs;
use crate::drawing::embed::{self, Destination};
use crate::drawing::frame;
use crate::job::{FsOp, Job};
use crate::project::files::FileIndex;
use crate::project::path::{RelPath, TextKind};
use crate::view::panes::PanePoint;
use crate::view::tab::Tab;

/// Where an image goes, decided from the focused tab when it was asked for.
#[derive(Clone, Debug)]
pub enum ImageInto {
    /// The image's bytes go into the drawing.
    Drawing(Destination),
    /// The image file is linked from the note, from the folder `dir` beside it.
    Note { dir: PathBuf },
}

pub enum ImageFrom {
    /// A canonical path, inside the project or not, whose name is an image encoding.
    File {
        path: PathBuf,
        encoding: Encoding,
    },
    Pasted(Arc<[u8]>, Encoding),
}

impl ImageFrom {
    /// `path` as an image, or why it isn't one.
    pub fn file(path: PathBuf) -> Result<ImageFrom, Notice> {
        match Encoding::of_path(&path) {
            Some(encoding) => Ok(ImageFrom::File { path, encoding }),
            None => Err(Notice::NotAnImage { path }),
        }
    }
}

/// A file name of lowercase letters, digits and single dashes, never empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KebabName(String); // private: lowercase ascii letters, digits and single dashes

impl KebabName {
    /// `name` lowercased, every run of other characters one dash; `image` when nothing is left.
    pub fn of(name: &str) -> KebabName {
        let parts: Vec<String> = (name.split(|c: char| !c.is_ascii_alphanumeric()))
            .filter(|part| !part.is_empty())
            .map(str::to_ascii_lowercase)
            .collect();
        match parts.join("-") {
            joined if joined.is_empty() => KebabName("image".to_owned()),
            joined => KebabName(joined),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Where an image for the focused tab goes; `at` is a point on the pane, else the view's centre.
pub fn into_focused(state: &State, at: Option<PanePoint>) -> Result<ImageInto, Notice> {
    match state.layout.focused().active() {
        Some(Tab::Ink(InkTabs::Paged(tab))) => {
            let target = PageLayer {
                page: tab.view.centre.page,
                layer: tab.hand.layer,
            };
            let file = tab.file();
            let destination = match at {
                None => Destination::Paged {
                    file,
                    target,
                    centre: tab.view.centre.at,
                },
                Some(PanePoint { at, viewport }) => {
                    let paged = state.pageds[file].edited.drawing();
                    let landed = frame::paged(paged, &tab.view, viewport)
                        .land::<Paged>(at, tab.hand.layer)
                        .ok_or(Notice::OffThePages)?;
                    Destination::Paged {
                        file,
                        target: landed.target,
                        centre: landed.onto.apply(at),
                    }
                }
            };
            Ok(ImageInto::Drawing(destination))
        }
        Some(Tab::Ink(InkTabs::Board(tab))) => {
            let file = tab.file();
            let centre = match at {
                None => tab.view.centre,
                Some(PanePoint { at, viewport }) => {
                    let board = state.boards[file].edited.drawing();
                    let landed = frame::board(board, &tab.view, viewport)
                        .land::<Board>(at, tab.hand.layer)
                        .ok_or(Notice::OffThePages)?;
                    landed.onto.apply(at)
                }
            };
            Ok(ImageInto::Drawing(Destination::Board {
                file,
                target: tab.hand.layer,
                centre,
            }))
        }
        Some(Tab::Text(text)) => {
            let buffer = &state.buffers[text.text().buffer()];
            match buffer.kind {
                TextKind::Markdown => Ok(ImageInto::Note {
                    dir: (buffer.path.parent())
                        .unwrap_or(state.project.root.path())
                        .to_owned(),
                }),
                TextKind::Typst | TextKind::Code => Err(Notice::NoImagesHere),
            }
        }
        Some(Tab::Image(_)) | None => Err(Notice::NoImagesHere),
    }
}

/// Puts the image where `into` says: embedded in a drawing, or linked from the focused note.
pub fn insert(state: &mut State, into: ImageInto, from: ImageFrom, out: &mut Vec<Effect>) {
    let result = match (into, from) {
        (ImageInto::Drawing(into), ImageFrom::File { path, encoding }) => {
            out.push(Effect::Job(Job::Embed {
                path,
                encoding,
                into,
            }));
            Ok(())
        }
        (ImageInto::Drawing(into), ImageFrom::Pasted(encoded, encoding)) => {
            let read = (crate::viewer::probe(&encoded, encoding)).map(|pixels| embed::Read {
                encoded,
                encoding,
                pixels,
            });
            embed::embedded(state, PathBuf::from("clipboard"), into, read);
            Ok(())
        }
        (ImageInto::Note { dir }, ImageFrom::File { path, encoding }) => {
            match RelPath::new(state.project.root.path(), &path) {
                Ok(file) => {
                    link(state, &file);
                    Ok(())
                }
                // A file from outside the project is copied beside the note first.
                Err(_) => {
                    let stem = path
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .unwrap_or("");
                    beside(state, &dir, &KebabName::of(stem), encoding).map(|file| {
                        out.push(Effect::Job(Job::Fs {
                            root: state.project.root.path().to_owned(),
                            op: FsOp::Copy {
                                from: path,
                                to: file.clone(),
                            },
                        }));
                        link(state, &file)
                    })
                }
            }
        }
        (ImageInto::Note { dir }, ImageFrom::Pasted(bytes, encoding)) => {
            let name = KebabName::of(&format!("pasted image {}", stamp(SystemTime::now())));
            beside(state, &dir, &name, encoding).map(|file| {
                out.push(Effect::Job(Job::Fs {
                    root: state.project.root.path().to_owned(),
                    op: FsOp::Write {
                        path: file.clone(),
                        bytes,
                    },
                }));
                link(state, &file)
            })
        }
    };
    match result {
        Ok(()) => {}
        Err(notice) => state.notice = Some(notice),
    }
}

/// A file in `dir` named `name` that doesn't exist yet: `name-2`, `name-3` after.
pub fn beside(
    state: &State,
    dir: &Path,
    name: &KebabName,
    encoding: Encoding,
) -> Result<RelPath, Notice> {
    let extension = extension(encoding);
    let free = (1..)
        .map(|count| match count {
            1 => dir.join(format!("{}.{extension}", name.as_str())),
            count => dir.join(format!("{}-{count}.{extension}", name.as_str())),
        })
        .find(|path| !path.exists())
        .expect("some count is free");
    RelPath::new(state.project.root.path(), &free)
        .map_err(|_| Notice::NoteOutsideProject { path: free })
}

/// `![[file]]` on the cursor's line when it's blank, else on a new line below it.
pub fn link(state: &mut State, file: &RelPath) {
    let name = file.file_name();
    // Before the index arrives, other files of the name can't be ruled out.
    let others = match &state.project.index {
        FileIndex::Ready(indexed) => (indexed.files.iter())
            .filter(|other| other.file_name() == name && *other != file)
            .count(),
        FileIndex::Loading { .. } => 1,
    };
    let target = match others {
        0 => name.to_owned(),
        _ => file.as_str().to_owned(),
    };
    edit(state, |doc, selection, _| {
        let text = doc.text.slice(..);
        let line = text.char_to_line(selection.primary().cursor(text));
        let start = text.line_to_char(line);
        let content = std::borrow::Cow::<str>::from(text.line(line));
        let (at, inserted) = match content.trim() {
            "" => (start, format!("![[{target}]]")),
            _ => {
                let end = start + text.line(line).len_chars()
                    - helix_core::line_ending::get_line_ending(&text.line(line))
                        .map_or(0, |ending| ending.len_chars());
                (end, format!("\n![[{target}]]"))
            }
        };
        let after = at + inserted.chars().count();
        Transaction::change(&doc.text, [(at, at, Some(inserted.into()))].into_iter())
            .with_selection(Selection::point(after))
    });
}

pub fn extension(encoding: Encoding) -> &'static str {
    match encoding {
        Encoding::Bitmap(Bitmap::Png) => "png",
        Encoding::Bitmap(Bitmap::Jpeg) => "jpg",
        Encoding::Bitmap(Bitmap::Webp) => "webp",
        Encoding::Bitmap(Bitmap::Gif) => "gif",
        Encoding::Bitmap(Bitmap::Bmp) => "bmp",
        Encoding::Svg => "svg",
    }
}

/// `YYYY-MM-DD-HHMMSS` in UTC.
pub fn stamp(time: SystemTime) -> String {
    let seconds = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    // Days since 1970 to the civil date, after Howard Hinnant's `civil_from_days`.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted + 2) / 5 + 1;
    let month = (shifted + 2) % 12 + 1;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}-{:02}{:02}{:02}",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

/// The clipboard's image, read by the host for a paste; `None` when it holds none this reads.
pub fn pasted(state: &mut State, into: ImageInto, image: Option<ImageFrom>, out: &mut Vec<Effect>) {
    match image {
        Some(image) => insert(state, into, image, out),
        None => state.notice = Some(Notice::NoImageOnClipboard),
    }
}

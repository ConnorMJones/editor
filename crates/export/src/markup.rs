use std::sync::Arc;

use hayro_write::hayro_syntax::Pdf;
use hayro_write::hayro_syntax::object::{Dict, ObjRef};
use hayro_write::{ExtractionQuery, extract};
use ink::doc::{PageIndex, Paged};
use ink::text::Fonts;
use kurbo::Affine;
use pdf_writer::{Chunk, Filter, Finish, Name, Ref, Str};
use reader::pdf::Document;
use reader::pdf::revise::{BLOB, OURS, Owner, ReviseError, Revision, Update, inline_dict};

use crate::{ExportError, scene};

pub const BLOB_LEVEL: u8 = 6;
pub const PRINT_FLAG: i32 = 4;

#[derive(Debug, thiserror::Error)]
pub enum MarkupError {
    #[error(transparent)]
    Revise(#[from] ReviseError),
    #[error(transparent)]
    Export(#[from] ExportError),
    #[error("the drawn marks could not be read back: {0}")]
    Drawn(String),
}

pub struct Appearance {
    pub page: usize,
    pub form: Ref,
    pub area: kurbo::Rect,
}

pub fn save(document: &Document, paged: &Paged, fonts: &Fonts) -> Result<Update, MarkupError> {
    let mut revision = document.revise()?;
    let shown = appearances(&mut revision, paged, fonts)?;
    let mut chunk = Chunk::new();
    let mut blob = Vec::new();
    ink::format::write_markup(paged, &mut blob);
    let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&blob, BLOB_LEVEL);
    let blob_ref = revision.allocate();
    chunk
        .stream(blob_ref, &compressed)
        .filter(Filter::FlateDecode);
    let mut annotated = Vec::with_capacity(shown.len());
    for appearance in &shown {
        let annotation = revision.allocate();
        let mut dict = chunk.indirect(annotation).dict();
        dict.pair(Name(b"Type"), Name(b"Annot"));
        dict.pair(Name(b"Subtype"), Name(b"Stamp"));
        dict.pair(Name(b"Rect"), rect(appearance.area));
        dict.pair(Name(b"F"), PRINT_FLAG);
        dict.pair(Name(OURS), true);
        dict.pair(
            Name(b"NM"),
            Str(format!("edt-ink-{}", appearance.page).as_bytes()),
        );
        dict.insert(Name(b"AP"))
            .dict()
            .pair(Name(b"N"), appearance.form);
        dict.finish();
        annotated.push((appearance.page, annotation));
    }
    revision.chunk(&chunk)?;
    for page in 0..paged.pages().len() {
        let (id, dict) = revision.page(page)?;
        let ours = annotated
            .iter()
            .find(|(on, _)| *on == page)
            .map(|(_, id)| *id);
        let had = (revision.annotations(&dict).iter()).any(|(_, owner)| *owner == Owner::Ours);
        match (ours, had) {
            (None, false) => {}
            (Some(_), _) | (None, true) => annotate(&mut revision, id, &dict, ours),
        }
    }
    let catalog = revision.catalog().ok_or(ReviseError::NoTrailer)?;
    let blob_entry = format!("{} 0 R", blob_ref.get()).into_bytes();
    revision.replace_dict(revision.root(), &catalog, &[], &[(BLOB, blob_entry)]);
    Ok(revision.finish())
}

pub fn unmarked(document: &Document) -> Result<Option<Update>, MarkupError> {
    match document.blob() {
        None => return Ok(None),
        Some(_) => {}
    }
    let mut revision = document.revise()?;
    for page in 0..document.pages.sizes().len() {
        let (id, dict) = revision.page(page)?;
        let ours =
            (revision.annotations(&dict).into_iter()).find(|(_, owner)| *owner == Owner::Ours);
        match ours {
            Some(_) => annotate(&mut revision, id, &dict, None),
            None => {}
        }
    }
    Ok(Some(revision.finish()))
}

pub fn flatten(document: &Document, paged: &Paged, fonts: &Fonts) -> Result<Update, MarkupError> {
    let mut revision = document.revise()?;
    let shown = appearances(&mut revision, paged, fonts)?;
    let mut chunk = Chunk::new();
    let (save, restore) = (revision.allocate(), revision.allocate());
    chunk.stream(save, b"q\n");
    chunk.stream(restore, b"\nQ\n");
    let drawn: Vec<(usize, Ref, Vec<u8>, Ref)> = (shown.iter())
        .map(|appearance| {
            let name = format!("EdtFlat{}", appearance.form.get()).into_bytes();
            let draw = revision.allocate();
            let mut content = b"q /".to_vec();
            content.extend(&name);
            content.extend(b" Do Q\n");
            chunk.stream(draw, &content);
            (appearance.page, appearance.form, name, draw)
        })
        .collect();
    revision.chunk(&chunk)?;
    let reference = |id: Ref| format!("{} 0 R", id.get()).into_bytes();
    for page in 0..paged.pages().len() {
        let (id, dict) = revision.page(page)?;
        let had = (revision.annotations(&dict).iter()).any(|(_, owner)| *owner == Owner::Ours);
        match (drawn.iter().find(|(on, ..)| *on == page), had) {
            (None, false) => {}
            (None, true) => annotate(&mut revision, id, &dict, None),
            (Some((_, form, name, draw)), _) => {
                let resources = revision.resources(&dict).unwrap_or_else(Dict::empty);
                let xobjects = resources
                    .get::<Dict<'_>>(b"XObject")
                    .unwrap_or_else(Dict::empty);
                let xobjects = inline_dict(&xobjects, &[], &[(name.as_slice(), reference(*form))]);
                let resources_ref = revision.allocate();
                revision.replace_dict(
                    ObjRef::new(resources_ref.get(), 0),
                    &resources,
                    &[],
                    &[(b"XObject", xobjects)],
                );
                let mut contents = b"[".to_vec();
                contents.extend(reference(save));
                for part in revision.contents(&dict) {
                    contents.push(b' ');
                    contents.extend(part);
                }
                contents.push(b' ');
                contents.extend(reference(restore));
                contents.push(b' ');
                contents.extend(reference(*draw));
                contents.push(b']');
                let theirs = theirs(&revision, &dict);
                let mut added: Vec<(&[u8], Vec<u8>)> = vec![
                    (b"Contents", contents),
                    (b"Resources", reference(resources_ref)),
                ];
                match theirs.as_slice() {
                    [] => {}
                    [_, ..] => added.push((b"Annots", list(&theirs))),
                }
                revision.replace_dict(id, &dict, &[b"Annots"], &added);
            }
        }
    }
    let catalog = revision.catalog().ok_or(ReviseError::NoTrailer)?;
    revision.replace_dict(revision.root(), &catalog, &[BLOB], &[]);
    Ok(revision.finish())
}

fn appearances(
    revision: &mut Revision<'_>,
    paged: &Paged,
    fonts: &Fonts,
) -> Result<Vec<Appearance>, MarkupError> {
    let marked: Vec<usize> = (0..paged.pages().len())
        .filter(|page| !scene::page(paged, PageIndex(*page)).marks.is_empty())
        .collect();
    match marked.as_slice() {
        [] => return Ok(Vec::new()),
        [_, ..] => {}
    }
    let drawn = crate::pdf::pdf(
        marked.iter().map(|page| scene::Scene {
            underlay: &scene::BLANK,
            ..scene::page(paged, PageIndex(*page))
        }),
        fonts,
    )?;
    let pdf =
        Pdf::new(Arc::new(drawn)).map_err(|error| MarkupError::Drawn(format!("{error:?}")))?;
    let queries: Vec<ExtractionQuery> = (0..marked.len())
        .map(ExtractionQuery::new_xobject)
        .collect();
    let extracted = extract(
        &pdf,
        Box::new(|| revision.allocate()),
        Default::default(),
        |_| {},
        &queries,
    )
    .map_err(|error| MarkupError::Drawn(format!("{error:?}")))?;
    revision.chunk(&extracted.chunk)?;
    let mut chunk = Chunk::new();
    let mut shown = Vec::with_capacity(marked.len());
    for (page, form) in marked.iter().zip(extracted.root_refs) {
        let form = form.map_err(|error| MarkupError::Drawn(format!("{error:?}")))?;
        let size = paged.stack().place(PageIndex(*page)).size;
        let (width, height) = (f64::from(size.width.get()), f64::from(size.height.get()));
        let flip = Affine::new([1.0, 0.0, 0.0, -1.0, 0.0, height]);
        let to_user = (revision.document.user_from_view(*page))
            .ok_or(MarkupError::Revise(ReviseError::NoPage(*page)))?
            * flip;
        let area = to_user.transform_rect_bbox(kurbo::Rect::new(0.0, 0.0, width, height));
        let [a, b, c, d, e, f] = to_user.as_coeffs();
        let content = format!("q {a} {b} {c} {d} {e} {f} cm /M Do Q\n");
        let wrapper = revision.allocate();
        let mut xobject = chunk.form_xobject(wrapper, content.as_bytes());
        xobject.bbox(rect(area));
        xobject.resources().x_objects().pair(Name(b"M"), form);
        xobject.finish();
        shown.push(Appearance {
            page: *page,
            form: wrapper,
            area,
        });
    }
    revision.chunk(&chunk)?;
    Ok(shown)
}

fn annotate<'a>(revision: &mut Revision<'a>, id: ObjRef, dict: &Dict<'a>, ours: Option<Ref>) {
    let mut kept = theirs(revision, dict);
    kept.extend(ours.map(|ours| format!("{} 0 R", ours.get()).into_bytes()));
    match kept.as_slice() {
        [] => revision.replace_dict(id, dict, &[b"Annots"], &[]),
        [_, ..] => revision.replace_dict(id, dict, &[], &[(b"Annots", list(&kept))]),
    }
}

fn theirs<'a>(revision: &Revision<'a>, dict: &Dict<'a>) -> Vec<Vec<u8>> {
    (revision.annotations(dict).into_iter())
        .filter_map(|(raw, owner)| match owner {
            Owner::Theirs => Some(raw),
            Owner::Ours => None,
        })
        .collect()
}

fn list(items: &[Vec<u8>]) -> Vec<u8> {
    let mut out = b"[".to_vec();
    for (index, item) in items.iter().enumerate() {
        match index {
            0 => {}
            _ => out.push(b' '),
        }
        out.extend(item);
    }
    out.push(b']');
    out
}

fn rect(area: kurbo::Rect) -> pdf_writer::Rect {
    pdf_writer::Rect::new(
        area.x0 as f32,
        area.y0 as f32,
        area.x1 as f32,
        area.y1 as f32,
    )
}

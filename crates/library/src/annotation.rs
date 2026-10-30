use std::cmp::Reverse;
use std::ops::Range;
use std::sync::Arc;

use model::sheet::PageIndex;
use reader::pdf::Document;
use uuid::Uuid;

pub const QUOTE_CONTEXT: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AnnotationId(pub Uuid);

impl AnnotationId {
    pub fn new() -> AnnotationId {
        AnnotationId(Uuid::now_v7())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TextSpan {
    pub page: PageIndex,
    pub chars: Range<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quote {
    exact: String, // private: an empty quote would match everywhere when re-anchoring
    pub prefix: String,
    pub suffix: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comment(String); // private: a blank comment is no comment, which `Option<Comment>` already says

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Highlight {
    pub id: AnnotationId,
    pub span: TextSpan,
    pub quote: Quote,
    pub comment: Option<Comment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lost {
    pub id: AnnotationId,
    pub page: PageIndex,
    pub quote: Quote,
    pub comment: Option<Comment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bookmark {
    pub id: AnnotationId,
    pub page: PageIndex,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Annotation {
    Highlight(Highlight),
    Lost(Lost),
    Bookmark(Bookmark),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Notebook {
    pub highlights: Vec<Highlight>,
    pub lost: Vec<Lost>,
    pub bookmarks: Vec<Bookmark>,
}

#[derive(Clone, Debug)]
pub enum Change {
    Put(Annotation),
    Remove(AnnotationId),
}

impl Comment {
    pub fn new(text: String) -> Option<Comment> {
        (!text.trim().is_empty()).then_some(Comment(text))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Quote {
    pub fn new(exact: String, prefix: String, suffix: String) -> Option<Quote> {
        (!exact.trim().is_empty()).then_some(Quote {
            exact,
            prefix,
            suffix,
        })
    }

    pub fn exact(&self) -> &str {
        &self.exact
    }

    pub fn find_in(&self, document: &Arc<Document>, near: PageIndex) -> Option<TextSpan> {
        let shared = |a: &mut dyn Iterator<Item = char>, b: &mut dyn Iterator<Item = char>| {
            a.zip(b).take_while(|(a, b)| a == b).count()
        };
        (0..document.pages.sizes().len())
            .filter_map(|page| Some((page, document.read_text(page).ok()?)))
            .flat_map(|(page, text)| {
                (text.text.match_indices(self.exact.as_str()))
                    .map(|(start, exact)| {
                        let end = start + exact.len();
                        let context =
                            shared(
                                &mut text.text[..start].chars().rev(),
                                &mut self.prefix.chars().rev(),
                            ) + shared(&mut text.text[end..].chars(), &mut self.suffix.chars());
                        let span = TextSpan {
                            page: PageIndex(page),
                            chars: text.chars_in(start..end),
                        };
                        (span, context, Reverse(page.abs_diff(near.0)))
                    })
                    .collect::<Vec<_>>()
            })
            .filter(|(span, ..)| !span.chars.is_empty())
            .max_by_key(|&(_, context, distance)| (context, distance))
            .map(|(span, ..)| span)
    }
}

impl Highlight {
    pub fn of(document: &Document, span: TextSpan) -> Option<Highlight> {
        let text = document.text(span.page.0)?;
        let exact = text.slice(span.chars.clone());
        let start = text.byte(span.chars.start);
        let before = &text.text[..start];
        let from = (before.char_indices().rev().nth(QUOTE_CONTEXT - 1)).map_or(0, |(at, _)| at);
        let after = &text.text[start + exact.len()..];
        let quote = Quote::new(
            exact.to_owned(),
            before[from..].to_owned(),
            after.chars().take(QUOTE_CONTEXT).collect(),
        )?;
        Some(Highlight {
            id: AnnotationId::new(),
            span,
            quote,
            comment: None,
        })
    }

    pub fn reanchor(self, document: &Arc<Document>) -> Annotation {
        let unmoved = (document.read_text(self.span.page.0))
            .is_ok_and(|text| text.slice(self.span.chars.clone()) == self.quote.exact);
        match (unmoved, self.quote.find_in(document, self.span.page)) {
            (true, _) => Annotation::Highlight(self),
            (false, Some(span)) => Annotation::Highlight(Highlight { span, ..self }),
            (false, None) => Annotation::Lost(Lost {
                id: self.id,
                page: self.span.page,
                quote: self.quote,
                comment: self.comment,
            }),
        }
    }

    pub fn place(&self) -> (PageIndex, usize) {
        (self.span.page, self.span.chars.start)
    }
}

impl Lost {
    pub fn reanchor(self, document: &Arc<Document>) -> Annotation {
        match self.quote.find_in(document, self.page) {
            Some(span) => Annotation::Highlight(Highlight {
                id: self.id,
                span,
                quote: self.quote,
                comment: self.comment,
            }),
            None => Annotation::Lost(self),
        }
    }
}

impl Annotation {
    pub fn id(&self) -> AnnotationId {
        match self {
            Annotation::Highlight(highlight) => highlight.id,
            Annotation::Lost(lost) => lost.id,
            Annotation::Bookmark(bookmark) => bookmark.id,
        }
    }

    pub fn place(&self) -> (PageIndex, usize) {
        match self {
            Annotation::Highlight(highlight) => highlight.place(),
            Annotation::Lost(Lost { page, .. }) | Annotation::Bookmark(Bookmark { page, .. }) => {
                (*page, 0)
            }
        }
    }
}

impl Notebook {
    pub fn apply(&mut self, change: &Change) {
        let replaced = match change {
            Change::Put(annotation) => annotation.id(),
            Change::Remove(id) => *id,
        };
        self.highlights.retain(|kept| kept.id != replaced);
        self.lost.retain(|kept| kept.id != replaced);
        self.bookmarks.retain(|kept| kept.id != replaced);
        match change {
            Change::Put(Annotation::Highlight(highlight)) => {
                let at =
                    (self.highlights).partition_point(|kept| kept.place() <= highlight.place());
                self.highlights.insert(at, highlight.clone());
            }
            Change::Put(Annotation::Lost(lost)) => {
                let at = self.lost.partition_point(|kept| kept.page <= lost.page);
                self.lost.insert(at, lost.clone());
            }
            Change::Put(Annotation::Bookmark(bookmark)) => {
                let at = self
                    .bookmarks
                    .partition_point(|kept| kept.page <= bookmark.page);
                self.bookmarks.insert(at, bookmark.clone());
            }
            Change::Remove(_) => {}
        }
    }

    pub fn in_reading_order(&self) -> Vec<Annotation> {
        let mut all: Vec<Annotation> = (self.highlights.iter().cloned())
            .map(Annotation::Highlight)
            .chain(self.lost.iter().cloned().map(Annotation::Lost))
            .chain(self.bookmarks.iter().cloned().map(Annotation::Bookmark))
            .collect();
        all.sort_by_key(Annotation::place);
        all
    }
}

impl FromIterator<Annotation> for Notebook {
    fn from_iter<I: IntoIterator<Item = Annotation>>(annotations: I) -> Notebook {
        let mut notebook = Notebook::default();
        for annotation in annotations {
            notebook.apply(&Change::Put(annotation));
        }
        notebook
    }
}

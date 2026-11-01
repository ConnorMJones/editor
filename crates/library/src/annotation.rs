use std::borrow::Cow;
use std::cmp::Reverse;
use uuid::Uuid;

use crate::text::{Part, Place, TextSpan, Texts};

pub const QUOTE_CONTEXT: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AnnotationId(pub Uuid);

impl AnnotationId {
    pub fn new() -> AnnotationId {
        AnnotationId(Uuid::now_v7())
    }

    pub fn parse(text: &str) -> Option<AnnotationId> {
        Uuid::parse_str(text).ok().map(AnnotationId)
    }
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
    pub part: Part,
    pub quote: Quote,
    pub comment: Option<Comment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bookmark {
    pub id: AnnotationId,
    pub at: Place,
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

    pub fn line(&self) -> Cow<'_, str> {
        let mut lines = self.0.lines();
        let first = lines.next().unwrap_or_default();
        match lines.next() {
            Some(_) => Cow::Owned(format!("{first}\u{2026}")),
            None => Cow::Borrowed(first),
        }
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

    pub fn find_in(&self, texts: &dyn Texts, near: Part) -> Option<TextSpan> {
        let shared = |a: &mut dyn Iterator<Item = char>, b: &mut dyn Iterator<Item = char>| {
            a.zip(b).take_while(|(a, b)| a == b).count()
        };
        (0..texts.parts())
            .filter_map(|part| Some((Part(part), texts.part(Part(part))?)))
            .flat_map(|(part, first)| {
                let next = texts.part(Part(part.0 + 1));
                let split = first.text().len();
                let joined = match &next {
                    Some(next) => Cow::Owned(format!("{}\n{}", first.text(), next.text())),
                    None => Cow::Borrowed(first.text()),
                };
                (joined.match_indices(self.exact.as_str()))
                    .filter(|(start, _)| *start < split)
                    .map(|(start, exact)| {
                        let end = start + exact.len();
                        let context =
                            shared(
                                &mut joined[..start].chars().rev(),
                                &mut self.prefix.chars().rev(),
                            ) + shared(&mut joined[end..].chars(), &mut self.suffix.chars());
                        let from = Place {
                            part,
                            char: first.chars_in(start..end.min(split)).start,
                        };
                        let to = match (end.checked_sub(split + 1), &next) {
                            (Some(into), Some(next)) => Place {
                                part: Part(part.0 + 1),
                                char: next.chars_in(0..into).end,
                            },
                            (Some(_), None) | (None, _) => Place {
                                part,
                                char: first.chars_in(start..end.min(split)).end,
                            },
                        };
                        (
                            TextSpan::new(from, to),
                            context,
                            Reverse(part.0.abs_diff(near.0)),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .filter_map(|(span, context, distance)| Some((span?, context, distance)))
            .max_by_key(|&(_, context, distance)| (context, distance))
            .map(|(span, ..)| span)
    }
}

impl Highlight {
    pub fn of(texts: &dyn Texts, span: TextSpan) -> Option<Highlight> {
        let exact = span.text(texts)?;
        let first = texts.part(span.start().part)?;
        let before = &first.text()[..first.byte(span.start().char)];
        let from = (before.char_indices().rev().nth(QUOTE_CONTEXT - 1)).map_or(0, |(at, _)| at);
        let last = texts.part(span.end().part)?;
        let after = &last.text()[last.byte(span.end().char)..];
        let quote = Quote::new(
            exact,
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

    pub fn reanchor(self, texts: &dyn Texts) -> Annotation {
        let unmoved = self
            .span
            .text(texts)
            .is_some_and(|text| text == self.quote.exact);
        match (unmoved, self.quote.find_in(texts, self.span.start().part)) {
            (true, _) => Annotation::Highlight(self),
            (false, Some(span)) => Annotation::Highlight(Highlight { span, ..self }),
            (false, None) => Annotation::Lost(Lost {
                id: self.id,
                part: self.span.start().part,
                quote: self.quote,
                comment: self.comment,
            }),
        }
    }

    pub fn place(&self) -> Place {
        self.span.start()
    }
}

impl Lost {
    pub fn reanchor(self, texts: &dyn Texts) -> Annotation {
        match self.quote.find_in(texts, self.part) {
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

    pub fn place(&self) -> Place {
        match self {
            Annotation::Highlight(highlight) => highlight.place(),
            Annotation::Lost(Lost { part, .. }) => Place {
                part: *part,
                char: 0,
            },
            Annotation::Bookmark(Bookmark { at, .. }) => *at,
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
                let at = self.lost.partition_point(|kept| kept.part <= lost.part);
                self.lost.insert(at, lost.clone());
            }
            Change::Put(Annotation::Bookmark(bookmark)) => {
                let at = self
                    .bookmarks
                    .partition_point(|kept| kept.at <= bookmark.at);
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

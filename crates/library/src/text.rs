use std::cmp::Ordering;
use std::ops::Range;
use std::sync::Arc;

use reader::pdf::{Document, PageText};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Part(pub usize);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Place {
    pub part: Part,
    pub char: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TextSpan {
    start: Place, // private: start < end, so a span is never empty or backwards
    end: Place,
}

pub trait PartText {
    fn text(&self) -> &str;

    fn byte(&self, char: usize) -> usize;

    fn chars_in(&self, bytes: Range<usize>) -> Range<usize>;

    fn after(&self, char: usize) -> usize;

    fn len(&self) -> usize;

    fn slice(&self, chars: Range<usize>) -> &str {
        let start = self.byte(chars.start);
        self.text()[start..self.byte(chars.end).max(start)].trim_end()
    }

    fn around(&self, char: usize, within: fn(char) -> bool) -> Range<usize> {
        let text = self.text();
        let at = self.byte(char);
        let start = (text[..at].char_indices().rev())
            .take_while(|&(_, c)| within(c))
            .last()
            .map_or(at, |(byte, _)| byte);
        let end = (text[at..].char_indices())
            .find(|&(_, c)| !within(c))
            .map_or(text.len(), |(byte, _)| at + byte);
        self.chars_in(start..end)
    }
}

pub trait Texts: Send + Sync {
    fn parts(&self) -> usize;

    fn part(&self, part: Part) -> Option<Arc<dyn PartText + Send + Sync>>;
}

impl TextSpan {
    pub fn new(start: Place, end: Place) -> Option<TextSpan> {
        (start < end).then_some(TextSpan { start, end })
    }

    pub fn within(part: Part, chars: Range<usize>) -> Option<TextSpan> {
        TextSpan::new(
            Place {
                part,
                char: chars.start,
            },
            Place {
                part,
                char: chars.end,
            },
        )
    }

    pub fn covering(a: Place, b: Place, texts: &dyn Texts) -> Option<TextSpan> {
        let (from, to) = (a.min(b), a.max(b));
        let end = Place {
            part: to.part,
            char: texts.part(to.part)?.after(to.char),
        };
        TextSpan::new(from, end)
    }

    pub fn start(&self) -> Place {
        self.start
    }

    pub fn end(&self) -> Place {
        self.end
    }

    pub fn parts(&self) -> impl Iterator<Item = Part> {
        (self.start.part.0..=self.end.part.0).map(Part)
    }

    pub fn contains(&self, at: Place) -> bool {
        self.start <= at && at < self.end
    }

    pub fn chars_in(&self, part: Part, text: &dyn PartText) -> Option<Range<usize>> {
        let start = match part.cmp(&self.start.part) {
            Ordering::Less => return None,
            Ordering::Equal => self.start.char,
            Ordering::Greater => 0,
        };
        let end = match part.cmp(&self.end.part) {
            Ordering::Greater => return None,
            Ordering::Equal => self.end.char,
            Ordering::Less => text.len(),
        };
        (start < end).then_some(start..end)
    }

    pub fn text(&self, texts: &dyn Texts) -> Option<String> {
        let parts = (self.parts())
            .map(|part| {
                let text = texts.part(part)?;
                let chars = self.chars_in(part, &*text).unwrap_or_default();
                Some(text.slice(chars).to_owned())
            })
            .collect::<Option<Vec<String>>>()?;
        Some(parts.join("\n"))
    }
}

impl PartText for PageText {
    fn text(&self) -> &str {
        &self.text
    }

    fn byte(&self, char: usize) -> usize {
        PageText::byte(self, char)
    }

    fn chars_in(&self, bytes: Range<usize>) -> Range<usize> {
        PageText::chars_in(self, bytes)
    }

    fn after(&self, char: usize) -> usize {
        (char + 1).min(self.chars.len())
    }

    fn len(&self) -> usize {
        self.chars.len()
    }

    fn slice(&self, chars: Range<usize>) -> &str {
        PageText::slice(self, chars)
    }
}

impl Texts for Arc<Document> {
    fn parts(&self) -> usize {
        self.pages.sizes().len()
    }

    fn part(&self, part: Part) -> Option<Arc<dyn PartText + Send + Sync>> {
        let text: Arc<dyn PartText + Send + Sync> = self.read_text(part.0).ok()?;
        Some(text)
    }
}

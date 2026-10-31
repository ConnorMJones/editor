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
    pub part: Part,
    pub chars: Range<usize>,
}

pub trait PartText {
    fn text(&self) -> &str;

    fn byte(&self, char: usize) -> usize;

    fn chars_in(&self, bytes: Range<usize>) -> Range<usize>;

    fn after(&self, char: usize) -> usize;

    fn slice(&self, chars: Range<usize>) -> &str {
        let start = self.byte(chars.start);
        self.text()[start..self.byte(chars.end).max(start)].trim_end()
    }
}

pub trait Texts: Send + Sync {
    fn parts(&self) -> usize;

    fn part(&self, part: Part) -> Option<Arc<dyn PartText + Send + Sync>>;
}

impl TextSpan {
    pub fn start(&self) -> Place {
        Place {
            part: self.part,
            char: self.chars.start,
        }
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

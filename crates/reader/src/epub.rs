use std::collections::HashMap;
use std::fs::File;

use model::href::{Href, normalize, percent_decode};
use roxmltree::{Document as Xml, Node, ParsingOptions};

use crate::zip::{Archive, ZipError};

pub const CONTAINER: &str = "META-INF/container.xml";
pub const CONTENTS_DEPTH: usize = 16;

pub struct Book {
    pub title: String,
    pub authors: Vec<String>,
    pub chapters: Vec<Chapter>,
    pub contents: Vec<Contents>,
    pub styles: HashMap<Box<str>, String>,
    pub archive: Archive,
}

pub struct Chapter {
    pub path: Box<str>,
    pub xhtml: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contents {
    pub title: String,
    pub depth: usize,
    pub at: Spot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spot {
    pub chapter: usize,
    pub anchor: Option<Box<str>>,
}

#[derive(Debug, thiserror::Error)]
pub enum EpubError {
    #[error(transparent)]
    Zip(#[from] ZipError),
    #[error("{path}: {reason}")]
    Xml { path: String, reason: String },
    #[error("the container names no package")]
    NoPackage,
    #[error("the package lists no chapters")]
    NoChapters,
}

impl Book {
    pub fn open(file: File) -> Result<Book, EpubError> {
        let _span = tracing::info_span!("epub open").entered();
        let archive = Archive::open(file)?;
        let container = text_of(&archive, CONTAINER)?;
        let package_path = parse(CONTAINER, &container, |root| {
            (root.descendants())
                .find(|node| node.has_tag_name_local("rootfile"))
                .and_then(|node| node.attribute("full-path"))
                .map(str::to_owned)
        })?
        .ok_or(EpubError::NoPackage)?;
        let package = text_of(&archive, &package_path)?;
        let read = parse(&package_path, &package, |root| {
            let named = |name: &'static str| {
                (root.descendants())
                    .filter(move |node| node.has_tag_name_local(name))
                    .filter_map(|node| node.text().map(|text| text.trim().to_owned()))
                    .filter(|text| !text.is_empty())
            };
            let items: Vec<Node<'_, '_>> = (root.descendants())
                .filter(|node| node.has_tag_name_local("item"))
                .collect();
            let styles: Vec<String> = (items.iter())
                .filter(|node| node.attribute("media-type") == Some("text/css"))
                .filter_map(|node| resolve(&package_path, node.attribute("href")?))
                .collect();
            let manifest: HashMap<&str, String> = (items.iter())
                .filter_map(|node| {
                    Some((
                        node.attribute("id")?,
                        resolve(&package_path, node.attribute("href")?)?,
                    ))
                })
                .collect();
            let nav = (items.iter())
                .find(|node| {
                    (node
                        .attribute("properties")
                        .unwrap_or_default()
                        .split_whitespace())
                    .any(|property| property == "nav")
                })
                .and_then(|node| node.attribute("href"))
                .and_then(|href| resolve(&package_path, href));
            let spine = (root.descendants()).find(|node| node.has_tag_name_local("spine"));
            let chapters: Vec<String> = (spine.iter())
                .flat_map(|spine| spine.children())
                .filter(|node| node.has_tag_name_local("itemref"))
                .filter_map(|node| manifest.get(node.attribute("idref")?).cloned())
                .collect();
            let ncx = (spine.and_then(|spine| spine.attribute("toc")))
                .and_then(|id| manifest.get(id).cloned());
            Package {
                title: named("title").next().unwrap_or_default(),
                authors: named("creator").collect(),
                chapters,
                styles,
                navigation: match (nav, ncx) {
                    (Some(nav), _) => Navigation::Nav(nav),
                    (None, Some(ncx)) => Navigation::Ncx(ncx),
                    (None, None) => Navigation::None,
                },
            }
        })?;
        let chapters = (read.chapters.iter())
            .map(|path| {
                Ok(Chapter {
                    path: path.clone().into_boxed_str(),
                    xhtml: text_of(&archive, path)?,
                })
            })
            .collect::<Result<Vec<_>, EpubError>>()?;
        match chapters.as_slice() {
            [] => return Err(EpubError::NoChapters),
            [_, ..] => {}
        }
        let styles = (read.styles.iter())
            .map(|path| Ok((path.clone().into_boxed_str(), text_of(&archive, path)?)))
            .collect::<Result<HashMap<_, _>, EpubError>>()?;
        let index: HashMap<&str, usize> = (chapters.iter().enumerate())
            .map(|(at, chapter)| (&*chapter.path, at))
            .collect();
        let spot = |from: &str, href: &str| match Href::parse(href) {
            Href::Url(_) => None,
            Href::Local { path, anchor } => {
                let path = match path {
                    "" => from.to_owned(),
                    path => resolve(from, path)?,
                };
                Some(Spot {
                    chapter: *index.get(path.as_str())?,
                    anchor: anchor.map(|anchor| percent_decode(anchor).into_boxed_str()),
                })
            }
        };
        let contents = match &read.navigation {
            Navigation::Nav(path) => {
                let text = text_of(&archive, path)?;
                parse(path, &text, |root| {
                    let navs: Vec<Node<'_, '_>> = root
                        .descendants()
                        .filter(|node| node.has_tag_name_local("nav"))
                        .collect();
                    let toc = (navs.iter())
                        .find(|nav| {
                            (nav.attributes()).any(|attribute| {
                                attribute.name() == "type" && attribute.value().contains("toc")
                            })
                        })
                        .or(navs.first());
                    let mut contents = Vec::new();
                    let mut pending: Vec<(Node<'_, '_>, usize)> = (toc.iter())
                        .flat_map(|toc| toc.children().filter(|node| node.has_tag_name_local("ol")))
                        .flat_map(|list| {
                            list.children().filter(|node| node.has_tag_name_local("li"))
                        })
                        .map(|item| (item, 0))
                        .collect();
                    pending.reverse();
                    while let Some((item, depth)) = pending.pop() {
                        let link = (item.children()).find(|node| {
                            node.has_tag_name_local("a") || node.has_tag_name_local("span")
                        });
                        let title = link.map(words).unwrap_or_default();
                        let at = link
                            .and_then(|link| link.attribute("href"))
                            .and_then(|href| spot(path, href));
                        match at {
                            Some(at) => contents.push(Contents { title, depth, at }),
                            None => {}
                        }
                        let children: Vec<(Node<'_, '_>, usize)> = (item.children())
                            .filter(|node| node.has_tag_name_local("ol"))
                            .flat_map(|list| {
                                list.children().filter(|node| node.has_tag_name_local("li"))
                            })
                            .map(|child| (child, depth + 1))
                            .filter(|_| depth < CONTENTS_DEPTH)
                            .collect();
                        pending.extend(children.into_iter().rev());
                    }
                    contents
                })?
            }
            Navigation::Ncx(path) => {
                let text = text_of(&archive, path)?;
                parse(path, &text, |root| {
                    let mut contents = Vec::new();
                    let map = root
                        .descendants()
                        .find(|node| node.has_tag_name_local("navMap"));
                    let mut pending: Vec<(Node<'_, '_>, usize)> = (map.iter())
                        .flat_map(|map| {
                            map.children()
                                .filter(|node| node.has_tag_name_local("navPoint"))
                        })
                        .map(|point| (point, 0))
                        .collect();
                    pending.reverse();
                    while let Some((point, depth)) = pending.pop() {
                        let title = (point.children())
                            .find(|node| node.has_tag_name_local("navLabel"))
                            .map(words)
                            .unwrap_or_default();
                        let at = (point.children())
                            .find(|node| node.has_tag_name_local("content"))
                            .and_then(|content| content.attribute("src"))
                            .and_then(|src| spot(path, src));
                        match at {
                            Some(at) => contents.push(Contents { title, depth, at }),
                            None => {}
                        }
                        let children: Vec<(Node<'_, '_>, usize)> = (point.children())
                            .filter(|node| node.has_tag_name_local("navPoint"))
                            .map(|child| (child, depth + 1))
                            .filter(|_| depth < CONTENTS_DEPTH)
                            .collect();
                        pending.extend(children.into_iter().rev());
                    }
                    contents
                })?
            }
            Navigation::None => Vec::new(),
        };
        Ok(Book {
            title: read.title,
            authors: read.authors,
            chapters,
            contents,
            styles,
            archive,
        })
    }
}

struct Package {
    title: String,
    authors: Vec<String>,
    chapters: Vec<String>,
    styles: Vec<String>,
    navigation: Navigation,
}

enum Navigation {
    Nav(String),
    Ncx(String),
    None,
}

trait Local {
    fn has_tag_name_local(&self, name: &str) -> bool;
}

impl Local for Node<'_, '_> {
    fn has_tag_name_local(&self, name: &str) -> bool {
        self.is_element() && self.tag_name().name() == name
    }
}

pub fn words(node: Node<'_, '_>) -> String {
    let text: String = (node.descendants())
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .collect();
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn text_of(archive: &Archive, path: &str) -> Result<String, EpubError> {
    let bytes = archive.read(path)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

pub fn parse<T>(
    path: &str,
    text: &str,
    read: impl FnOnce(Node<'_, '_>) -> T,
) -> Result<T, EpubError> {
    let text = xml_entities(text);
    let options = ParsingOptions {
        allow_dtd: true,
        ..ParsingOptions::default()
    };
    let xml = Xml::parse_with_options(&text, options).map_err(|error| EpubError::Xml {
        path: path.to_owned(),
        reason: error.to_string(),
    })?;
    Ok(read(xml.root_element()))
}

pub fn resolve(from: &str, href: &str) -> Option<String> {
    let folder = from.rsplit_once('/').map_or("", |(folder, _)| folder);
    let decoded = percent_decode(href);
    normalize(folder, &decoded).map(|parts| parts.join("/"))
}

const HTML_ENTITIES: [(&str, char); 24] = [
    ("nbsp", '\u{a0}'),
    ("ensp", '\u{2002}'),
    ("emsp", '\u{2003}'),
    ("thinsp", '\u{2009}'),
    ("shy", '\u{ad}'),
    ("ndash", '\u{2013}'),
    ("mdash", '\u{2014}'),
    ("hellip", '\u{2026}'),
    ("lsquo", '\u{2018}'),
    ("rsquo", '\u{2019}'),
    ("ldquo", '\u{201c}'),
    ("rdquo", '\u{201d}'),
    ("laquo", '\u{ab}'),
    ("raquo", '\u{bb}'),
    ("copy", '\u{a9}'),
    ("reg", '\u{ae}'),
    ("trade", '\u{2122}'),
    ("deg", '\u{b0}'),
    ("times", '\u{d7}'),
    ("middot", '\u{b7}'),
    ("bull", '\u{2022}'),
    ("sect", '\u{a7}'),
    ("para", '\u{b6}'),
    ("dagger", '\u{2020}'),
];

const XML_ENTITIES: [&str; 5] = ["amp", "lt", "gt", "quot", "apos"];

pub fn xml_entities(text: &str) -> std::borrow::Cow<'_, str> {
    let named = |rest: &str| -> Option<(usize, char)> {
        let end = rest.find(';').filter(|end| *end <= 10)?;
        let name = &rest[1..end];
        match name {
            name if XML_ENTITIES.contains(&name) || name.starts_with('#') => None,
            name => {
                let found = HTML_ENTITIES.iter().find(|(known, _)| *known == name);
                Some((end + 1, found.map_or('\u{fffd}', |(_, char)| *char)))
            }
        }
    };
    match (text.match_indices('&')).find_map(|(at, _)| named(&text[at..]).map(|_| at)) {
        None => std::borrow::Cow::Borrowed(text),
        Some(_) => {
            let mut out = String::with_capacity(text.len());
            let mut rest = text;
            while let Some(at) = rest.find('&') {
                out.push_str(&rest[..at]);
                match named(&rest[at..]) {
                    Some((length, char)) => {
                        out.push(char);
                        rest = &rest[at + length..];
                    }
                    None => {
                        out.push('&');
                        rest = &rest[at + 1..];
                    }
                }
            }
            out.push_str(rest);
            std::borrow::Cow::Owned(out)
        }
    }
}

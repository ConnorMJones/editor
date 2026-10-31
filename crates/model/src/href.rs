#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Href<'a> {
    Url(&'a str),
    Local {
        path: &'a str,
        anchor: Option<&'a str>,
    },
}

impl<'a> Href<'a> {
    pub fn parse(text: &'a str) -> Href<'a> {
        match scheme(text) {
            Some(_) => Href::Url(text),
            None => match text.split_once('#') {
                Some((path, anchor)) => Href::Local {
                    path,
                    anchor: Some(anchor),
                },
                None => Href::Local {
                    path: text,
                    anchor: None,
                },
            },
        }
    }
}

pub fn scheme(text: &str) -> Option<&str> {
    let (scheme, _) = text.split_once(':')?;
    let mut chars = scheme.chars();
    let first = chars.next()?;
    let valid = first.is_ascii_alphabetic()
        && scheme.len() > 1
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'));
    valid.then_some(scheme)
}

pub fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = |byte: u8| (byte as char).to_digit(16);
        match (
            bytes[at],
            bytes.get(at + 1).copied().and_then(hex),
            bytes.get(at + 2).copied().and_then(hex),
        ) {
            (b'%', Some(high), Some(low)) => {
                out.push((high * 16 + low) as u8);
                at += 3;
            }
            (byte, _, _) => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_owned())
}

pub fn normalize<'a>(base: &'a str, rest: &'a str) -> Option<Vec<&'a str>> {
    let mut parts: Vec<&str> = base.split('/').filter(|part| !part.is_empty()).collect();
    for part in rest.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    Some(parts)
}

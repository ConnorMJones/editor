use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

use gpui::{App, RenderImage, Window};

pub struct Trimmed {
    pub kept: usize,
    pub dropped: usize,
}

pub fn keep_within<K: Clone + Eq + Hash, E>(
    entries: &mut HashMap<K, E>,
    budget: usize,
    age: impl Fn(&E) -> u64,
    image: impl Fn(&E) -> Option<&Arc<RenderImage>>,
    window: &mut Window,
    cx: &mut App,
) -> Trimmed {
    let bytes = |image: &RenderImage| {
        let size = image.size(0);
        size.width.0 as usize * size.height.0 as usize * 4
    };
    let mut ready: Vec<(u64, K, usize)> = (entries.iter())
        .filter_map(|(key, entry)| Some((age(entry), key.clone(), bytes(image(entry)?))))
        .collect();
    let mut kept: usize = ready.iter().map(|(_, _, bytes)| bytes).sum();
    ready.sort_by_key(|(age, ..)| std::cmp::Reverse(*age));
    let mut dropped = 0;
    for (_, key, bytes) in ready.into_iter().filter(|(age, ..)| *age > 0) {
        match kept.checked_sub(budget) {
            Some(1..) => {}
            Some(0) | None => break,
        }
        match entries.remove(&key) {
            Some(entry) => match image(&entry) {
                Some(image) => cx.drop_image(image.clone(), Some(window)),
                None => {}
            },
            None => {}
        }
        kept -= bytes;
        dropped += 1;
    }
    Trimmed { kept, dropped }
}

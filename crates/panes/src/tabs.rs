use std::cmp::Ordering;

use model::command::Direction;
use model::config::OpenAs;

pub trait Tabbed {
    type Key: Copy + Eq;

    fn key(&self) -> Self::Key;
    fn split_copy(&self) -> Self;
}

pub struct Pane<T> {
    pub tabs: Option<Tabs<T>>,
}

impl<T> Pane<T> {
    pub const EMPTY: Pane<T> = Pane { tabs: None };

    pub fn active(&self) -> Option<&T> {
        self.tabs.as_ref().map(|tabs| &tabs.items[tabs.active].tab)
    }

    pub fn active_mut(&mut self) -> Option<&mut T> {
        self.tabs
            .as_mut()
            .map(|tabs| &mut tabs.items[tabs.active].tab)
    }

    pub fn items(&self) -> &[Item<T>] {
        match &self.tabs {
            Some(tabs) => &tabs.items,
            None => &[],
        }
    }

    pub fn items_mut(&mut self) -> &mut [Item<T>] {
        match &mut self.tabs {
            Some(tabs) => &mut tabs.items,
            None => &mut [],
        }
    }
}

pub struct Tabs<T> {
    // private: the list is never empty, the indices point into it, and no two tabs show one file; a
    // direct change can show a tab that is gone or the same file twice in one pane.
    items: Vec<Item<T>>,
    active: usize,
    preview: Option<Preview>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preview {
    pub index: usize,
    pub kind: PreviewKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewKind {
    Open,
    SplitCopy,
}

pub struct Item<T> {
    pub tab: T,
    pub used: Tick,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tick(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum InvalidTabs {
    #[error("a tab list needs at least one tab")]
    Empty,
    #[error("active tab {0} does not exist")]
    Active(usize),
    #[error("preview tab {0} does not exist")]
    Preview(usize),
    #[error("tab {0} repeats an earlier tab")]
    Duplicate(usize),
}

impl<T: Tabbed> Tabs<T> {
    pub fn new(tab: T, open: OpenAs, now: Tick) -> Tabs<T> {
        Tabs {
            items: vec![Item { tab, used: now }],
            active: 0,
            preview: match open {
                OpenAs::Preview => Some(Preview {
                    index: 0,
                    kind: PreviewKind::Open,
                }),
                OpenAs::Permanent => None,
            },
        }
    }

    pub fn split_copy(tab: T, now: Tick) -> Tabs<T> {
        Tabs {
            items: vec![Item { tab, used: now }],
            active: 0,
            preview: Some(Preview {
                index: 0,
                kind: PreviewKind::SplitCopy,
            }),
        }
    }

    pub fn from_parts(
        items: Vec<Item<T>>,
        active: usize,
        preview: Option<usize>,
    ) -> Result<Tabs<T>, InvalidTabs> {
        let duplicate = items.iter().enumerate().find_map(|(index, item)| {
            let key = item.tab.key();
            items[..index]
                .iter()
                .any(|earlier| earlier.tab.key() == key)
                .then_some(index)
        });
        match (items.len(), duplicate, preview) {
            (0, _, _) => Err(InvalidTabs::Empty),
            (_, Some(index), _) => Err(InvalidTabs::Duplicate(index)),
            (len, None, _) if active >= len => Err(InvalidTabs::Active(active)),
            (len, None, Some(preview)) if preview >= len => Err(InvalidTabs::Preview(preview)),
            (_, None, _) => Ok(Tabs {
                items,
                active,
                preview: preview.map(|index| Preview {
                    index,
                    kind: PreviewKind::Open,
                }),
            }),
        }
    }

    pub fn items(&self) -> &[Item<T>] {
        &self.items
    }

    pub fn items_mut(&mut self) -> &mut [Item<T>] {
        &mut self.items
    }

    pub fn active(&self) -> usize {
        self.active
    }

    pub fn preview(&self) -> Option<usize> {
        self.preview.map(|preview| preview.index)
    }

    pub fn find(&self, key: T::Key) -> Option<usize> {
        self.items.iter().position(|item| item.tab.key() == key)
    }

    pub fn activate(&mut self, index: usize, now: Tick) {
        match self.items.get_mut(index) {
            Some(item) => {
                item.used = now;
                self.active = index;
            }
            None => {}
        }
    }

    pub fn keep(&mut self) {
        self.preview = None;
    }

    pub fn open(&mut self, tab: T, open: OpenAs, now: Tick) -> Option<Item<T>> {
        let item = Item { tab, used: now };
        match (self.find(item.tab.key()), open, self.preview) {
            (Some(index), OpenAs::Permanent, _) => {
                match self.preview.map(|preview| preview.index) == Some(index) {
                    true => self.preview = None,
                    false => {}
                }
                self.activate(index, now);
                None
            }
            (Some(index), OpenAs::Preview, _) => {
                self.activate(index, now);
                None
            }
            // A split's copy only stands in until the pane gets something, so any open replaces it.
            (
                None,
                OpenAs::Permanent,
                Some(Preview {
                    index,
                    kind: PreviewKind::SplitCopy,
                }),
            ) if index == self.active => {
                self.preview = None;
                Some(std::mem::replace(&mut self.items[index], item))
            }
            (None, OpenAs::Permanent, _) => {
                self.insert_after_active(item);
                None
            }
            (None, OpenAs::Preview, Some(Preview { index: preview, .. }))
                if preview == self.active =>
            {
                Some(std::mem::replace(&mut self.items[preview], item))
            }
            (None, OpenAs::Preview, Some(Preview { index: preview, .. })) => {
                let old = self.items.remove(preview);
                match preview < self.active {
                    true => self.active -= 1,
                    false => {}
                }
                self.preview = Some(Preview {
                    index: self.insert_after_active(item),
                    kind: PreviewKind::Open,
                });
                Some(old)
            }
            (None, OpenAs::Preview, None) => {
                self.preview = Some(Preview {
                    index: self.insert_after_active(item),
                    kind: PreviewKind::Open,
                });
                None
            }
        }
    }

    pub fn one(item: Item<T>) -> Tabs<T> {
        Tabs {
            items: vec![item],
            active: 0,
            preview: None,
        }
    }

    pub fn take(&mut self, item: Item<T>, at: Option<usize>, now: Tick) {
        match (self.find(item.tab.key()), at) {
            (Some(index), _) => self.activate(index, now),
            (None, None) => {
                self.insert_after_active(item);
            }
            (None, Some(at)) => self.rearrange(now, |items| {
                let at = at.min(items.len());
                items.insert(at, item);
                at
            }),
        }
    }

    pub fn shift(&mut self, direction: Direction, now: Tick) {
        let to = match direction {
            Direction::Forward => (self.active + 1).min(self.items.len() - 1),
            Direction::Backward => self.active.saturating_sub(1),
        };
        self.reorder(self.active, to, now)
    }

    pub fn last_used(&self) -> Option<usize> {
        (self.items.iter().enumerate())
            .filter(|(index, _)| *index != self.active)
            .max_by_key(|(_, item)| item.used)
            .map(|(index, _)| index)
    }

    pub fn reorder(&mut self, from: usize, to: usize, now: Tick) {
        self.rearrange(now, |items| {
            let item = items.remove(from);
            let to = to.min(items.len());
            items.insert(to, item);
            to
        })
    }

    fn rearrange(&mut self, now: Tick, change: impl FnOnce(&mut Vec<Item<T>>) -> usize) {
        let preview =
            (self.preview).map(|preview| (self.items[preview.index].tab.key(), preview.kind));
        let active = change(&mut self.items);
        self.preview =
            preview.and_then(|(key, kind)| self.find(key).map(|index| Preview { index, kind }));
        self.activate(active, now);
    }

    fn insert_after_active(&mut self, item: Item<T>) -> usize {
        let index = self.active + 1;
        self.items.insert(index, item);
        match &mut self.preview {
            Some(preview) if preview.index >= index => preview.index += 1,
            Some(_) | None => {}
        }
        self.active = index;
        index
    }

    pub fn remove(mut self, index: usize, now: Tick) -> (Option<Tabs<T>>, Item<T>) {
        let item = self.items.remove(index);
        self.preview = match self.preview {
            Some(preview) if preview.index == index => None,
            Some(preview) if preview.index > index => Some(Preview {
                index: preview.index - 1,
                ..preview
            }),
            other => other,
        };
        let recent = self
            .items
            .iter()
            .enumerate()
            .max_by_key(|(_, item)| item.used)
            .map(|(index, _)| index);
        let active = match (self.active.cmp(&index), recent) {
            (_, None) => return (None, item),
            (Ordering::Less, Some(_)) => self.active,
            (Ordering::Greater, Some(_)) => self.active - 1,
            (Ordering::Equal, Some(recent)) => recent,
        };
        self.activate(active, now);
        (Some(self), item)
    }
}

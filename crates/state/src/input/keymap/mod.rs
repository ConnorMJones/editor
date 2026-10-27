use model::command::Command;
use model::key::Key;

mod docs;
mod tables;

pub use docs::reference;
use tables::TABLES;

/// Which binding table applies; derived from mode, overlay and focus each key press.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Context {
    Normal,
    Insert,
    Select,
    Prompt,
    Tree,
    TreeInput,
    Picker,
    Changes,
    Confirm,
    Document,
    Pages,
    Drawing,
    ImageViewer,
    DrawingText,
    PdfRead,
    PdfMarkup,
}

impl Context {
    pub const COUNT: usize = 16;
    pub const ALL: [Context; Context::COUNT] = [
        Context::Normal,
        Context::Insert,
        Context::Select,
        Context::Prompt,
        Context::Tree,
        Context::TreeInput,
        Context::Picker,
        Context::Changes,
        Context::Confirm,
        Context::Document,
        Context::Pages,
        Context::Drawing,
        Context::ImageViewer,
        Context::DrawingText,
        Context::PdfRead,
        Context::PdfMarkup,
    ];
}

pub enum Trie {
    Leaf(Command),
    Node(Node),
}

/// `keys` holds `Key::packed` values sorted for binary search; `children` is parallel to it.
#[derive(Default)]
pub struct Node {
    pub keys: Vec<u32>,
    pub children: Vec<Trie>,
}

pub enum Resolved<'a> {
    Command(&'a Command),
    Pending,
    Unbound,
}

pub struct Keymap {
    pub tries: [Trie; Context::COUNT],
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum KeymapError {
    #[error("{context:?}: binding {keys:?} conflicts with an existing prefix or leaf")]
    Conflict { context: Context, keys: Vec<Key> },
    #[error("{context:?}: empty key sequence")]
    Empty { context: Context },
}

type Bindings = &'static [(&'static [Key], Command)];

fn tables(context: Context) -> &'static [Bindings] {
    TABLES[context as usize]
}

/// First binding of `command` in `context`, for display.
pub fn keys_for(context: Context, command: &Command) -> Option<&'static [Key]> {
    tables(context)
        .iter()
        .copied()
        .flatten()
        .find(|(_, bound)| bound == command)
        .map(|(keys, _)| *keys)
}

fn insert(node: &mut Node, keys: &[Key], command: &Command) -> Result<(), ()> {
    match keys {
        [] => Err(()),
        [key, rest @ ..] => match (node.keys.binary_search(&key.packed()), rest) {
            (Err(slot), []) => {
                node.keys.insert(slot, key.packed());
                node.children.insert(slot, Trie::Leaf(command.clone()));
                Ok(())
            }
            (Err(slot), _) => {
                let mut child = Node::default();
                insert(&mut child, rest, command)?;
                node.keys.insert(slot, key.packed());
                node.children.insert(slot, Trie::Node(child));
                Ok(())
            }
            (Ok(slot), _) => match (&mut node.children[slot], rest) {
                (Trie::Node(child), [_, ..]) => insert(child, rest, command),
                _ => Err(()),
            },
        },
    }
}

fn build_trie(context: Context) -> Result<Trie, KeymapError> {
    let mut root = Node::default();
    for (keys, command) in tables(context).iter().flat_map(|table| table.iter()) {
        match keys.is_empty() {
            true => return Err(KeymapError::Empty { context }),
            false => insert(&mut root, keys, command).map_err(|()| KeymapError::Conflict {
                context,
                keys: keys.to_vec(),
            })?,
        }
    }
    Ok(Trie::Node(root))
}

impl Keymap {
    pub fn helix() -> Result<Keymap, KeymapError> {
        let [
            normal,
            insert,
            select,
            prompt,
            tree,
            tree_input,
            picker,
            changes,
            confirm,
            document,
            pages,
            drawing,
            image_viewer,
            drawing_text,
            pdf_read,
            pdf_markup,
        ] = Context::ALL;
        Ok(Keymap {
            tries: [
                build_trie(normal)?,
                build_trie(insert)?,
                build_trie(select)?,
                build_trie(prompt)?,
                build_trie(tree)?,
                build_trie(tree_input)?,
                build_trie(picker)?,
                build_trie(changes)?,
                build_trie(confirm)?,
                build_trie(document)?,
                build_trie(pages)?,
                build_trie(drawing)?,
                build_trie(image_viewer)?,
                build_trie(drawing_text)?,
                build_trie(pdf_read)?,
                build_trie(pdf_markup)?,
            ],
        })
    }

    pub fn resolve(&self, context: Context, keys: &[Key]) -> Resolved<'_> {
        let mut trie = &self.tries[context as usize];
        for key in keys {
            trie = match trie {
                Trie::Node(node) => match node.keys.binary_search(&key.packed()) {
                    Ok(i) => &node.children[i],
                    Err(_) => return Resolved::Unbound,
                },
                Trie::Leaf(_) => return Resolved::Unbound,
            };
        }
        match trie {
            Trie::Leaf(command) => Resolved::Command(command),
            Trie::Node(_) => Resolved::Pending,
        }
    }
}

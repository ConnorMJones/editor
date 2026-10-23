use std::sync::Arc;

use crate::doc::Marks;

/// A drawing's layers, bottom first; never empty.
#[derive(Clone, Debug, PartialEq)]
pub struct Layers(Vec<Layer>);

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub name: LayerName,
    pub visibility: Visibility,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    Shown,
    Hidden,
}

/// A layer's position in its drawing's list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LayerIndex(pub usize);

/// One line of text that isn't blank.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerName(String);

#[derive(Debug, thiserror::Error, PartialEq)]
#[error("layer name {0:?} is blank or spans lines")]
pub struct InvalidLayerName(pub String);

impl LayerName {
    pub fn new(name: String) -> Result<LayerName, InvalidLayerName> {
        match !name.trim().is_empty() && !name.contains('\n') {
            true => Ok(LayerName(name)),
            false => Err(InvalidLayerName(name)),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Layers {
    /// One shown layer called "Layer 1".
    pub fn new() -> Layers {
        Layers(vec![Layer {
            name: LayerName("Layer 1".to_owned()),
            visibility: Visibility::Shown,
        }])
    }

    /// `None` for an empty list.
    pub fn from_list(list: Vec<Layer>) -> Option<Layers> {
        (!list.is_empty()).then_some(Layers(list))
    }

    pub fn get(&self, index: LayerIndex) -> Option<&Layer> {
        self.0.get(index.0)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Bottom first.
    pub fn iter(&self) -> impl Iterator<Item = (LayerIndex, &Layer)> {
        self.0
            .iter()
            .enumerate()
            .map(|(index, layer)| (LayerIndex(index), layer))
    }

    pub fn push(&mut self, layer: Layer) -> LayerIndex {
        self.0.push(layer);
        LayerIndex(self.0.len() - 1)
    }

    pub fn get_mut(&mut self, index: LayerIndex) -> Option<&mut Layer> {
        self.0.get_mut(index.0)
    }
}

impl Default for Layers {
    fn default() -> Layers {
        Layers::new()
    }
}

/// A page's or board's marks, one set per layer of its drawing; the drawing keeps the count equal
/// to its layers'.
#[derive(Clone, Debug)]
pub struct LayerMarks(Vec<Arc<Marks>>);

impl LayerMarks {
    pub fn empty(layers: &Layers) -> LayerMarks {
        LayerMarks(vec![Arc::default(); layers.len()])
    }

    /// One set of marks for each of `layers`, from `each`.
    pub fn build<E>(
        layers: &Layers,
        mut each: impl FnMut(LayerIndex) -> Result<Marks, E>,
    ) -> Result<LayerMarks, E> {
        (layers.iter())
            .map(|(index, _)| each(index).map(Arc::new))
            .collect::<Result<Vec<_>, E>>()
            .map(LayerMarks)
    }

    pub fn get(&self, layer: LayerIndex) -> Option<&Marks> {
        self.0.get(layer.0).map(Arc::as_ref)
    }

    pub fn get_mut(&mut self, layer: LayerIndex) -> Option<&mut Arc<Marks>> {
        self.0.get_mut(layer.0)
    }

    pub fn push_layer(&mut self) {
        self.0.push(Arc::default());
    }

    /// The marks of shown layers, bottom first.
    pub fn shown<'a>(&'a self, layers: &'a Layers) -> impl Iterator<Item = &'a Marks> + 'a {
        layers
            .iter()
            .zip(&self.0)
            .filter(|((_, layer), _)| layer.visibility == Visibility::Shown)
            .map(|(_, marks)| marks.as_ref())
    }

    pub fn iter(&self) -> impl Iterator<Item = &Marks> {
        self.0.iter().map(Arc::as_ref)
    }
}

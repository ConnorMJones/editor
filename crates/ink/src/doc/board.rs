use crate::doc::{
    Background, ChangeError, Drawing, LayerChange, LayerIndex, LayerMarks, Layers, MarkChange,
    Marks, apply_marks,
};

/// One unbounded surface; on a board the sheet and the surface are the same space.
#[derive(Clone, Debug)]
pub struct Board {
    pub background: Background,
    layers: Layers,    // private: one set of marks per layer
    marks: LayerMarks, // private: one set of marks per layer
}

#[derive(Clone, Debug)]
pub enum BoardChange {
    Marks(MarkChange<LayerIndex>),
    Layers(LayerChange),
}

impl From<MarkChange<LayerIndex>> for BoardChange {
    fn from(change: MarkChange<LayerIndex>) -> BoardChange {
        BoardChange::Marks(change)
    }
}

impl From<LayerChange> for BoardChange {
    fn from(change: LayerChange) -> BoardChange {
        BoardChange::Layers(change)
    }
}

impl Board {
    pub fn new(background: Background) -> Board {
        let layers = Layers::new();
        Board {
            background,
            marks: LayerMarks::empty(&layers),
            layers,
        }
    }

    /// A board whose marks are read with `marks` once per layer.
    pub fn build<R, E>(
        background: Background,
        layers: Layers,
        source: &mut R,
        marks: fn(&mut R, LayerIndex) -> Result<Marks, E>,
    ) -> Result<Board, E> {
        Ok(Board {
            background,
            marks: LayerMarks::build(&layers, |layer| marks(source, layer))?,
            layers,
        })
    }

    pub fn layer_marks(&self) -> &LayerMarks {
        &self.marks
    }
}

impl Drawing for Board {
    type Target = LayerIndex;
    type Surface = ();
    type Change = BoardChange;

    fn target((): (), layer: LayerIndex) -> LayerIndex {
        layer
    }

    fn surface(_: LayerIndex) {}

    fn layers(&self) -> &Layers {
        &self.layers
    }

    fn marks(&self, target: LayerIndex) -> Option<&Marks> {
        self.marks.get(target)
    }

    fn apply(&mut self, change: BoardChange) -> Result<(), ChangeError> {
        match change {
            BoardChange::Marks(change) => {
                let target = change.target();
                let marks = self
                    .marks
                    .get_mut(target)
                    .ok_or(ChangeError::NoLayer(target.0))?;
                apply_marks(marks, change)
            }
            BoardChange::Layers(LayerChange::Add(layer)) => {
                self.layers.push(layer);
                self.marks.push_layer();
                Ok(())
            }
            BoardChange::Layers(LayerChange::Set(index, layer)) => {
                *self
                    .layers
                    .get_mut(index)
                    .ok_or(ChangeError::NoLayer(index.0))? = layer;
                Ok(())
            }
        }
    }
}

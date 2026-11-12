use helix_core::Selection;

use crate::render::document::{DocumentSeen, DocumentState};
use crate::render::pages::{PagesSeen, PagesState};
use crate::text::buffer::BufferId;
use crate::view::tab::{BesideMut, Opened, Showing, ShowingMut, Tab, TextTabs};
use crate::view::text_view::TextView;

pub use panes::{
    Child, CloseError, DropOn, Flex, InvalidFlex, InvalidTabs, LayoutError, MIN_SHARE, Node,
    NotInLayout, PaneId, PanePoint, Place, Preview, PreviewKind, Rect, Split, SplitError,
    SplitPath, Tick, Zone, child_rects,
};

pub type Layout = panes::Layout<Tab>;
pub type Pane = panes::Pane<Tab>;
pub type Tabs = panes::Tabs<Tab>;
pub type Item = panes::Item<Tab>;

pub trait TabLayout {
    fn tab_of(&mut self, pane: PaneId, buffer: BufferId) -> Option<&mut TextTabs>;
    fn selection_of(&self, buffer: BufferId) -> Selection;
    fn document(&self, pane: PaneId) -> Option<(&TextView, &DocumentState)>;
    fn document_mut(
        &mut self,
        pane: PaneId,
    ) -> Option<(
        &mut TextView,
        &mut DocumentState,
        BesideMut<'_, DocumentSeen>,
    )>;
    fn pages(&self, pane: PaneId) -> Option<(&TextView, &PagesState)>;
    fn pages_mut(
        &mut self,
        pane: PaneId,
    ) -> Option<(&mut TextView, &mut PagesState, BesideMut<'_, PagesSeen>)>;
}

pub trait TabPane {
    fn active_text(&self) -> Option<&TextTabs>;
    fn active_text_mut(&mut self) -> Option<&mut TextTabs>;
}

impl TabPane for Pane {
    fn active_text(&self) -> Option<&TextTabs> {
        self.active()?.as_text()
    }

    fn active_text_mut(&mut self) -> Option<&mut TextTabs> {
        self.active_mut()?.as_text_mut()
    }
}

impl TabLayout for Layout {
    fn tab_of(&mut self, pane: PaneId, buffer: BufferId) -> Option<&mut TextTabs> {
        let pane = self.find_pane(pane)?;
        let index = pane.tabs.as_ref()?.find(Opened::Text(buffer))?;
        pane.items_mut()[index].tab.as_text_mut()
    }

    fn selection_of(&self, buffer: BufferId) -> Selection {
        (self.panes().values())
            .flat_map(Pane::items)
            .filter_map(|item| item.tab.as_text().map(TextTabs::text))
            .find(|view| view.buffer() == buffer)
            .map_or_else(|| Selection::point(0), |view| view.selection.clone())
    }

    fn document(&self, pane: PaneId) -> Option<(&TextView, &DocumentState)> {
        match self.panes().get(pane)?.active_text()?.showing() {
            Showing::Document(text, state, _) => Some((text, state)),
            Showing::Source(_) | Showing::Diff(..) | Showing::Pages(..) => None,
        }
    }

    fn document_mut(
        &mut self,
        pane: PaneId,
    ) -> Option<(
        &mut TextView,
        &mut DocumentState,
        BesideMut<'_, DocumentSeen>,
    )> {
        match self.find_pane(pane)?.active_text_mut()?.showing_mut() {
            ShowingMut::Document(text, state, beside) => Some((text, state, beside)),
            ShowingMut::Source(_) | ShowingMut::Diff(..) | ShowingMut::Pages(..) => None,
        }
    }

    fn pages(&self, pane: PaneId) -> Option<(&TextView, &PagesState)> {
        match self.panes().get(pane)?.active_text()?.showing() {
            Showing::Pages(text, state, _) => Some((text, state)),
            Showing::Source(_) | Showing::Diff(..) | Showing::Document(..) => None,
        }
    }

    fn pages_mut(
        &mut self,
        pane: PaneId,
    ) -> Option<(&mut TextView, &mut PagesState, BesideMut<'_, PagesSeen>)> {
        match self.find_pane(pane)?.active_text_mut()?.showing_mut() {
            ShowingMut::Pages(text, state, beside) => Some((text, state, beside)),
            ShowingMut::Source(_) | ShowingMut::Diff(..) | ShowingMut::Document(..) => None,
        }
    }
}

use model::geom::{Point, Vector};
use model::sheet::{
    Fit, PageIndex, PageSize, PlaneView, SheetView, Stack, StackEdit, Viewport, Zoom,
};
use model::units::Px;

fn stack(pages: usize) -> Stack {
    Stack::new(vec![PageSize::A4; pages]).unwrap()
}

fn viewport(width: f32, height: f32) -> Viewport {
    Viewport::new(Px(width), Px(height)).unwrap()
}

/// The pane rectangle the first page covers, as (left, right).
fn page_columns(view: &SheetView, stack: &Stack, viewport: Viewport) -> (f32, f32) {
    let frame = view.frame(stack, viewport);
    let page = frame.page(stack, PageIndex(0));
    let left = page.to_pane.apply(Point::new(0.0, 0.0)).x;
    let right = page
        .to_pane
        .apply(Point::new(PageSize::A4.width.get(), 0.0))
        .x;
    (left, right)
}

#[test]
fn fitting_the_width_fills_the_pane_edge_to_edge() {
    let stack = stack(3);
    let view = SheetView::start(&stack);
    for (width, height) in [(800.0, 600.0), (300.0, 900.0), (1900.0, 400.0)] {
        let (left, right) = page_columns(&view, &stack, viewport(width, height));
        assert!(left.abs() < 1e-2, "{width}: {left}");
        assert!((right - width).abs() < 1e-2, "{width}: {right}");
    }
}

#[test]
fn fitting_the_height_fills_the_pane_top_to_bottom() {
    let stack = stack(3);
    let mut view = SheetView::start(&stack);
    view.fit = Fit::Height;
    let pane = viewport(1200.0, 600.0);
    view.go_to(&stack, PageIndex(1), Some(pane));
    let frame = view.frame(&stack, pane);
    let page = frame.page(&stack, PageIndex(1));
    let top = page.to_pane.apply(Point::new(0.0, 0.0));
    let bottom = page
        .to_pane
        .apply(Point::new(0.0, PageSize::A4.height.get()));
    assert!(top.y.abs() < 1e-2 && (bottom.y - 600.0).abs() < 1e-2);
    let middle = page
        .to_pane
        .apply(Point::new(PageSize::A4.width.get() * 0.5, 0.0));
    assert!(
        (middle.x - 600.0).abs() < 1e-2,
        "centred across: {middle:?}"
    );
}

#[test]
fn panning_never_leaves_the_pages() {
    let stack = stack(2);
    let mut view = SheetView::start(&stack);
    let pane = viewport(800.0, 600.0);
    for by in [Vector::new(0.0, -1e6), Vector::new(1e6, 0.0)] {
        view.pan(&stack, pane, by);
        let (left, right) = page_columns(&view, &stack, pane);
        assert!(
            left.abs() < 1e-2 && (right - 800.0).abs() < 1e-2,
            "a fitted width can't pan sideways"
        );
    }
    let frame = view.frame(&stack, pane);
    let top = frame
        .page(&stack, PageIndex(0))
        .to_pane
        .apply(Point::new(0.0, 0.0));
    assert!(top.y.abs() < 1e-2, "stops at the first page's top");
    view.pan(&stack, pane, Vector::new(0.0, 1e6));
    let frame = view.frame(&stack, pane);
    let last = frame.page(&stack, PageIndex(1));
    let bottom = last
        .to_pane
        .apply(Point::new(0.0, PageSize::A4.height.get()));
    assert!((bottom.y - 600.0).abs() < 1e-2);
    assert_eq!(view.centre.page, PageIndex(1), "the page in focus follows");
}

#[test]
fn a_resize_keeps_the_same_page_point_at_the_centre() {
    let stack = stack(5);
    let mut view = SheetView::start(&stack);
    view.go_to(&stack, PageIndex(2), Some(viewport(800.0, 600.0)));
    view.zoom = Zoom::ONE.by(2.0);
    let before = view.centre;
    for pane in [viewport(400.0, 600.0), viewport(1600.0, 300.0)] {
        let frame = view.frame(&stack, pane);
        let middle = frame
            .from_pane
            .apply(Point::new(pane.width.get() * 0.5, pane.height.get() * 0.5));
        let page = stack.place(PageIndex(2)).place.apply(before.at);
        assert!(middle.distance(page) < 1e-2, "{middle:?} {page:?}");
    }
}

#[test]
fn zooming_about_a_point_keeps_it_under_the_cursor() {
    let stack = stack(3);
    let mut view = SheetView::start(&stack);
    view.go_to(&stack, PageIndex(1), None);
    let pane = viewport(800.0, 600.0);
    let at = Point::new(200.0, 150.0);
    let under = view.frame(&stack, pane).from_pane.apply(at);
    view.zoom_about(&stack, pane, at, 3.0);
    let after = view.frame(&stack, pane).to_pane.apply(under);
    assert!(after.distance(at) < 1e-2, "{after:?}");
}

#[test]
fn views_stay_on_their_page_as_pages_come_and_go() {
    let mut stack = stack(3);
    let mut view = SheetView::start(&stack);
    view.go_to(&stack, PageIndex(1), None);
    view.follow(stack.insert(PageIndex(0), PageSize::A4).unwrap());
    assert_eq!(view.centre.page, PageIndex(2));
    view.follow(stack.remove(PageIndex(3)).unwrap());
    assert_eq!(view.centre.page, PageIndex(2));
    view.follow(stack.remove(PageIndex(0)).unwrap());
    assert_eq!(view.centre.page, PageIndex(1));
    assert_eq!(
        stack.insert(PageIndex(9), PageSize::A4),
        Err(model::sheet::StackError::NoPage(9))
    );
    let mut one = Stack::one(PageSize::A4);
    assert_eq!(
        one.remove(PageIndex(0)),
        Err(model::sheet::StackError::LastPage)
    );
    assert!(matches!(
        stack.insert(PageIndex(2), PageSize::A4),
        Ok(StackEdit::Inserted(PageIndex(2)))
    ));
}

#[test]
fn a_plane_zooms_about_the_cursor() {
    let mut view = PlaneView::START;
    let pane = viewport(800.0, 600.0);
    let at = Point::new(700.0, 100.0);
    let under = view.frame(pane).from_pane.apply(at);
    view.zoom_about(pane, at, 0.5);
    assert!(view.frame(pane).to_pane.apply(under).distance(at) < 1e-3);
}

#[test]
fn stepping_the_fit_never_shows_the_same_scale_twice() {
    let stack = stack(2);
    let mut view = SheetView::start(&stack);
    let wide = viewport(1200.0, 600.0);
    view.step_fit(&stack, Some(wide));
    assert_eq!(view.fit, Fit::Height);
    view.step_fit(&stack, Some(wide));
    assert_eq!(view.fit, Fit::Width);
    // The whole page fills the height of a wide pane and the width of a tall one.
    for (pane, after) in [(wide, Fit::Width), (viewport(300.0, 1200.0), Fit::Height)] {
        view.fit = Fit::Page;
        view.step_fit(&stack, Some(pane));
        assert_eq!(view.fit, after);
    }
    view.fit = Fit::Page;
    view.step_fit(&stack, None);
    assert_eq!(view.fit, Fit::Width, "unmeasured, the width");
}

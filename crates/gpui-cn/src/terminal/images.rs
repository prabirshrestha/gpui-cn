//! Kitty graphics images on the grid: where a placement goes and the GPUI
//! images it is drawn from.
//!
//! The engine sends each image's pixels once, as RGBA cut to the placed
//! part. The cache turns them into a GPUI image on first use and keeps it
//! while a frame places it, so a scroll or a cursor blink uploads nothing.
//! An image no frame places any more leaves the sprite atlas.

use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::{Bounds, Pixels, RenderImage, Window, point, size};

use crate::terminal::frame::{ImageLayer, ImagePlacement, TerminalFrame, TerminalImage};
use crate::terminal::geometry::Geometry;

/// One image ready to paint.
#[derive(Clone)]
pub(crate) struct PaintedImage {
    pub layer: ImageLayer,
    /// The whole drawn image, which can reach past the grid.
    pub bounds: Bounds<Pixels>,
    pub image: Arc<RenderImage>,
}

/// The GPUI images of the images the last frame placed, by image key.
#[derive(Default)]
pub(crate) struct ImageCache {
    images: HashMap<u64, Arc<RenderImage>>,
    /// What the last prepaint placed, kept for tests.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) placed: Vec<(ImageLayer, Bounds<Pixels>)>,
}

impl ImageCache {
    /// The images of `frame` placed on `geometry`, in drawing order. Images
    /// the frame no longer places are removed from the window's atlas.
    pub(crate) fn prepare(
        &mut self,
        frame: &TerminalFrame,
        geometry: Geometry,
        window: &mut Window,
    ) -> Vec<PaintedImage> {
        let mut used = HashMap::with_capacity(frame.images.len());
        let mut painted = Vec::with_capacity(frame.images.len());
        for placement in frame.images.iter() {
            let Some(bounds) = placement_bounds(placement, frame, geometry) else {
                continue;
            };
            let key = placement.image.key();
            let image = match used.get(&key).or_else(|| self.images.get(&key)) {
                Some(image) => Arc::clone(image),
                None => match render_image(&placement.image) {
                    Some(image) => Arc::new(image),
                    None => continue,
                },
            };
            used.insert(key, Arc::clone(&image));
            painted.push(PaintedImage {
                layer: placement.layer,
                bounds,
                image,
            });
        }
        for (key, image) in self.images.drain() {
            if !used.contains_key(&key) {
                let _ = window.drop_image(image);
            }
        }
        self.images = used;
        self.placed = painted.iter().map(|p| (p.layer, p.bounds)).collect();
        painted
    }
}

/// Where `placement` is drawn on `geometry`, or `None` when it has no area
/// or does not touch the grid. The frame's offsets and sizes are device
/// pixels of its viewport cell, scaled here to the element's cell.
pub(crate) fn placement_bounds(
    placement: &ImagePlacement,
    frame: &TerminalFrame,
    geometry: Geometry,
) -> Option<Bounds<Pixels>> {
    if placement.width == 0 || placement.height == 0 {
        return None;
    }
    #[allow(clippy::cast_precision_loss)]
    let (device_width, device_height) = (
        frame.viewport.cell_width() as f32,
        frame.viewport.cell_height() as f32,
    );
    let x_scale = f32::from(geometry.cell.width) / device_width;
    let y_scale = f32::from(geometry.cell.height) / device_height;
    #[allow(clippy::cast_precision_loss)]
    let bounds = Bounds::new(
        geometry.origin
            + point(
                geometry.cell.width * placement.column as f32
                    + Pixels::from(placement.offset_x as f32 * x_scale),
                geometry.cell.height * placement.row as f32
                    + Pixels::from(placement.offset_y as f32 * y_scale),
            ),
        size(
            Pixels::from(placement.width as f32 * x_scale),
            Pixels::from(placement.height as f32 * y_scale),
        ),
    );
    let visible = bounds.intersect(&geometry.grid_bounds());
    (visible.size.width > Pixels::ZERO && visible.size.height > Pixels::ZERO).then_some(bounds)
}

/// A GPUI image of `image`, whose sprite atlas wants BGRA.
fn render_image(image: &TerminalImage) -> Option<RenderImage> {
    let mut bgra = image.rgba().to_vec();
    for pixel in bgra.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    let buffer = image::RgbaImage::from_raw(image.width(), image.height(), bgra)?;
    Some(RenderImage::new([image::Frame::new(buffer)]))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{AppContext as _, Bounds, TestAppContext, point, px, size};

    use super::{ImageCache, placement_bounds};
    use crate::terminal::frame::{
        ImageLayer, ImagePlacement, TerminalFrame, TerminalImage, Viewport,
    };
    use crate::terminal::geometry::Geometry;
    use crate::terminal::{FixtureSource, Terminal, TerminalConfig, TerminalState};

    fn setup() -> (TerminalFrame, Geometry, Arc<TerminalImage>) {
        // 16x32 device pixel cells at scale 2, so 8x16 logical cells.
        let viewport = Viewport::new(10, 5, 16, 32).unwrap();
        let frame = TerminalFrame::new(viewport, Vec::new());
        let geometry = Geometry {
            bounds: Bounds::new(point(px(0.), px(0.)), size(px(200.), px(200.))),
            origin: point(px(4.), px(6.)),
            cell: size(px(8.), px(16.)),
            columns: 10,
            rows: 5,
        };
        let image = Arc::new(TerminalImage::new(1, 1, vec![0, 0, 0, 255]).unwrap());
        (frame, geometry, image)
    }

    #[test]
    fn a_placement_lands_on_its_cell_at_the_logical_size() {
        let (frame, geometry, image) = setup();
        let placement = ImagePlacement::new(image, 2, 1)
            .with_offset(4, 8)
            .with_size(32, 64);
        let bounds = placement_bounds(&placement, &frame, geometry).unwrap();
        assert_eq!(bounds.origin, point(px(4. + 16. + 2.), px(6. + 16. + 4.)));
        assert_eq!(bounds.size, size(px(16.), px(32.)));
    }

    #[test]
    fn a_placement_partly_above_the_grid_keeps_its_full_size() {
        let (frame, geometry, image) = setup();
        let placement = ImagePlacement::new(image, 0, -1).with_size(16, 64);
        let bounds = placement_bounds(&placement, &frame, geometry).unwrap();
        assert_eq!(bounds.origin.y, px(6. - 16.));
        assert_eq!(bounds.size.height, px(32.));
    }

    struct Harness(gpui_kit::Entity<TerminalState>);

    impl gpui_kit::Render for Harness {
        fn render(
            &mut self,
            _: &mut gpui_kit::Window,
            _: &mut gpui_kit::Context<Self>,
        ) -> impl gpui_kit::IntoElement {
            use gpui_kit::{ParentElement as _, Styled as _};
            gpui_kit::div()
                .size_full()
                .child(Terminal::new("terminal", &self.0))
        }
    }

    /// A terminal in a window showing `bytes`, after its first frames.
    fn open(
        bytes: Vec<u8>,
        cx: &mut TestAppContext,
    ) -> (gpui_kit::AnyWindowHandle, gpui_kit::Entity<TerminalState>) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::init(cx);
        });
        let state = std::rc::Rc::new(std::cell::RefCell::new(None));
        let handle = cx.open_window(size(px(400.), px(200.)), {
            let state = state.clone();
            move |window, cx| {
                let terminal = cx.new(|cx| {
                    TerminalState::new(
                        FixtureSource::new(bytes),
                        TerminalConfig::default(),
                        window,
                        cx,
                    )
                });
                state.replace(Some(terminal.clone()));
                let harness = cx.new(|cx| {
                    cx.observe(&terminal, |_, _, cx| cx.notify()).detach();
                    Harness(terminal)
                });
                gpui_kit::base::Root::new(harness, window, cx)
            }
        });
        for _ in 0..3 {
            cx.run_until_parked();
            cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
                .unwrap();
        }
        let state = state.borrow_mut().take().unwrap();
        (handle.into(), state)
    }

    #[gpui_kit::test]
    fn the_element_paints_a_kitty_image_over_its_cells(cx: &mut TestAppContext) {
        let mut bytes = b"ab".to_vec();
        bytes.extend_from_slice(
            b"\x1b_Ga=T,q=2,f=32,s=2,v=2,c=2,r=1;/wAA/wD/AP8AAP//////gA==\x1b\\",
        );
        let (_, state) = open(bytes, cx);
        state.read_with(cx, |state, _| {
            assert_eq!(state.frame().images.len(), 1);
            let geometry = state.geometry.unwrap();
            let first = geometry.cell_bounds(2, 0);
            let expected = Bounds::new(
                first.origin,
                size(geometry.cell.width * 2.0, geometry.cell.height),
            );
            assert_eq!(
                state.image_cache.placed,
                vec![(ImageLayer::AboveText, expected)]
            );
            assert_eq!(state.image_cache.images.len(), 1);
        });
    }

    #[gpui_kit::test]
    fn an_image_no_frame_places_leaves_the_cache(cx: &mut TestAppContext) {
        let (handle, state) = open(Vec::new(), cx);
        let (mut frame, geometry, image) = setup();
        frame = frame.with_images([ImagePlacement::new(image, 0, 0).with_size(16, 32)]);
        let mut cache = ImageCache::default();
        cx.update_window(handle, |_, window, _| {
            assert_eq!(cache.prepare(&frame, geometry, window).len(), 1);
            assert_eq!(cache.images.len(), 1);
            let empty = frame.clone().with_images([]);
            assert!(cache.prepare(&empty, geometry, window).is_empty());
            assert!(cache.images.is_empty());
        })
        .unwrap();
        drop(state);
    }

    #[test]
    fn a_placement_outside_the_grid_or_without_area_is_skipped() {
        let (frame, geometry, image) = setup();
        let above = ImagePlacement::new(image.clone(), 0, -3).with_size(16, 64);
        assert!(placement_bounds(&above, &frame, geometry).is_none());
        let right = ImagePlacement::new(image.clone(), 10, 0).with_size(16, 32);
        assert!(placement_bounds(&right, &frame, geometry).is_none());
        let empty = ImagePlacement::new(image, 0, 0).with_size(0, 32);
        assert!(placement_bounds(&empty, &frame, geometry).is_none());
    }
}

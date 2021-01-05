//! A simple application that overlays an interactive hyperbezier segment
//! over a cubic Bézier generated from the same points.
use std::sync::Arc;

mod mouse;
use mouse::{Drag, Mouse, MouseDelegate, TaggedEvent};
use spline::SplineSpec;

use wasm_bindgen::prelude::*;

use druid::{
    kurbo::{BezPath, Circle, CubicBez, Line},
    piet::StrokeStyle,
    widget::prelude::*,
    AppLauncher, Color, Data, MouseEvent, Point, Rect, WindowDesc,
};

const MIN_CLICK_DISTANCE: f64 = 8.0;
const CLICK_PENALTY_FACTOR: f64 = 2.0;
const CLICK_PENALTY: f64 = MIN_CLICK_DISTANCE / CLICK_PENALTY_FACTOR;

const SMOOTH_POINT_OUTER_COLOR: Color = Color::rgb8(0x44, 0x28, 0xEC);
const SMOOTH_POINT_INNER_COLOR: Color = Color::rgb8(0x57, 0x9A, 0xFF);
const OFF_CURVE_POINT_OUTER_COLOR: Color = Color::grey8(0x99);
const OFF_CURVE_POINT_INNER_COLOR: Color = Color::grey8(0xCC);
//const BEZIER_FILL_COLOR: Color = Color::rgba8(0x42, 0xad, 0x3a, 0x88);
const BEZIER_FILL_COLOR: Color = Color::grey8(0xbc);
const HYPER_FILL_COLOR: Color = Color::rgba8(0xb7, 0x1d, 0x98, 0x88);

#[derive(Debug, Clone, Data)]
struct Segment {
    p0: Point,
    p1: Point,
    p1_auto: bool,
    p2: Point,
    p2_auto: bool,
    p3: Point,
    selected: Option<u8>,
    #[data(ignore)]
    spline_path: Arc<BezPath>,
}

#[wasm_bindgen]
pub fn comparison_main() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    main()
}

pub fn main() {
    // describe the main window
    let main_window = WindowDesc::new(move || SegmentView::default())
        .title("Spline Toy")
        .with_min_size((200., 200.))
        .window_size((400.0, 400.0));

    // start the application
    AppLauncher::with_window(main_window)
        .launch(Segment::default())
        .expect("Failed to launch application");
}

#[derive(Debug, Clone, Default)]
struct SegmentView {
    mouse: Mouse,
    mouse_state: MouseState,
}

impl Widget<Segment> for SegmentView {
    fn event(&mut self, _ctx: &mut EventCtx, event: &Event, data: &mut Segment, _env: &Env) {
        let pre_data = data.clone();
        match event {
            Event::MouseUp(m) => {
                self.mouse
                    .mouse_event(TaggedEvent::Up(m.clone()), data, &mut self.mouse_state)
            }
            Event::MouseMove(m) => {
                self.mouse
                    .mouse_event(TaggedEvent::Moved(m.clone()), data, &mut self.mouse_state)
            }
            Event::MouseDown(m) => {
                self.mouse
                    .mouse_event(TaggedEvent::Down(m.clone()), data, &mut self.mouse_state)
            }
            _ => (),
        }
        if !pre_data.same(data) {
            data.update_spline();
        }
    }

    fn lifecycle(
        &mut self,
        _ctx: &mut LifeCycleCtx,
        _event: &LifeCycle,
        _data: &Segment,
        _env: &Env,
    ) {
    }

    fn update(&mut self, ctx: &mut UpdateCtx, old_data: &Segment, data: &Segment, _env: &Env) {
        if !old_data.same(data) {
            ctx.request_paint()
        }
    }

    fn layout(
        &mut self,
        _ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        _data: &Segment,
        _env: &Env,
    ) -> Size {
        bc.max()
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &Segment, _: &Env) {
        ctx.clear(Color::WHITE);
        // first draw the cubic:
        let cubic = data.to_cubic();
        ctx.fill(cubic, &BEZIER_FILL_COLOR);
        ctx.fill(&*data.spline_path, &HYPER_FILL_COLOR);
        ctx.stroke(&*data.spline_path, &Color::BLACK, 1.0);

        draw_on_curve(ctx, data.p0, data.selected == Some(0));
        draw_on_curve(ctx, data.p3, data.selected == Some(3));
        draw_off_curve(
            ctx,
            data.p1,
            data.p0,
            data.selected == Some(1),
            data.p1_auto,
        );
        draw_off_curve(
            ctx,
            data.p2,
            data.p3,
            data.selected == Some(2),
            data.p2_auto,
        );
    }
}

fn draw_on_curve(ctx: &mut PaintCtx, pt: Point, selected: bool) {
    let rad = if selected { 5.0 } else { 4.0 };
    let circ = Circle::new(pt, rad);
    ctx.fill(circ, &SMOOTH_POINT_INNER_COLOR);
    if selected {
        ctx.stroke(circ, &SMOOTH_POINT_OUTER_COLOR, 2.0);
    }
}

fn draw_off_curve(ctx: &mut PaintCtx, off: Point, on: Point, selected: bool, is_auto: bool) {
    let thickness = if selected { 2.0 } else { 1.0 };
    let radius = if selected { 4.0 } else { 3.0 };
    let edge = radius * 2.0;
    let line = Line::new(off, on);
    let handle_color = if selected {
        &OFF_CURVE_POINT_OUTER_COLOR
    } else {
        &OFF_CURVE_POINT_INNER_COLOR
    };

    if is_auto {
        let stroke = StrokeStyle::new().dash(vec![2.0, 4.0], 1.0);
        ctx.stroke_styled(line, &OFF_CURVE_POINT_INNER_COLOR, 1.0, &stroke);
        let rect = Rect::from_center_size(off, (edge, edge));
        let line1 = Line::new(rect.origin(), (rect.max_x(), rect.max_y()));
        let line2 = Line::new((rect.x0, rect.y1), (rect.x1, rect.y0));
        ctx.stroke(line1, handle_color, thickness);
        ctx.stroke(line2, handle_color, thickness);
    } else {
        ctx.stroke(line, &OFF_CURVE_POINT_INNER_COLOR, 1.0);
        let circ = Circle::new(off, radius);
        ctx.fill(circ, &OFF_CURVE_POINT_INNER_COLOR);
        if selected {
            ctx.stroke(circ, &OFF_CURVE_POINT_OUTER_COLOR, 2.0);
        }
    }
}

#[derive(Debug, Clone, Default)]
struct MouseState;

impl MouseState {
    fn point_for_click(&self, segment: &Segment, point: Point) -> Option<u8> {
        let mut closest = f64::MAX;
        let mut hit = None;
        for (i, pt) in segment.iter().enumerate() {
            let id = i as u8;
            let is_selected = Some(id) == segment.selected;
            let dist = pt.distance(point);
            let penalty = if is_selected { CLICK_PENALTY } else { 0.0 };
            let score = dist - penalty;
            if (score) < closest {
                closest = score;
                if score < MIN_CLICK_DISTANCE {
                    hit = Some(id);
                }
            }
        }
        hit
    }
}

impl MouseDelegate<Segment> for MouseState {
    fn cancel(&mut self, _data: &mut Segment) {
        //self.drag = None;
    }

    fn left_down(&mut self, event: &MouseEvent, data: &mut Segment) {
        if event.count == 1 {
            data.selected = self.point_for_click(data, event.pos);
        } else if event.count == 2 {
            if data.selected == Some(1) {
                data.p1_auto = !data.p1_auto;
            } else if data.selected == Some(2) {
                data.p2_auto = !data.p2_auto;
            }
        }
    }

    fn left_drag_changed(&mut self, drag: Drag, data: &mut Segment) {
        match data.selected {
            Some(0) => data.p0 = drag.current.pos,
            Some(1) => {
                data.p1_auto = false;
                data.p1 = drag.current.pos;
            }
            Some(2) => {
                data.p2_auto = false;
                data.p2 = drag.current.pos;
            }
            Some(3) => data.p3 = drag.current.pos,
            Some(_) => unreachable!(),
            None => (),
        }
    }
}

impl Segment {
    fn iter(&self) -> impl Iterator<Item = Point> {
        let mut idx = 0;
        let pts = (self.p0, self.p1, self.p2, self.p3);
        std::iter::from_fn(move || {
            idx += 1;
            match idx {
                1 => Some(pts.0),
                2 => Some(pts.1),
                3 => Some(pts.2),
                4 => Some(pts.3),
                _ => None,
            }
        })
    }

    fn to_cubic(&self) -> CubicBez {
        let Segment { p0, p1, p2, p3, .. } = *self;
        CubicBez { p0, p1, p2, p3 }
    }

    fn update_spline(&mut self) {
        let mut spline = SplineSpec::new();
        spline.move_to(self.p0);
        let p1 = if self.p1_auto { None } else { Some(self.p1) };
        let p2 = if self.p2_auto { None } else { Some(self.p2) };
        spline.spline_to(p1, p2, self.p3, true);
        let s = spline.solve();
        let seg = s.segments().first().unwrap();

        if p1.is_none() {
            self.p1 = seg.p1;
        }

        if p2.is_none() {
            self.p2 = seg.p2;
        }

        *Arc::make_mut(&mut self.spline_path) = s.render();
    }
}

impl Default for Segment {
    fn default() -> Self {
        let mut seg = Segment {
            p0: Point::new(100.0, 200.0),
            p1: Point::new(150.0, 150.0),
            p1_auto: false,
            p2: Point::new(250.0, 150.0),
            p2_auto: false,
            p3: Point::new(300.0, 200.0),
            selected: None,
            spline_path: Arc::new(BezPath::new()),
        };
        seg.update_spline();
        seg
    }
}

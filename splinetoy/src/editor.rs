use druid::{
    commands,
    kurbo::{Circle, CubicBez, Line, PathSeg, Point, Vec2},
    piet::StrokeStyle,
    widget::prelude::*,
    Color, Data, Env, HotKey, KbKey, Rect, SysMods, Widget, WidgetPod,
};

use crate::edit_session::EditSession;
use crate::mouse::{Mouse, TaggedEvent};
use crate::path::{Path, SplinePoint};
use crate::pen::Pen;
use crate::toolbar::{FloatingPanel, Toolbar};
use crate::tools::{Tool, ToolId};

const ON_CURVE_CORNER_COLOR: Color = Color::rgb8(0x4B, 0x4E, 0xFF);
const ON_CURVE_SMOOTH_COLOR: Color = Color::rgb8(0x37, 0xA7, 0x62);
const OFF_CURVE_COLOR: Color = Color::grey8(0xbb);
const OFF_CURVE_SELECTED_COLOR: Color = Color::grey8(0x88);
const FLOATING_PANEL_PADDING: f64 = 20.0;

pub struct Editor {
    toolbar: WidgetPod<(), FloatingPanel<Toolbar>>,
    mouse: Mouse,
    tool: Box<dyn Tool>,
    preview: bool,
}

impl Editor {
    pub fn new() -> Editor {
        Editor {
            toolbar: WidgetPod::new(FloatingPanel::new(Toolbar::default())),
            mouse: Mouse::default(),
            tool: Box::new(Pen::default()),
            preview: false,
        }
    }

    fn set_tool(&mut self, ctx: &mut EventCtx, tool: ToolId) {
        if tool != self.tool.name() {
            let tool = crate::tools::tool_for_id(tool).unwrap();
            self.tool = tool;
            self.mouse.reset();
            self.tool.init_mouse(&mut self.mouse);
            let cursor = self.tool.preferred_cursor();
            ctx.set_cursor(&cursor);
        }
    }

    fn send_mouse(&mut self, ctx: &mut EventCtx, event: TaggedEvent, data: &mut EditSession) {
        if !event.inner().button.is_right() {
            // set active, to ensure we receive events if the mouse leaves
            // the window:
            match &event {
                TaggedEvent::Down(_) => ctx.set_active(true),
                TaggedEvent::Up(m) if m.buttons.is_empty() => ctx.set_active(false),
                _ => (),
            };

            self.tool.mouse_event(event, &mut self.mouse, ctx, data);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn save_contents(&self, _: &EditSession) {}

    #[cfg(target_arch = "wasm32")]
    fn save_contents(&self, data: &EditSession) {
        let b64 = data.to_base64_bincode();
        web_sys::console::log_1(&format!("b64 len = {}", b64.len()).into());

        if let Some(window) = web_sys::window() {
            window.location().set_search(&b64);
            web_sys::console::log_1(&format!("set search '{}'", b64).into());
        } else {
            web_sys::console::log_1(&format!("failed to get window handle").into());
        }
    }
}

impl Widget<EditSession> for Editor {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut EditSession, env: &Env) {
        if let Event::KeyDown(k) = event {
            if let Some(new_tool) = self.toolbar.widget().inner().tool_for_keypress(k) {
                let cmd = crate::toolbar::SET_TOOL.with(new_tool);
                ctx.submit_command(cmd);
                ctx.set_handled();
                return;
            } else if HotKey::new(SysMods::Shift, "S").matches(k) {
                self.save_contents(data);
            }
        }

        self.toolbar.event(ctx, event, &mut (), env);
        if ctx.is_handled() {
            return;
        }
        match event {
            Event::WindowConnected => {
                ctx.set_cursor(&self.tool.preferred_cursor());
                ctx.submit_command(crate::toolbar::SET_TOOL.with(crate::pen::TOOL_NAME));
                ctx.request_update();
                ctx.request_focus();
            }
            Event::Command(cmd) if cmd.is(crate::toolbar::SET_TOOL) => {
                let tool = cmd.get_unchecked(crate::toolbar::SET_TOOL);
                self.set_tool(ctx, tool);
            }
            Event::MouseUp(m) => self.send_mouse(ctx, TaggedEvent::Up(m.clone()), data),
            Event::MouseMove(m) => self.send_mouse(ctx, TaggedEvent::Moved(m.clone()), data),
            Event::MouseDown(m) => self.send_mouse(ctx, TaggedEvent::Down(m.clone()), data),
            Event::KeyDown(k) => {
                if k.key == KbKey::Character(" ".into()) {
                    self.preview = true;
                    ctx.request_paint();
                }
                self.tool.key_down(k, ctx, data);
            }
            Event::KeyUp(k) => {
                if k.key == KbKey::Character(" ".into()) {
                    self.preview = false;
                    ctx.request_paint();
                }
                self.tool.key_up(k, ctx, data);
            }
            Event::Command(cmd) if cmd.is(commands::SAVE_FILE_AS) => {
                let file_info = cmd.get_unchecked(commands::SAVE_FILE_AS);
                let json = data.to_json();
                if let Err(e) = std::fs::write(file_info.path(), json.as_bytes()) {
                    println!("Error writing json: {}", e);
                }
            }
            _ => (),
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, _: &EditSession, env: &Env) {
        self.toolbar.lifecycle(ctx, event, &(), env);
    }

    fn update(
        &mut self,
        ctx: &mut UpdateCtx,
        old_data: &EditSession,
        data: &EditSession,
        env: &Env,
    ) {
        if !old_data.same(data) {
            ctx.request_layout();
        }
        self.toolbar.update(ctx, &(), env);
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        _: &EditSession,
        env: &Env,
    ) -> Size {
        let child_bc = bc.loosen();
        let size = self.toolbar.layout(ctx, &child_bc, &(), env);
        let orig = (FLOATING_PANEL_PADDING, FLOATING_PANEL_PADDING);
        self.toolbar
            .set_layout_rect(ctx, &(), env, Rect::from_origin_size(orig, size));
        bc.max()
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &EditSession, env: &Env) {
        ctx.clear(Color::WHITE);
        if self.preview {
            ctx.fill(data.bezier(), &Color::BLACK);
            return;
        }

        for path in data.iter_paths() {
            ctx.stroke(path.bezier(), &Color::BLACK, 1.0);
            if !path.points().is_empty() {
                let first_selected = data.is_selected(path.first_point().unwrap().id);
                draw_first_point(ctx, path, first_selected);
                let mut last_point = path.first_point().unwrap();
                for pt in path.iter_points().skip(1) {
                    let is_selected = data.is_selected(pt.id);
                    if pt.is_on_curve() {
                        draw_on_curve(ctx, pt.point, pt.is_smooth(), is_selected);
                    }
                    let is_selected = handle_is_selected(pt, last_point, data);
                    draw_handle_if_needed(ctx, pt, last_point, is_selected);
                    last_point = pt;
                }

                if let Some(pt) = path.trailing() {
                    draw_handle_if_needed(ctx, SplinePoint::control(pt, true), last_point, false);
                }
            }
        }

        self.toolbar.paint(ctx, &(), env);
    }
}

fn handle_is_selected(p1: SplinePoint, p2: SplinePoint, data: &EditSession) -> bool {
    (p1.is_control() && data.is_selected(p1.id)) || (p2.is_control() && data.is_selected(p2.id))
}

fn draw_first_point(ctx: &mut PaintCtx, path: &Path, is_selected: bool) {
    if path.is_closed() {
        return;
    }
    let rad = if is_selected { 4.0 } else { 3.0 };
    let color = if is_selected {
        OFF_CURVE_SELECTED_COLOR
    } else {
        OFF_CURVE_COLOR
    };

    if path.points().len() == 1 {
        let first = path.points()[0].point;
        let circ = Circle::new(first, rad);
        ctx.fill(circ, &color);
    } else if let Some(tangent) = match path.bezier().segments().next() {
        Some(PathSeg::Line(line)) => Some((line.p1 - line.p0).normalize()),
        Some(PathSeg::Cubic(cubic)) => Some(tangent_vector(0.05, cubic).normalize()),
        _ => None,
    } {
        let p0 = path.points()[0].point;
        let line = perp(p0, p0 + tangent, 10.0);
        ctx.stroke(line, &color, 2.0);
    }
}

/// Create a line perpendicular to the line `(p1, p2)`, centered on `p1`.
fn perp(p0: Point, p1: Point, len: f64) -> Line {
    let perp_vec = Vec2::new(p0.y - p1.y, p1.x - p0.x);
    let norm_perp = perp_vec / perp_vec.hypot();
    let p2 = p0 + (len * -0.5) * norm_perp;
    let p3 = p0 + (len * 0.5) * norm_perp;
    Line::new(p2, p3)
}

/// Return the tangent of the cubic bezier `cb`, at time `t`, as a vector
/// relative to the path's start point.
fn tangent_vector(t: f64, cb: CubicBez) -> Vec2 {
    debug_assert!(t >= 0.0 && t <= 1.0);
    let CubicBez { p0, p1, p2, p3 } = cb;
    let one_minus_t = 1.0 - t;
    3.0 * one_minus_t.powi(2) * (p1 - p0)
        + 6.0 * t * one_minus_t * (p2 - p1)
        + 3.0 * t.powi(2) * (p3 - p2)
}

fn draw_on_curve(ctx: &mut PaintCtx, pt: Point, smooth: bool, selected: bool) {
    let rad = if selected { 5.0 } else { 4.0 };
    if smooth {
        let circ = Circle::new(pt, rad);
        if selected {
            ctx.fill(circ, &ON_CURVE_SMOOTH_COLOR);
        } else {
            ctx.stroke(circ, &ON_CURVE_SMOOTH_COLOR, 1.0);
        }
    } else {
        let square = Rect::from_center_size(pt, (rad * 2.0, rad * 2.0));
        if selected {
            ctx.fill(square, &ON_CURVE_CORNER_COLOR);
        } else {
            ctx.stroke(square, &ON_CURVE_CORNER_COLOR, 1.0);
        }
    }
}

fn draw_handle_if_needed(ctx: &mut PaintCtx, p1: SplinePoint, p2: SplinePoint, selected: bool) {
    if p1.type_.variant_eq(&p2.type_) {
        return;
    }

    let is_auto = p1.is_auto() || p2.is_auto();
    let handle_pt = if p1.is_control() { p1.point } else { p2.point };
    let thickness = if selected { 2.0 } else { 1.0 };
    let radius = if selected { 4.0 } else { 3.0 };
    let edge = radius * 2.0;
    let line = Line::new(p1.point, p2.point);
    let handle_color = if selected {
        &OFF_CURVE_SELECTED_COLOR
    } else {
        &OFF_CURVE_COLOR
    };

    if is_auto {
        let stroke = StrokeStyle::new().dash(vec![2.0, 4.0], 1.0);
        ctx.stroke_styled(line, &OFF_CURVE_COLOR, 1.0, &stroke);
        let rect = Rect::from_center_size(handle_pt, (edge, edge));
        let line1 = Line::new(rect.origin(), (rect.max_x(), rect.max_y()));
        let line2 = Line::new((rect.x0, rect.y1), (rect.x1, rect.y0));
        ctx.stroke(line1, handle_color, thickness);
        ctx.stroke(line2, handle_color, thickness);
    } else {
        ctx.stroke(line, &OFF_CURVE_COLOR, 1.0);
        let circ = Circle::new(handle_pt, radius);
        if selected {
            ctx.fill(circ, handle_color);
        } else {
            ctx.stroke(circ, handle_color, 1.0);
        }
    }
}

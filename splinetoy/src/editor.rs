use std::time::Duration;

#[cfg(target_arch = "wasm32")]
use std::sync::{
    atomic::{AtomicBool, Ordering::Relaxed},
    Arc,
};

use druid::{
    commands,
    kurbo::{Circle, CubicBez, Line, PathSeg, Point, Vec2},
    piet::StrokeStyle,
    widget::prelude::*,
    Color, Data, Env, HotKey, KbKey, Rect, SysMods, TimerToken, Widget, WidgetPod,
};

use crate::edit_session::EditSession;
use crate::mouse::{Mouse, TaggedEvent};
use crate::path::{Path, SplinePoint};
use crate::pen::Pen;
use crate::save::SessionState;
use crate::toolbar::{FloatingPanel, Toolbar};
use crate::tools::{Tool, ToolId};

const SMOOTH_POINT_OUTER_COLOR: Color = Color::rgb8(0x44, 0x28, 0xEC);
const SMOOTH_POINT_INNER_COLOR: Color = Color::rgb8(0x57, 0x9A, 0xFF);
const CORNER_POINT_OUTER_COLOR: Color = Color::rgb8(0x20, 0x8E, 0x56);
const CORNER_POINT_INNER_COLOR: Color = Color::rgb8(0x6A, 0xE7, 0x56);
const OFF_CURVE_POINT_OUTER_COLOR: Color = Color::grey8(0x99);
const OFF_CURVE_POINT_INNER_COLOR: Color = Color::grey8(0xCC);
const FLOATING_PANEL_PADDING: f64 = 20.0;
const SAVE_DURATION: Duration = Duration::from_secs(1);

pub struct Editor {
    toolbar: WidgetPod<(), FloatingPanel<Toolbar>>,
    mouse: Mouse,
    tool: Box<dyn Tool>,
    save_token: TimerToken,
    preview: bool,
    /// if true we are locked in select mode and hide the toolbar.
    select_only: bool,

    #[cfg(target_arch = "wasm32")]
    we_set_anchor: Arc<AtomicBool>,
}

impl Editor {
    pub fn new() -> Editor {
        Editor {
            toolbar: WidgetPod::new(FloatingPanel::new(Toolbar::default())),
            mouse: Mouse::default(),
            tool: Box::new(Pen::default()),
            save_token: TimerToken::INVALID,
            preview: false,
            select_only: false,
            #[cfg(target_arch = "wasm32")]
            we_set_anchor: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn from_saved(tool: ToolId, select_only: bool) -> Editor {
        let tool = crate::tools::tool_for_id(tool).unwrap();
        Editor {
            tool,
            select_only: select_only,
            ..Editor::new()
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

    #[allow(unused_variables)]
    fn save_contents(&self, data: &EditSession) {
        #[cfg(target_arch = "wasm32")]
        {
            self.get_session_state(data).save_to_url();
            self.we_set_anchor.store(true, Relaxed);
        }
    }

    /// Hack: on wasm, we want to force reload when the *user* changes the
    /// hash/anchor string, but not if we've set it ourselves.
    #[allow(dead_code)]
    fn setup_reload_listener(&self) {
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::{closure::Closure, JsCast};
            let window = match web_sys::window() {
                Some(window) => window,
                None => return,
            };
            let flag = self.we_set_anchor.clone();
            let callback: Box<dyn Fn() -> ()> = Box::new(move || {
                if !flag.swap(false, Relaxed) {
                    if let Some(window) = web_sys::window() {
                        let _ = window.location().reload();
                        web_sys::console::log_1(&format!("reloading").into())
                    }
                }
            });
            let closure = Closure::wrap(callback);
            let _ = window
                .add_event_listener_with_callback("hashchange", closure.as_ref().unchecked_ref());
            // we could stash this in Editor but both need to live for the duration
            // of the program?
            closure.forget();
        }
    }

    fn get_session_state(&self, data: &EditSession) -> SessionState {
        crate::save::SessionState {
            paths: data.iter_paths().map(Path::solver).cloned().collect(),
            selection: data.selection(),
            tool: self.tool.name(),
            select_only: self.select_only,
        }
    }
}

impl Widget<EditSession> for Editor {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut EditSession, env: &Env) {
        if let Event::KeyDown(k) = event {
            if !self.select_only {
                if let Some(new_tool) = self.toolbar.widget().inner().tool_for_keypress(k) {
                    let cmd = crate::toolbar::SET_TOOL.with(new_tool);
                    ctx.submit_command(cmd);
                    ctx.set_handled();
                    return;
                }
            }
            if HotKey::new(SysMods::Shift, "S").matches(k) {
                self.save_contents(data);
            } else if HotKey::new(SysMods::Shift, "+").matches(k) {
                data.scale_up();
            } else if HotKey::new(SysMods::Shift, "_").matches(k) {
                data.scale_down();
            }
        }

        if matches!(event, Event::Command(_)) || !self.select_only {
            self.toolbar.event(ctx, event, &mut (), env);
        }

        if ctx.is_handled() {
            return;
        }
        match event {
            Event::WindowConnected => {
                ctx.set_cursor(&self.tool.preferred_cursor());
                ctx.submit_command(crate::toolbar::SET_TOOL.with(self.tool.name()));
                ctx.request_update();
                ctx.request_focus();
                #[cfg(target_arch = "wasm32")]
                {
                    self.setup_reload_listener();
                }
            }
            Event::Timer(token) if *token == self.save_token => {
                self.save_contents(data);
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
            Event::Command(cmd) if cmd.is(crate::toolbar::SET_TOOL) => {
                let tool = cmd.get_unchecked(crate::toolbar::SET_TOOL);
                self.set_tool(ctx, *tool);
            }
            Event::Command(cmd) if cmd.is(crate::TOGGLE_PREVIEW_LOCK) => {
                if !self.select_only {
                    ctx.submit_command(crate::toolbar::SET_TOOL.with(ToolId::Select));
                }
                self.select_only = !self.select_only;
                ctx.request_paint();
            }
            Event::Command(cmd) if cmd.is(crate::RECENTER_GLYPH) => {
                data.recenter_glyph();
            }
            Event::Command(cmd) if cmd.is(commands::SAVE_FILE_AS) => {
                let file_info = cmd.get_unchecked(commands::SAVE_FILE_AS);
                let json = data.to_json();
                if let Err(e) = std::fs::write(file_info.path(), json.as_bytes()) {
                    println!("Error writing json: {}", e);
                }
            }
            Event::Command(cmd) if cmd.is(crate::SAVE_BINARY) => {
                let file_info = cmd.get_unchecked(crate::SAVE_BINARY);
                let save_data = match self.get_session_state(data).encode() {
                    Ok(data) => data,
                    Err(e) => {
                        eprintln!("error encoding session: {}", e);
                        return;
                    }
                };
                if let Err(e) = std::fs::write(file_info.path(), save_data.as_bytes()) {
                    println!("Error writing json: {}", e);
                }
            }

            Event::Command(cmd) if cmd.is(commands::OPEN_FILE) => {
                let file_info = cmd.get_unchecked(commands::OPEN_FILE);
                let extension = file_info
                    .path()
                    .extension()
                    .map(|s| s.to_string_lossy())
                    .unwrap();
                let bytes = std::fs::read(file_info.path()).unwrap();
                let session = match extension.as_ref() {
                    "json" => SessionState::from_json(&bytes),
                    "splinetoy" => SessionState::from_bytes(&bytes),
                    _ => panic!("unexpected file extension '{}'", extension),
                };
                let session = match session {
                    Ok(sesh) => sesh,
                    Err(e) => {
                        eprintln!("error loading data: {}", e);
                        return;
                    }
                };
                ctx.submit_command(crate::toolbar::SET_TOOL.with(session.tool));
                self.select_only = session.select_only;
                *data = session.into_edit_session();
                ctx.request_layout();
                ctx.request_paint();
            }
            _ => (),
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, _: &EditSession, env: &Env) {
        if !self.select_only {
            self.toolbar.lifecycle(ctx, event, &(), env);
        }
    }

    fn update(
        &mut self,
        ctx: &mut UpdateCtx,
        old_data: &EditSession,
        data: &EditSession,
        env: &Env,
    ) {
        if !old_data.same(data) {
            self.save_token = ctx.request_timer(SAVE_DURATION);
            ctx.request_layout();
        }
        if !self.select_only {
            self.toolbar.update(ctx, &(), env);
        }
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        _: &EditSession,
        env: &Env,
    ) -> Size {
        if !self.select_only {
            let child_bc = bc.loosen();
            let size = self.toolbar.layout(ctx, &child_bc, &(), env);
            let orig = (FLOATING_PANEL_PADDING, FLOATING_PANEL_PADDING);
            self.toolbar
                .set_layout_rect(ctx, &(), env, Rect::from_origin_size(orig, size));
        }
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

        if !self.select_only {
            self.toolbar.paint(ctx, &(), env);
        }
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

    if path.points().len() == 1 {
        let first = path.points()[0].point;
        let circ = Circle::new(first, rad);
        ctx.fill(circ, &OFF_CURVE_POINT_INNER_COLOR);
        if is_selected {
            ctx.stroke(circ, &OFF_CURVE_POINT_OUTER_COLOR, 2.0);
        }
    } else if let Some(tangent) = match path.bezier().segments().next() {
        Some(PathSeg::Line(line)) => Some((line.p1 - line.p0).normalize()),
        Some(PathSeg::Cubic(cubic)) => Some(tangent_vector(0.05, cubic).normalize()),
        _ => None,
    } {
        let p0 = path.points()[0].point;
        let line = perp(p0, p0 + tangent, 10.0);
        if is_selected {
            ctx.stroke(line, &OFF_CURVE_POINT_OUTER_COLOR, 2.0);
        } else {
            ctx.stroke(line, &OFF_CURVE_POINT_INNER_COLOR, 2.0);
        }
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
        ctx.fill(circ, &SMOOTH_POINT_INNER_COLOR);
        if selected {
            ctx.stroke(circ, &SMOOTH_POINT_OUTER_COLOR, 2.0);
        }
    //} else {
    //ctx.stroke(circ, &ON_CURVE_SMOOTH_COLOR, 1.0);
    //}
    } else {
        let square = Rect::from_center_size(pt, (rad * 2.0, rad * 2.0));
        ctx.fill(square, &CORNER_POINT_INNER_COLOR);
        if selected {
            ctx.stroke(square, &CORNER_POINT_OUTER_COLOR, 2.0);
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
        &OFF_CURVE_POINT_OUTER_COLOR
    } else {
        &OFF_CURVE_POINT_INNER_COLOR
    };

    if is_auto {
        let stroke = StrokeStyle::new().dash(vec![2.0, 4.0], 1.0);
        ctx.stroke_styled(line, &OFF_CURVE_POINT_INNER_COLOR, 1.0, &stroke);
        let rect = Rect::from_center_size(handle_pt, (edge, edge));
        let line1 = Line::new(rect.origin(), (rect.max_x(), rect.max_y()));
        let line2 = Line::new((rect.x0, rect.y1), (rect.x1, rect.y0));
        ctx.stroke(line1, handle_color, thickness);
        ctx.stroke(line2, handle_color, thickness);
    } else {
        ctx.stroke(line, &OFF_CURVE_POINT_INNER_COLOR, 1.0);
        let circ = Circle::new(handle_pt, radius);
        ctx.fill(circ, &OFF_CURVE_POINT_INNER_COLOR);
        if selected {
            ctx.stroke(circ, &OFF_CURVE_POINT_OUTER_COLOR, 2.0);
        }
    }
}

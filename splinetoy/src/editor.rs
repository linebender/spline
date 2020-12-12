use druid::{
    commands,
    kurbo::{Circle, Line, Point},
    piet::StrokeStyle,
    widget::{prelude::*, Label},
    Color, Data, Env, KbKey, Rect, Widget, WidgetPod,
};

use crate::edit_session::EditSession;
use crate::mouse::{Mouse, TaggedEvent};
use crate::path::SplinePoint;
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
    points_label: Label<()>,
    mouse: Mouse,
    tool: Box<dyn Tool>,
    label_size: Size,
}

impl Editor {
    pub fn new() -> Editor {
        Editor {
            points_label: Label::new("")
                .with_text_size(12.0)
                .with_text_color(Color::grey(0.3)),
            toolbar: WidgetPod::new(FloatingPanel::new(Toolbar::default())),
            label_size: Size::ZERO,
            mouse: Mouse::default(),
            tool: Box::new(Pen::default()),
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
}

impl Widget<EditSession> for Editor {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut EditSession, env: &Env) {
        if let Event::KeyDown(k) = event {
            if let Some(new_tool) = self.toolbar.widget().inner().tool_for_keypress(k) {
                let cmd = crate::toolbar::SET_TOOL.with(new_tool);
                ctx.submit_command(cmd);
                ctx.set_handled();
                return;
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
            Event::KeyDown(key) if key.key == KbKey::Backspace => {
                data.remove_last_segment();
            }
            Event::KeyDown(key) if key.key == KbKey::Escape => {
                data.close();
            }
            Event::Command(cmd) if cmd.is(commands::SAVE_FILE) => {
                if let Some(file_info) = cmd.get_unchecked(commands::SAVE_FILE) {
                    let json = data.to_json();
                    if let Err(e) = std::fs::write(file_info.path(), json.as_bytes()) {
                        println!("Error writing json: {}", e);
                    }
                }
            }
            _ => (),
        }
        self.points_label.event(ctx, event, &mut (), env);
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, _: &EditSession, env: &Env) {
        self.points_label.lifecycle(ctx, event, &(), env);
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
        self.points_label.update(ctx, &(), &(), env);
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
        self.label_size = self.points_label.layout(ctx, &bc.loosen(), &(), env);
        bc.max()
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &EditSession, env: &Env) {
        ctx.clear(Color::WHITE);
        ctx.stroke(data.path.bezier(), &Color::BLACK, 1.0);
        if !data.path.points().is_empty() {
            let mut last_point = data.path.points()[0];
            for pt in data.path.points() {
                let is_selected = data.is_selected(pt.id);
                if pt.is_on_curve() {
                    draw_on_curve(ctx, pt.point, pt.is_smooth(), is_selected);
                }
                let is_selected = handle_is_selected(*pt, last_point, data);
                draw_handle_if_needed(ctx, *pt, last_point, is_selected);
                last_point = *pt;
            }

            if let Some(pt) = data.path.trailing() {
                draw_handle_if_needed(ctx, SplinePoint::control(pt, true), last_point, false);
            }
        }

        let origin = (10.0, ctx.size().height - self.label_size.height - 10.0);
        self.points_label.draw_at(ctx, origin);
        self.toolbar.paint(ctx, &(), env);
    }
}

fn handle_is_selected(p1: SplinePoint, p2: SplinePoint, data: &EditSession) -> bool {
    (p1.is_control() && data.is_selected(p1.id)) || (p2.is_control() && data.is_selected(p2.id))
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

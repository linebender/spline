use druid::{EventCtx, KbKey, KeyEvent, MouseEvent, Vec2};

use crate::mouse::{Drag, Mouse, MouseDelegate, TaggedEvent};
use crate::tools::{self, EditType, Tool, ToolId};
use crate::{edit_session::EditSession, path::PointId};

/// A set of states that are possible while handling a mouse drag.
#[derive(Debug, Clone)]
enum DragState {
    /// State for a drag that is moving an on-curve point.
    MovePoint(PointId),
    /// State for a drag that is moving an off-curve point.
    MoveControl(PointId),
    ///// State if some earlier gesture consumed the mouse-down, and we should not
    ///// recognize a drag.
    //Suppress,
    None,
}

/// The state of the selection tool.
#[derive(Debug, Default, Clone)]
pub struct Select {
    /// the state preserved between drag events.
    drag: DragState,
    //last_pos: Point,
    /// The edit type produced by the current event, if any.
    ///
    /// This is stashed here because we can't return anything from the methods in
    /// `MouseDelegate`.
    ///
    /// It is an invariant that this is always `None`, except while we are in
    /// a `key_down`, `key_up`, or `mouse_event` method.
    this_edit_type: Option<EditType>,
}

impl Select {
    fn nudge(&mut self, data: &mut EditSession, event: &KeyEvent) {
        let mut nudge = match event.key {
            KbKey::ArrowLeft => Vec2::new(-1.0, 0.),
            KbKey::ArrowRight => Vec2::new(1.0, 0.),
            KbKey::ArrowUp => Vec2::new(0.0, -1.0),
            KbKey::ArrowDown => Vec2::new(0.0, 1.0),
            _ => unreachable!(),
        };

        if event.mods.meta() {
            nudge *= 100.;
        } else if event.mods.shift() {
            nudge *= 10.;
        }

        if event.mods.alt() {
            data.nudge_selected_path(nudge);
        } else {
            data.nudge_selection(nudge);
        }
    }
}

impl Tool for Select {
    fn key_down(
        &mut self,
        key: &KeyEvent,
        ctx: &mut EventCtx,
        data: &mut EditSession,
    ) -> Option<EditType> {
        match key {
            e if e.key == KbKey::ArrowLeft
                || e.key == KbKey::ArrowDown
                || e.key == KbKey::ArrowUp
                || e.key == KbKey::ArrowRight =>
            {
                self.nudge(data, key);
            }
            e if e.key == KbKey::Backspace => {
                data.delete();
                ctx.set_handled();
            }
            _ => (),
        };
        None
    }

    fn mouse_event(
        &mut self,
        event: TaggedEvent,
        mouse: &mut Mouse,
        ctx: &mut EventCtx,
        data: &mut EditSession,
    ) -> Option<EditType> {
        assert!(self.this_edit_type.is_none());
        if matches!(&event, TaggedEvent::Down(_)) {
            ctx.set_active(true);
        }
        if matches!(&event, TaggedEvent::Up(m) if m.buttons.is_empty()) {
            ctx.set_active(false);
        }
        mouse.mouse_event(event, data, self);
        self.this_edit_type.take()
    }

    fn name(&self) -> ToolId {
        ToolId::Select
    }
}

impl MouseDelegate<EditSession> for Select {
    fn left_down(&mut self, event: &MouseEvent, data: &mut EditSession) {
        assert!(matches!(self.drag, DragState::None));
        if event.count == 1 {
            let point = data.hit_test_points(event.pos, None);
            data.set_selection(point);
            if point.is_none() && event.mods.alt() {
                // see if we clicked a line?
                data.maybe_convert_line_to_spline(event.pos);
            }
        } else if event.count == 2 {
            data.toggle_selected_point_type();
        }
    }

    fn left_up(&mut self, _event: &MouseEvent, _data: &mut EditSession) {
        self.drag = DragState::None;
    }

    fn left_drag_began(&mut self, drag: Drag, data: &mut EditSession) {
        match data.selected_point() {
            Some(pt) if pt.is_on_curve() => {
                self.drag = DragState::MovePoint(pt.id);
                data.move_point(pt.id, drag.current.pos);
            }
            Some(pt) => {
                self.drag = DragState::MoveControl(pt.id);
                data.update_handle(pt.id, drag.current.pos, drag.current.mods.shift());
            }
            None => (),
        }
    }

    fn left_drag_changed(&mut self, drag: Drag, data: &mut EditSession) {
        match self.drag {
            DragState::MovePoint(id) => {
                let Drag { start, current, .. } = drag;
                let point = if current.mods.shift() {
                    tools::axis_locked_point(current.pos, start.pos)
                } else {
                    current.pos
                };
                data.move_point(id, point);
            }
            DragState::MoveControl(id) => {
                data.update_handle(id, drag.current.pos, drag.current.mods.shift())
            }
            DragState::None => (),
        }
    }

    fn left_drag_ended(&mut self, _drag: Drag, _data: &mut EditSession) {
        if self.drag.is_move() {
            self.this_edit_type = Some(EditType::DragUp);
        }
    }

    fn cancel(&mut self, _data: &mut EditSession) {
        self.drag = DragState::None
    }
}

impl Default for DragState {
    fn default() -> Self {
        DragState::None
    }
}

impl DragState {
    fn is_move(&self) -> bool {
        matches!(self, DragState::MovePoint(_))
    }
}

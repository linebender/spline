use druid::{EventCtx, KbKey, KeyEvent, MouseEvent};

use crate::mouse::{Drag, Mouse, MouseDelegate, TaggedEvent};
use crate::tools::{EditType, Tool, ToolId};
use crate::{edit_session::EditSession, path::PointId};

pub const TOOL_NAME: &str = "Select";

/// A set of states that are possible while handling a mouse drag.
#[derive(Debug, Clone)]
enum DragState {
    /// State for a drag that is moving an off-curve point.
    MovePoint(PointId),
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

impl Tool for Select {
    fn key_down(
        &mut self,
        key: &KeyEvent,
        ctx: &mut EventCtx,
        data: &mut EditSession,
    ) -> Option<EditType> {
        if key.key == KbKey::Backspace {
            data.delete();
            ctx.set_handled();
        }
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
        "Select"
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
        if let Some(pt) = data.selected_point() {
            self.drag = DragState::MovePoint(pt.id);
            data.move_point(pt.id, drag.current.pos);
        }
    }

    fn left_drag_changed(&mut self, drag: Drag, data: &mut EditSession) {
        if let DragState::MovePoint(id) = self.drag {
            data.move_point(id, drag.current.pos);
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

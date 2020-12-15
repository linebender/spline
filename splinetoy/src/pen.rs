//! The bezier pen tool.

use druid::{EventCtx, KbKey, KeyEvent, MouseEvent};

use crate::{
    edit_session::EditSession,
    mouse::{Drag, Mouse, MouseDelegate, TaggedEvent},
    //path::Path,
    tools::{self, EditType, Tool, ToolId},
};

pub const TOOL_NAME: &str = "Pen";

/// The state of the pen.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Pen {
    //is_draggable: bool,
}

impl MouseDelegate<EditSession> for Pen {
    fn cancel(&mut self, _data: &mut EditSession) {}

    fn left_down(&mut self, event: &MouseEvent, data: &mut EditSession) {
        if event.count == 1 {
            let smooth = event.mods.alt();
            let point = match data.path.points().last() {
                Some(prev) if event.mods.shift() => tools::axis_locked_point(event.pos, prev.point),
                _ => event.pos,
            };
            data.add_point(point, smooth);
        } else if event.count == 2 {
        }
    }

    fn left_drag_changed(&mut self, drag: Drag, data: &mut EditSession) {
        let Drag { start, current, .. } = drag;
        let point = if current.mods.shift() {
            tools::axis_locked_point(current.pos, start.pos)
        } else {
            current.pos
        };
        data.update_for_drag(point);
    }
}

impl Tool for Pen {
    fn mouse_event(
        &mut self,
        event: TaggedEvent,
        mouse: &mut Mouse,
        ctx: &mut EventCtx,
        data: &mut EditSession,
    ) -> Option<EditType> {
        if matches!(&event, TaggedEvent::Down(_)) {
            ctx.set_active(true);
        }
        if matches!(&event, TaggedEvent::Up(m) if m.buttons.is_empty()) {
            ctx.set_active(false);
        }
        mouse.mouse_event(event, data, self);
        None
    }

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

    fn name(&self) -> ToolId {
        TOOL_NAME
    }
}

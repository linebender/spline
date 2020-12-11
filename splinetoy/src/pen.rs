//! The bezier pen tool.

use druid::{EventCtx, KbKey, KeyEvent, MouseEvent};

use crate::{
    edit_session::EditSession,
    mouse::{Drag, Mouse, MouseDelegate, TaggedEvent},
    //path::Path,
    tools::{EditType, Tool, ToolId},
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
        //self.is_draggable = false;
        //let vport = data.viewport;
        if event.count == 1 {
            let smooth = event.mods.alt();
            data.add_point(event.pos, smooth);
        } else if event.count == 2 {
        }
    }

    fn left_up(&mut self, _event: &MouseEvent, _data: &mut EditSession) {
        //if let Some(path) = data.active_path_mut() {
        //if path.is_closed() || path.points().len() > 1 && !path.last_segment_is_curve() {
        //path.clear_trailing();
        //}
        //}
    }

    fn left_drag_changed(&mut self, drag: Drag, data: &mut EditSession) {
        //if !self.is_draggable {
        //return;
        //}
        let Drag { current, .. } = drag;
        data.update_for_drag(current.pos);
        //self.this_edit_type = Some(EditType::Drag);
    }

    fn left_drag_ended(&mut self, _: Drag, _: &mut EditSession) {
        // TODO: this logic needs rework. A click-drag sequence should be a single
        // undo group.
        //self.this_edit_type = Some(EditType::DragUp);
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
        //self.this_edit_type.take()
    }

    fn key_down(
        &mut self,
        event: &KeyEvent,
        _ctx: &mut EventCtx,
        _data: &mut EditSession,
    ) -> Option<EditType> {
        //assert!(self.this_edit_type.is_none());
        match event {
            e if e.key == KbKey::Backspace => {
                //data.delete_selection();
                //self.this_edit_type = Some(EditType::Normal);
            }
            // TODO: should support nudging; basically a lot of this should
            // be shared with selection.
            _ => return None,
        }
        None
        //self.this_edit_type.take()
    }

    fn name(&self) -> ToolId {
        TOOL_NAME
    }
}

//use std::collections::HashSet;

use druid::{Data, Point};

use crate::path::{Path, PointId, SplinePoint};

const MIN_CLICK_DISTANCE: f64 = 10.0;
const CLICK_PENALTY: f64 = MIN_CLICK_DISTANCE / 2.0;

#[derive(Clone, Debug, Data)]
pub struct EditSession {
    pub path: Path,
    selection: Option<PointId>,
}

impl EditSession {
    pub fn new() -> EditSession {
        EditSession {
            path: Path::new(),
            selection: None,
        }
    }

    pub fn add_point(&mut self, point: Point, smooth: bool) {
        if !self.path.closed() {
            let sel = self.path.add_point(point, smooth);
            self.selection = Some(sel);
        }
    }

    pub fn update_for_drag(&mut self, handle: Point) {
        self.path.update_for_drag(handle)
    }

    pub fn remove_last_segment(&mut self) {
        self.selection = self.path.remove_last_segment()
    }

    pub fn close(&mut self) {
        self.path.close();
        self.selection = None;
    }

    pub fn is_selected(&self, id: PointId) -> bool {
        Some(id) == self.selection
    }

    pub fn selected_point(&self) -> Option<SplinePoint> {
        self.selection
            .and_then(|id| self.path.points().iter().find(|pt| pt.id == id).copied())
    }

    pub fn set_selection(&mut self, selection: Option<PointId>) {
        self.selection = selection;
    }

    pub fn maybe_convert_line_to_spline(&mut self, point: Point) {
        self.path
            .maybe_convert_line_to_spline(point, MIN_CLICK_DISTANCE);
    }

    pub fn toggle_selected_point_type(&mut self) {
        if let Some(id) = self.selection {
            self.path.toggle_point_type(id);
        }
    }

    pub fn move_point(&mut self, id: PointId, pos: Point) {
        self.path.move_point(id, pos);
    }

    pub fn hit_test_points(&self, point: Point, max_dist: Option<f64>) -> Option<PointId> {
        let max_dist = max_dist.unwrap_or(MIN_CLICK_DISTANCE);
        let mut best = None;
        for p in self.path.points() {
            let dist = p.point.distance(point);
            // penalize the currently selected point
            let sel_penalty = if Some(p.id) == self.selection {
                CLICK_PENALTY
            } else {
                0.0
            };
            let score = dist + sel_penalty;
            if dist < max_dist && best.map(|(s, _id)| score < s).unwrap_or(true) {
                best = Some((score, p.id))
            }
        }
        best.map(|(_score, id)| id)
    }

    pub fn to_json(&self) -> String {
        let paths = [self.path.solver()];
        serde_json::to_string_pretty(&paths).unwrap_or_default()
    }
}

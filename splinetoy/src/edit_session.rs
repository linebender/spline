use std::sync::Arc;

use druid::kurbo::BezPath;
use druid::{Data, Point, Vec2};

use crate::path::{Path, PointId, SplinePoint};

const MIN_CLICK_DISTANCE: f64 = 10.0;
const CLICK_PENALTY_FACTOR: f64 = 2.0;

#[derive(Clone, Debug, Data)]
pub struct EditSession {
    path: Path,
    paths: Arc<Vec<Path>>,
    selection: Option<PointId>,
}

impl EditSession {
    pub fn new() -> EditSession {
        EditSession {
            path: Path::new(),
            selection: None,
            paths: Arc::new(Vec::new()),
        }
    }

    pub fn from_saved(mut paths: Vec<Path>, sel: Option<PointId>) -> Self {
        let path = if paths.last().map(Path::is_closed).unwrap_or(false) {
            paths.pop().unwrap()
        } else {
            Path::new()
        };
        let selection = sel.or_else(|| path.last_point().map(|pt| pt.id));
        EditSession {
            path,
            paths: Arc::new(paths),
            selection,
        }
    }

    pub fn active_path(&self) -> &Path {
        &self.path
    }

    pub fn iter_paths(&self) -> impl Iterator<Item = &Path> {
        Some(&self.path).into_iter().chain(self.paths.iter())
    }

    fn iter_paths_mut(&mut self) -> impl Iterator<Item = &mut Path> {
        let paths = Arc::make_mut(&mut self.paths);
        Some(&mut self.path).into_iter().chain(paths.iter_mut())
    }

    pub fn bezier(&self) -> BezPath {
        self.iter_paths()
            .flat_map(|p| p.bezier().elements())
            .copied()
            .collect()
    }

    pub fn add_point(&mut self, point: Point, smooth: bool) {
        // if the current path is closed we need to add a new path
        if self.path.is_closed() {
            let path = std::mem::replace(&mut self.path, Path::new());
            Arc::make_mut(&mut self.paths).push(path);
        }

        if self
            .path
            .points()
            .first()
            .map(|pt| pt.point.distance(point) < MIN_CLICK_DISTANCE)
            .unwrap_or(false)
        {
            self.selection = Some(self.path.close(smooth));
        } else if let Some((idx, _)) = self.nearest_segment_for_point(point) {
            let sel = match idx {
                0 => self.path.insert_point_on_path(point),
                n => Arc::make_mut(&mut self.paths)
                    .get_mut(n - 1)
                    .unwrap()
                    .insert_point_on_path(point),
            };
            self.selection = Some(sel);
        } else {
            let sel = self.path.add_point(point, smooth);
            self.selection = Some(sel);
        }
    }

    pub fn update_for_drag(&mut self, handle: Point) {
        self.path.update_for_drag(handle)
    }

    pub fn delete(&mut self) {
        if let Some(sel) = self.selection.take() {
            self.selection = self.path_containing_pt_mut(sel).delete(sel);
        }
        if self.selection.is_none() {
            Arc::make_mut(&mut self.paths).retain(|path| !path.points().is_empty())
        }
    }

    pub fn nudge_selection(&mut self, delta: Vec2) {
        if let Some(sel) = self.selection {
            self.path_containing_pt_mut(sel).nudge(sel, delta);
        }
    }

    pub fn nudge_selected_path(&mut self, delta: Vec2) {
        if let Some(sel) = self.selection {
            self.path_containing_pt_mut(sel).nudge_all(delta);
        }
    }

    pub fn is_selected(&self, id: PointId) -> bool {
        Some(id) == self.selection
    }

    pub fn selected_point(&self) -> Option<SplinePoint> {
        self.selection.and_then(|id| {
            self.iter_paths()
                .flat_map(Path::points)
                .find(|pt| pt.id == id)
                .copied()
        })
    }

    pub fn selection(&self) -> Option<PointId> {
        self.selection
    }

    pub fn set_selection(&mut self, selection: Option<PointId>) {
        self.selection = selection;
    }

    pub fn update_handle(&mut self, point: PointId, new_pos: Point, axis_locked: bool) {
        self.path_containing_pt_mut(point)
            .update_handle(point, new_pos, axis_locked);
    }

    pub fn maybe_convert_line_to_spline(&mut self, point: Point) {
        let closest = self.nearest_segment_for_point(point);
        match closest {
            Some((0, _)) => self
                .path
                .maybe_convert_line_to_spline(point, MIN_CLICK_DISTANCE),
            Some((n, _)) => Arc::make_mut(&mut self.paths)
                .get_mut(n - 1)
                .unwrap()
                .maybe_convert_line_to_spline(point, MIN_CLICK_DISTANCE),
            _ => (),
        }
    }

    /// returns a path index and a distance, where '0' is the active path
    fn nearest_segment_for_point(&self, point: Point) -> Option<(usize, f64)> {
        self.iter_paths().enumerate().fold(None, |acc, (i, path)| {
            let dist = path.nearest_segment_distance(point);
            match acc {
                Some((cur_idx, cur_dist)) if cur_dist < dist => Some((cur_idx, cur_dist)),
                _ if dist < MIN_CLICK_DISTANCE => Some((i, dist)),
                _ => None,
            }
        })
    }

    pub fn toggle_selected_point_type(&mut self) {
        if let Some(id) = self.selection {
            self.path_containing_pt_mut(id).toggle_point_type(id);
        }
    }

    pub fn toggle_point_type(&mut self, id: PointId) {
        self.path_containing_pt_mut(id).toggle_point_type(id);
    }

    pub fn move_point(&mut self, id: PointId, pos: Point) {
        self.path_containing_pt_mut(id).move_point(id, pos);
    }

    pub fn hit_test_points(&self, point: Point, max_dist: Option<f64>) -> Option<PointId> {
        let max_dist = max_dist.unwrap_or(MIN_CLICK_DISTANCE);
        let penalty = max_dist / CLICK_PENALTY_FACTOR;
        let mut best = None;
        for p in self.iter_paths().flat_map(Path::points) {
            let dist = p.point.distance(point);
            // penalize the currently selected point
            let score = if Some(p.id) == self.selection {
                dist + penalty
            } else {
                dist
            };
            if dist < max_dist && best.map(|(s, _id)| score < s).unwrap_or(true) {
                best = Some((score, p.id))
            }
        }
        best.map(|(_score, id)| id)
    }

    /// move the glyph so that its origin is near the top left
    pub fn recenter_glyph(&mut self) {
        const DESIRED_ORIGIN: Point = Point::new(80., 60.0);
        let mut bboxes = self.iter_paths().filter_map(|path| {
            if path.points().is_empty() {
                None
            } else {
                Some(path.bounding_box())
            }
        });
        let init = match bboxes.next() {
            Some(bbox) => bbox,
            None => return,
        };

        let bbox = bboxes.fold(init, |all, this| all.union(this));
        let nudge_delta = DESIRED_ORIGIN - bbox.origin();
        for path in self.iter_paths_mut() {
            path.nudge_all(nudge_delta);
        }
    }

    pub fn to_json(&self) -> String {
        let paths = [self.path.solver()];
        serde_json::to_string_pretty(&paths).unwrap_or_default()
    }

    fn path_containing_pt_mut(&mut self, point: PointId) -> &mut Path {
        if self.path.contains_point(point) {
            &mut self.path
        } else {
            let paths = Arc::make_mut(&mut self.paths);
            paths.iter_mut().find(|p| p.contains_point(point)).unwrap()
        }
    }
}

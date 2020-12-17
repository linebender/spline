use std::sync::Arc;

use druid::kurbo::{BezPath, Line, ParamCurve, ParamCurveNearest};
use druid::{Data, Point, Vec2};
use spline::{Element, SplineSpec};

#[derive(Clone, Debug, Data)]
pub struct Path {
    points: Arc<Vec<SplinePoint>>,
    bezier: Arc<BezPath>,
    trailing: Option<Point>,
    #[data(ignore)]
    solver: SplineSpec,
    closed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Data)]
pub struct PointId {
    id: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Data)]
pub enum PointType {
    OnCurve { smooth: bool },
    Control { auto: bool },
}

#[derive(Debug, Clone, Copy, Data)]
pub struct SplinePoint {
    pub type_: PointType,
    pub point: Point,
    pub id: PointId,
}

impl PointId {
    fn next() -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT_ID: AtomicUsize = AtomicUsize::new(5);
        PointId {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        }
    }
}

impl PointType {
    pub fn variant_eq(&self, other: &PointType) -> bool {
        match (self, other) {
            (PointType::OnCurve { .. }, PointType::OnCurve { .. }) => true,
            (PointType::Control { .. }, PointType::Control { .. }) => true,
            _ => false,
        }
    }

    pub fn is_auto(&self) -> bool {
        matches!(self, PointType::Control { auto: true })
    }

    pub fn is_control(&self) -> bool {
        matches!(self, PointType::Control { .. })
    }

    pub fn is_on_curve(&self) -> bool {
        matches!(self, PointType::OnCurve { .. })
    }

    pub fn is_smooth(&self) -> bool {
        matches!(self, PointType::OnCurve { smooth: true })
    }
}

impl SplinePoint {
    fn on_curve(point: Point, smooth: bool) -> SplinePoint {
        SplinePoint {
            point,
            type_: PointType::OnCurve { smooth },
            id: PointId::next(),
        }
    }

    pub fn control(point: Point, auto: bool) -> SplinePoint {
        SplinePoint {
            point,
            type_: PointType::Control { auto },
            id: PointId::next(),
        }
    }
    pub fn is_auto(&self) -> bool {
        self.type_.is_auto()
    }

    pub fn is_control(&self) -> bool {
        self.type_.is_control()
    }

    pub fn is_on_curve(&self) -> bool {
        self.type_.is_on_curve()
    }

    pub fn is_smooth(&self) -> bool {
        self.type_.is_smooth()
    }

    pub fn toggle_type(&mut self) {
        match &mut self.type_ {
            PointType::OnCurve { smooth } => *smooth = !*smooth,
            PointType::Control { auto } => *auto = !*auto,
        }
    }
}

impl Default for Path {
    fn default() -> Self {
        Path {
            points: Arc::new(Vec::new()),
            bezier: Arc::new(BezPath::default()),
            solver: SplineSpec::new(),
            trailing: None,
            closed: false,
        }
    }
}

impl Path {
    pub fn new() -> Path {
        Path::default()
        //Path::debug()
    }

    //fn debug() -> Path {
    //let mut path = Path::default();
    //path.move_to((100., 100.), false);
    //path.spline_to((200., 150.), false);
    //path.line_to((50., 200.0), true);
    //path.line_to((250.0, 250.0), false);
    //path.spline_to((300., 300.0), false);
    //path.after_change();
    //path
    //}

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn points(&self) -> &[SplinePoint] {
        &self.points
    }

    pub fn bezier(&self) -> &BezPath {
        &self.bezier
    }

    pub fn trailing(&self) -> Option<Point> {
        self.trailing
    }

    pub fn solver(&self) -> &SplineSpec {
        &self.solver
    }

    fn points_mut(&mut self) -> &mut Vec<SplinePoint> {
        Arc::make_mut(&mut self.points)
    }

    pub fn iter_points<'a>(&'a self) -> impl Iterator<Item = SplinePoint> + 'a {
        let first = if self.closed {
            self.first_point()
        } else {
            None
        };
        first.into_iter().chain(self.points.iter().copied())
    }

    pub fn first_point(&self) -> Option<SplinePoint> {
        if self.closed {
            self.points().last().cloned()
        } else {
            self.points().first().cloned()
        }
    }

    pub fn contains_point(&self, id: PointId) -> bool {
        self.points().iter().any(|pt| pt.id == id)
    }

    pub fn add_point(&mut self, point: Point, smooth: bool) -> PointId {
        if self.points.is_empty() {
            self.move_to(point, smooth);
        } else if !smooth && self.trailing.is_none() {
            self.line_to(point, smooth);
        } else {
            self.spline_to(point, smooth);
        }
        self.after_change();
        self.trailing = None;
        self.points().last().map(|pt| pt.id).unwrap()
    }

    fn move_to(&mut self, point: impl Into<Point>, smooth: bool) {
        let point = point.into();
        self.points_mut().push(SplinePoint::on_curve(point, smooth));
        self.solver.move_to(point);
    }

    fn line_to(&mut self, point: impl Into<Point>, smooth: bool) {
        let point = point.into();
        self.points_mut().push(SplinePoint::on_curve(point, smooth));
        self.solver.line_to(point, smooth);
    }

    fn spline_to(&mut self, p3: impl Into<Point>, smooth: bool) {
        let p3 = p3.into();
        let prev = self.points().last().cloned().unwrap().point;
        let p1 = prev.lerp(p3, 1.0 / 3.0);
        let p2 = prev.lerp(p3, 2.0 / 3.0);
        self.points_mut().push(SplinePoint::control(p1, true));
        self.points_mut().push(SplinePoint::control(p2, true));
        self.points_mut().push(SplinePoint::on_curve(p3, smooth));
        self.solver.spline_to(None, None, p3, smooth);
    }

    /// Given a click position, find the closest point on the spline
    /// to that position and add a point there.
    pub fn insert_point_on_path(&mut self, pt: Point) -> PointId {
        let spline = self.solver.solve().into_owned();
        let seg_beziers = spline
            .segments()
            .iter()
            .map(|seg| seg.to_bezier())
            .collect::<Vec<_>>();

        let mut closest = f64::MAX;
        let mut seg_idx = 0;
        let mut new_pt = Point::ZERO;

        for (i, bez) in seg_beziers.iter().enumerate() {
            let (b_closest, b_point) = bez.segments().fold((f64::MAX, Point::ZERO), |acc, seg| {
                let (t, dist) = seg.nearest(pt, 0.1);
                if dist < acc.0 {
                    (dist, seg.eval(t))
                } else {
                    acc
                }
            });
            if b_closest < closest {
                closest = b_closest;
                new_pt = b_point;
                seg_idx = i;
            }
        }
        let skip_n = if self.closed { 0 } else { 1 };
        let mut pt_idx = 0;
        let mut segs_seen = 0;

        for (i, pt) in self.points().iter().skip(skip_n).enumerate() {
            if segs_seen == seg_idx {
                pt_idx = i;
                break;
            }
            if pt.is_on_curve() {
                segs_seen += 1;
            }
        }

        let is_line = spline.segments()[seg_idx].is_line();
        let is_smooth = self
            .points()
            .iter()
            .skip(skip_n + pt_idx)
            .skip_while(|pt| pt.is_control())
            .next()
            .map(SplinePoint::is_smooth)
            .unwrap();

        let new_on_curve = SplinePoint::on_curve(new_pt, is_smooth);
        self.points_mut().insert(pt_idx, new_on_curve);
        if !is_line {
            self.points_mut()
                .insert(pt_idx, SplinePoint::control(new_pt, true));
            self.points_mut()
                .insert(pt_idx, SplinePoint::control(new_pt, true));
        }
        self.rebuild_solver();
        self.after_change();
        new_on_curve.id
    }

    pub fn delete(&mut self, id: PointId) -> Option<PointId> {
        let pos = self.idx_for_point(id).unwrap();
        let pt = self.points[pos];
        // deleting a control point deletes *both* control points:
        if pt.is_control() {
            self.points_mut().remove(pos);
            if self.points.get(pos).map(|pt| pt.is_control()) == Some(true) {
                self.points_mut().remove(pos);
            } else {
                self.points_mut().remove(pos - 1);
            }
        } else {
            let total_on_curve = self.points.iter().filter(|pt| pt.is_on_curve()).count();
            if total_on_curve < 4 && self.closed {
                self.points_mut().clear();
                self.after_change();
                return None;
            }

            self.points_mut().remove(pos);
            // on-curve at idx 0 can't be a 'spline to'
            if pos > 0
                && self
                    .points
                    .get(pos - 1)
                    .map(SplinePoint::is_control)
                    .unwrap_or(false)
            {
                self.points_mut().remove(pos - 1);
                self.points_mut().remove(pos - 2);
            }

            // if removing the first point, and it is followed by a splineto,
            // remove the control points
            if !self.is_closed() && self.points.get(0).map(|pt| pt.is_control()) == Some(true) {
                self.points_mut().remove(0);
                self.points_mut().remove(0);
            }
        }

        self.rebuild_solver();
        self.after_change();
        // select the last point on delete?
        self.points().last().map(|pt| pt.id)
    }

    pub fn nudge(&mut self, id: PointId, delta: Vec2) {
        let idx = self.idx_for_point(id).unwrap();
        let new_pos = self.points().get(idx).unwrap().point + delta;
        self.move_point(id, new_pos);
    }

    pub fn close(&mut self, smooth: bool) {
        assert!(!self.closed);
        let first = self.points_mut().remove(0);
        if smooth {
            self.spline_to(first.point, smooth);
        } else {
            self.line_to(first.point, smooth);
        }
        self.closed = true;
        self.rebuild_solver();
        self.after_change();
    }

    pub fn update_for_drag(&mut self, handle: Point) {
        assert!(!self.points.is_empty());
        if self.points.len() > 1 && !self.last_segment_is_curve() {
            self.convert_last_to_curve();
        }
        self.update_trailing(handle);
        self.after_change();
    }

    pub fn move_point(&mut self, id: PointId, new_point: Point) {
        let pos = self.idx_for_point(id).unwrap();
        let point = self.points_mut().get_mut(pos).unwrap();
        point.point = new_point;
        if point.is_auto() {
            point.toggle_type();
        }

        self.rebuild_solver();
        self.after_change();
        //FIXME: if we're smooth, and the opposite handle is non-auto, we should
        //update that handle as well?
    }

    /// If this is an on-curve point, toggle its smoothness
    pub fn toggle_point_type(&mut self, id: PointId) {
        let pos = self
            .idx_for_point(id)
            .expect("selected point always exists");

        self.points_mut().get_mut(pos).unwrap().toggle_type();
        self.rebuild_solver();
        self.after_change();
    }

    pub fn nearest_segment_distance(&self, point: Point) -> f64 {
        self.bezier.segments().fold(f64::MAX, |acc, seg| {
            seg.nearest(point, 0.1).1.sqrt().min(acc)
        })
    }

    pub fn maybe_convert_line_to_spline(&mut self, click: Point, max_dist: f64) {
        if self.points().is_empty() {
            return;
        }
        let mut best = (f64::MAX, 0);
        let mut prev_point = self.first_point().map(|pt| pt.point);
        let n_skip = if self.closed { 0 } else { 1 };
        for (i, pt) in self.points().iter().enumerate().skip(n_skip) {
            if pt.is_control() {
                prev_point = None;
                continue;
            } else {
                if let Some(prev) = prev_point.take() {
                    let line = Line::new(prev, pt.point);
                    let closest = line.nearest(click, 0.1).1.sqrt();
                    if closest < best.0 {
                        best = (closest, i)
                    }
                }
            }
            prev_point = Some(pt.point);
        }

        if best.0 > max_dist {
            return;
        }

        let start_ix = (self.points.len() + best.1 - 1) % self.points.len();
        let start = self.points[start_ix].point;
        let end = self.points[best.1].point;
        let p1 = start.lerp(end, 1.0 / 3.0);
        let p2 = start.lerp(end, 2.0 / 3.0);
        self.points_mut()
            .insert(best.1, SplinePoint::control(p1, true));
        self.points_mut()
            .insert(best.1 + 1, SplinePoint::control(p2, true));

        self.rebuild_solver();
        self.after_change();
    }

    fn after_change(&mut self) {
        if self.points.len() > 1 {
            self.rebuild_spline()
        } else {
            self.bezier = Arc::new(BezPath::default());
        }
    }

    fn idx_for_point(&self, id: PointId) -> Option<usize> {
        self.points.iter().position(|pt| pt.id == id)
    }

    /// rebuilds the solver from scratch, which is easier than trying to
    /// incrementally update it for some operations.
    fn rebuild_solver(&mut self) {
        let mut solver = SplineSpec::new();
        *solver.elements_mut() = self.iter_spline_elements().collect();
        if self.closed {
            solver.close();
        }
        self.solver = solver;
    }

    /// Takes the current solver and updates the position of auto points based
    /// on their position in the resolved spline.
    fn rebuild_spline(&mut self) {
        let Path { solver, points, .. } = self;
        let spline = solver.solve();
        let points = Arc::make_mut(points);
        let mut ix = if self.closed { 0 } else { 1 };
        for segment in spline.segments() {
            if segment.is_line() {
                // I think we do no touchup, here?
                match points.get(ix).map(|pt| pt.type_) {
                    Some(PointType::OnCurve { .. }) => {
                        // expected case
                        ix += 1;
                    }
                    Some(PointType::Control { .. }) => {
                        eprintln!(
                            "segment is line but control points exist, we should delete them?"
                        );
                        ix += 3;
                    }
                    None => panic!("missing point at idx {}", ix),
                };
            } else {
                let p1 = points.get_mut(ix).unwrap();
                if matches!(p1.type_, PointType::Control { auto: true }) {
                    p1.point = segment.p1;
                }
                let p2 = points.get_mut(ix + 1).unwrap();
                if matches!(p2.type_, PointType::Control { auto: true }) {
                    p2.point = segment.p2;
                }
                ix += 3;
            }
        }

        self.bezier = Arc::new(spline.render());

        // and then we want to actually update our stored points:
    }

    fn iter_spline_elements(&self) -> impl Iterator<Item = Element> {
        SplineElementIter::new(self.points.clone(), self.closed)
    }

    fn last_segment_is_curve(&self) -> bool {
        let len = self.points.len();
        len > 2 && matches!(self.points[len - 2].type_, PointType::Control { .. })
    }

    fn convert_last_to_curve(&mut self) {
        if let Some(prev_point) = self.points_mut().pop() {
            assert!(self.trailing.is_none());
            self.solver.elements_mut().pop();
            self.spline_to(prev_point.point, true);
        }
    }

    /// Update the curve while the user drags a new control point.
    fn update_trailing(&mut self, handle: Point) {
        if self.points.len() > 1 {
            let len = self.points.len();
            assert!(matches!(self.points[len - 1].type_, PointType::OnCurve { .. }));
            assert!(matches!(self.points[len - 2].type_, PointType::Control { .. }));
            let on_curve_pt = self.points[len - 1].point;
            let new_p = on_curve_pt - (handle - on_curve_pt);
            self.points_mut()[len - 2].point = new_p;
            self.points_mut()[len - 2].type_ = PointType::Control { auto: false };
            let last_el = self.solver.elements_mut().last_mut().unwrap();
            if let Element::SplineTo(_, p2, _, _) = last_el {
                *p2 = Some(new_p);
            } else {
                panic!("unexpected element {:?}", last_el);
            }
        }
        self.trailing = Some(handle);
    }
}

struct SplineElementIter {
    points: Arc<Vec<SplinePoint>>,
    start: Option<Point>,
    ix: usize,
}

impl SplineElementIter {
    fn new(points: Arc<Vec<SplinePoint>>, closed: bool) -> SplineElementIter {
        let start = if closed {
            points.last()
        } else {
            points.first()
        };
        let start = start.map(|pt| pt.point);
        let ix = if closed { 0 } else { 1 };
        SplineElementIter { points, start, ix }
    }

    fn is_done(&self) -> bool {
        self.points.is_empty() || self.ix == self.points.len()
    }
}

impl Iterator for SplineElementIter {
    type Item = Element;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(start) = self.start.take() {
            return Some(Element::MoveTo(start));
        }
        if self.is_done() {
            return None;
        }

        let next_pt = self.points[self.ix];
        match next_pt.type_ {
            PointType::OnCurve { smooth } => {
                self.ix += 1;
                Some(Element::LineTo(next_pt.point, smooth))
            }
            PointType::Control { auto } => {
                let p1 = if auto { None } else { Some(next_pt.point) };
                let p2 = self
                    .points
                    .get(self.ix + 1)
                    .map(|pt| match pt.type_ {
                        PointType::Control { auto: true } => None,
                        PointType::Control { auto: false } => Some(pt.point),
                        _ => panic!("missing offcurve point: ix {} {:#?}", self.ix, &self.points),
                    })
                    .unwrap();
                let p3 = self.points[self.ix + 2];
                let smooth = match p3.type_ {
                    PointType::OnCurve { smooth } => smooth,
                    _ => panic!("missing on curve point"),
                };
                self.ix += 3;
                Some(Element::SplineTo(p1, p2, p3.point, smooth))
            }
        }
    }
}

use std::sync::Arc;

use druid::kurbo::{BezPath, Line, ParamCurveNearest};
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

    pub fn delete(&mut self, id: PointId) -> Option<PointId> {
        let pos = self.idx_for_point(id).unwrap();
        let pt = self.points[pos];
        if pt.is_control() {
            self.points_mut().remove(pos);
            if self.points.get(pos).map(|pt| pt.is_control()) == Some(true) {
                self.points_mut().remove(pos);
            } else {
                // if the other point in this segment isn't after us, it must be before:
                self.points_mut().remove(pos - 1);
            }
            let (el, _) = self.element_containing_idx_mut(pos);
            if let Element::SplineTo(_, _, point, smooth) = el {
                *el = Element::LineTo(*point, *smooth);
            }
        } else {
            let element_idx = self.idx_for_element_containing_point(pos);
            let removed = self.solver.elements_mut().remove(element_idx);
            if element_idx == 0 {
                if let Some(el) = self.solver.elements_mut().get_mut(0) {
                    *el = Element::MoveTo(el.endpoint());
                }
            }
            self.points_mut().remove(pos);
            if matches!(removed, Element::SplineTo(..)) {
                self.points_mut().remove(pos - 1);
                self.points_mut().remove(pos - 2);
            }
            // if removing the first point, and it is followed by a splineto,
            // remove the control points
            if self.points.get(0).map(|pt| pt.is_control()) == Some(true) {
                self.points_mut().remove(0);
                self.points_mut().remove(0);
            }
        }
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
        let first = self.points.first().cloned().unwrap();
        if smooth {
            self.spline_to(first.point, smooth);
        } else {
            self.line_to(first.point, smooth);
        }
        self.closed = true;
        self.solver.close();
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

        let (elem, idx) = self.element_containing_idx_mut(pos);
        match elem {
            Element::MoveTo(pt) | Element::LineTo(pt, _) => *pt = new_point,
            Element::SplineTo(p1, p2, p3, _) => match idx {
                0 => *p1 = Some(new_point),
                1 => *p2 = Some(new_point),
                2 => *p3 = new_point,
                _ => unreachable!(),
            },
        }
        self.after_change();
        //FIXME: if we're smooth, and the opposite handle is non-auto, we should
        //update that handle as well?
    }

    /// If this is an on-curve point, toggle its smoothness
    pub fn toggle_point_type(&mut self, id: PointId) {
        let pos = self
            .idx_for_point(id)
            .expect("selected point always exists");
        let pt = self.points.get(pos).unwrap();

        let new_auto = match pt.type_ {
            PointType::Control { auto: true } => Some(pt.point),
            _ => None,
        };

        self.points_mut().get_mut(pos).unwrap().toggle_type();
        let (elem, idx) = self.element_containing_idx_mut(pos);
        match elem {
            Element::LineTo(_, smooth) => *smooth = !*smooth,
            Element::SplineTo(p1, p2, _, smooth) => match idx {
                0 => *p1 = new_auto,
                1 => *p2 = new_auto,
                2 => *smooth = !*smooth,
                _ => (),
            },
            _ => (),
        }
        self.after_change();
    }

    /// Given an index into our points array, returns the spline element
    /// containing that index, as well as the position within that element
    /// of the point in question.
    fn element_containing_idx_mut(&mut self, idx: usize) -> (&mut Element, usize) {
        let mut dist_to_pt = idx;
        for element in self.solver.elements_mut().iter_mut() {
            match element {
                Element::MoveTo(..) | Element::LineTo(..) if dist_to_pt == 0 => {
                    return (element, 0)
                }
                Element::LineTo(..) | Element::MoveTo(..) => dist_to_pt -= 1,
                Element::SplineTo(..) if (0..=2).contains(&dist_to_pt) => {
                    return (element, dist_to_pt)
                }
                Element::SplineTo(..) => dist_to_pt = dist_to_pt.saturating_sub(3),
            }
        }
        unreachable!();
    }

    fn idx_for_element_containing_point(&self, idx: usize) -> usize {
        let mut dist_to_pt = idx;
        for (i, element) in self.solver.elements().iter().enumerate() {
            match element {
                Element::MoveTo(..) | Element::LineTo(..) if dist_to_pt == 0 => return i,
                Element::SplineTo(..) if (0..=2).contains(&dist_to_pt) => return i,
                Element::LineTo(..) | Element::MoveTo(..) => dist_to_pt -= 1,
                Element::SplineTo(..) => dist_to_pt = dist_to_pt.saturating_sub(3),
            }
        }
        unreachable!();
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
        let mut prev_point = self.points().first().map(|pt| pt.point);
        for (i, pt) in self.points().iter().enumerate().skip(1) {
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

        // insert two auto points:
        assert!(best.1 > 0);
        let start = self.points[best.1 - 1].point;
        let end = self.points[best.1].point;
        let p1 = start.lerp(end, 1.0 / 3.0);
        let p2 = start.lerp(end, 2.0 / 3.0);
        self.points_mut()
            .insert(best.1, SplinePoint::control(p1, true));
        self.points_mut()
            .insert(best.1 + 1, SplinePoint::control(p2, true));

        // and convert the appropriate solver element to be a splineto:
        let (el, _) = self.element_containing_idx_mut(best.1);
        if let Element::LineTo(p1, smooth) = el {
            *el = Element::SplineTo(None, None, *p1, *smooth);
        } else {
            eprintln!(
                "failed to update element after line->spline conversion: {:?}",
                el
            );
        }
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

    fn rebuild_spline(&mut self) {
        let Path { solver, points, .. } = self;
        let spline = solver.solve();
        let points = Arc::make_mut(points);
        let mut ix = 1;
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
                    None => panic!("missing point at idx {}"),
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

    //fn iter_spline_ops(&self) -> impl Iterator<Item = SplineOp> {
    //SplineOpIter {
    //points: self.points.clone(),
    //ix: 0,
    //}
    //}

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

#[derive(Debug, Clone, Copy)]
enum SplineOp {
    MoveTo(Point),
    LineTo(Point, bool),
    SplineTo(Option<Point>, Option<Point>, Point, bool),
}

struct SplineOpIter {
    points: Arc<Vec<SplinePoint>>,
    ix: usize,
}

impl Iterator for SplineOpIter {
    type Item = SplineOp;
    fn next(&mut self) -> Option<Self::Item> {
        if self.ix == self.points.len() {
            return None;
        }

        let next_pt = self.points[self.ix];
        match next_pt.type_ {
            PointType::OnCurve { smooth } => {
                self.ix += 1;
                if self.ix == 1 {
                    Some(SplineOp::MoveTo(self.points[self.ix - 1].point))
                } else {
                    Some(SplineOp::LineTo(next_pt.point, smooth))
                }
            }
            PointType::Control { auto } => {
                let p1 = if auto { None } else { Some(next_pt.point) };
                let p2 = self
                    .points
                    .get(self.ix + 1)
                    .map(|pt| match pt.type_ {
                        PointType::Control { auto: true } => None,
                        PointType::Control { auto: false } => Some(pt.point),
                        _ => panic!("missing offcurve point"),
                    })
                    .unwrap();
                let p3 = self.points[self.ix + 2];
                let smooth = match p3.type_ {
                    PointType::OnCurve { smooth } => smooth,
                    _ => panic!("missing on curve point"),
                };
                self.ix += 3;
                Some(SplineOp::SplineTo(p1, p2, p3.point, smooth))
            }
        }
    }
}

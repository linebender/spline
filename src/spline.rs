//! A general purpose spline with explicit control.

use std::borrow::Cow;

use kurbo::{Affine, BezPath, PathEl, Point, Vec2};
#[cfg(feature = "serde")]
use serde_::{Deserialize, Serialize};

use crate::hyperbezier::{self, HyperBezier, ThetaParams};
use crate::simple_spline;
use crate::util;

/// The specification of a spline curve.
///
/// Currently this represents a single subpath.
#[derive(Clone, Debug)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(crate = "serde_")
)]
pub struct SplineSpec {
    elements: Vec<Element>,
    is_closed: bool,
    /// The free thetas to solve for.
    ///
    /// There is one of these for each smooth on-curve point with an auto
    /// point on both sides.
    #[cfg_attr(feature = "serde", serde(skip))]
    ths: Vec<f64>,
    #[cfg_attr(feature = "serde", serde(skip))]
    dths: Vec<f64>,
    /// The tentative solution.
    #[cfg_attr(feature = "serde", serde(skip))]
    segments: Vec<Segment>,
    /// `true` if the inputs have changed, and the spline needs to be solved.
    #[cfg_attr(feature = "serde", serde(skip, default = "serde_true"))]
    dirty: bool,
}

#[cfg(feature = "serde")]
fn serde_true() -> bool {
    true
}

/// A solved spline.
///
/// This can be converted to a Bézier path with [`Spline::render`].
#[derive(Clone, Debug)]
pub struct Spline<'spec> {
    segments: Cow<'spec, [Segment]>,
    is_closed: bool,
}

/// A single spline segment.
#[derive(Clone, Debug)]
pub struct Segment {
    pub p0: Point,
    pub p1: Point,
    pub p2: Point,
    pub p3: Point,
    /// Tangent angle relative to chord at start point.
    pub th0: f64,
    /// Tangent angle relative to chord at end point.
    pub th1: f64,
    /// Actual curvature at start point.
    pub k0: f64,
    /// Actual curvature at end point.
    pub k1: f64,
    pub hb: HyperBezier,
    /// Length of unit-arclen hb chord (for curvature).
    ch: f64,
}

/// An imperative description of a spline path.
#[derive(Copy, Clone, Debug)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(crate = "serde_")
)]
pub enum Element {
    /// The start of a spline path.
    MoveTo(Point),
    /// A line-to operation.
    ///
    /// The `bool` indicates whether the operation is smooth, that is whether
    /// or not the curve into and out of the control point maintains consistent
    /// [curvature][smoothness]. This is only relevant if this is followed
    /// by a [`SplineTo`] element.
    ///
    /// [smoothness]: https://en.wikipedia.org/wiki/Smoothness#Smoothness_of_curves_and_surfaces
    /// [`SplineTo`]: [Element::SplineTo]
    LineTo(Point, bool),
    /// A spline-to operation.
    ///
    /// The first two points are like the control or off-curve points in
    /// a Bézier curve; if they are `None` they will be autocomputed by
    /// the solver.
    ///
    /// The `bool` indicates whether the operation is smooth, as per [`LineTo`].
    ///
    /// [`LineTo`]: [Element::LineTo]
    SplineTo(Option<Point>, Option<Point>, Point, bool),
}

impl SplineSpec {
    /// Start a new spline.
    pub fn new() -> SplineSpec {
        SplineSpec {
            elements: Vec::new(),
            is_closed: false,
            ths: Vec::new(),
            dths: Vec::new(),
            segments: Vec::new(),
            dirty: true,
        }
    }

    pub fn move_to(&mut self, p: Point) {
        debug_assert!(self.elements.is_empty());
        self.elements.push(Element::MoveTo(p));
        self.dirty = true;
    }

    pub fn line_to(&mut self, p: Point, is_smooth: bool) {
        debug_assert!(!self.elements.is_empty());
        self.elements.push(Element::LineTo(p, is_smooth));
        self.dirty = true;
    }

    pub fn spline_to(&mut self, p1: Option<Point>, p2: Option<Point>, p3: Point, is_smooth: bool) {
        debug_assert!(!self.elements.is_empty());
        self.elements.push(Element::SplineTo(p1, p2, p3, is_smooth));
        self.dirty = true;
    }

    pub fn close(&mut self) {
        debug_assert!(self.elements.len() > 1);
        self.is_closed = true;
        self.dirty = true;
    }

    pub fn is_closed(&self) -> bool {
        self.is_closed
    }

    pub fn elements(&self) -> &[Element] {
        &self.elements
    }

    /// Return a mutable reference to the elements.
    ///
    /// This can be used to update the elements in place, such as while editing.
    /// This always marks us as dirty; if you do not need mutable access you
    /// should prefer [`SplineSpec::elements`].
    ///
    /// # Note
    ///
    /// It is possible via this method to leave the elements in an inconsistent
    /// state, such as by inserting multiple `MoveTo` elements. Care is advised.
    pub fn elements_mut(&mut self) -> &mut Vec<Element> {
        self.dirty = true;
        &mut self.elements
    }

    /// Returns the current solution, if it is up-to-date.
    ///
    /// If it is not up-to-date, you need to call [`solve`](SplineSpec::solve)
    /// first.
    pub fn segments(&self) -> Option<&[Segment]> {
        if self.dirty {
            None
        } else {
            Some(&self.segments)
        }
    }

    /// Returns the solved spline based on the current elements.
    ///
    /// The returned [`Spline`] borrows data from `self`; if you need an
    /// owned version you can call [`Spline::into_owned`].
    pub fn solve(&mut self) -> Spline {
        if self.dirty {
            self.segments = self.initial_segs();
            self.ths = self.initial_ths();
            self.dths = vec![0.0; self.ths.len()];
            self.update_segs();
            for i in 0..10 {
                let _err = self.iterate(i);
                //eprintln!("err = {}", err);
                self.adjust_tensions(i);
                self.update_segs();
            }
            self.dirty = false;
        }

        Spline {
            segments: Cow::Borrowed(self.segments.as_slice()),
            is_closed: self.is_closed,
        }
    }

    /// Create initial segments.
    fn initial_segs(&self) -> Vec<Segment> {
        if self.elements.len() > 1 {
            let mut p0 = self.elements[0].endpoint();
            (self.elements[1..])
                .iter()
                .map(|el| {
                    let p3 = el.endpoint();
                    let seg = if let &Element::SplineTo(Some(p1), Some(p2), _p3, _) = el {
                        // Both points given, we can compute the final spline segment now.
                        let v = p3 - p0;
                        // This takes (0, 0) to p0 and (1, 0) to p3.
                        let a = Affine::new([v.x, v.y, -v.y, v.x, p0.x, p0.y]);
                        let a_inv = a.inverse();
                        let (th0, bias0) = HyperBezier::params_for_v((a_inv * p1).to_vec2());
                        let (th1, bias1) =
                            HyperBezier::params_for_v(Point::new(1.0, 0.0) - a_inv * p2);
                        // TODO: signs feel reversed here, but it all works out in the end.
                        let theta_params = ThetaParams {
                            th0: -th0,
                            bias0,
                            th1,
                            bias1,
                        };
                        let hb = HyperBezier::solve_for_theta(&theta_params);
                        Segment::make(p0, Some(p1), Some(p2), p3, -th0, th1, hb)
                    } else {
                        Segment::line(p0, p3)
                    };
                    p0 = p3;
                    seg
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    fn initial_ths(&self) -> Vec<f64> {
        let mut ths = Vec::new();
        for i in 1..self.elements.len() {
            if self.elements[i].is_auto_p1()
                && self.prev_el(i).map(Element::is_auto_p2).unwrap_or(false)
            {
                let d0 = self.chord(self.prev_ix(i));
                let d1 = self.chord(i);
                let th0 = d0.atan2();
                let th1 = d1.atan2();
                let bend = util::mod_tau(th1 - th0);
                // This is a bit different than the research spline, but is
                // intended to ensure that the chord angle never exceeds pi/2.
                let th = util::mod_tau(th0 + 0.5 * bend);
                ths.push(th);
            }
        }
        ths
    }

    /// Generate segments from the spline spec and thetas.
    fn update_segs(&mut self) {
        let n_seg = self.segments.len();
        let mut th_ix = 0;
        for i in 0..n_seg {
            if let Element::SplineTo(p1, p2, p3, _is_smooth) = self.elements[i + 1] {
                if p1.is_some() && p2.is_some() {
                    // All parameters already determined from given points.
                    continue;
                }
                let p0 = self.segments[i].p0;
                let v = p3 - p0;
                let a = Affine::new([v.x, v.y, -v.y, v.x, p0.x, p0.y]);
                let a_inv = a.inverse();
                let chord_th = v.atan2();
                let (th0, bias0) = if let Some(p1) = p1 {
                    let v0 = (a_inv * p1).to_vec2();
                    let (th0, bias0) = HyperBezier::params_for_v(v0);
                    (Some(th0), Some(bias0))
                } else {
                    match self.prev_el(i + 1) {
                        Some(Element::SplineTo(_, None, _, _)) => {
                            let th0 = util::mod_tau(self.ths[th_ix] - chord_th);
                            th_ix = (th_ix + 1) % self.ths.len();
                            (Some(th0), None)
                        }
                        Some(Element::SplineTo(_, Some(p2), _, _)) => {
                            let prev_ch_th = (p0 - *p2).atan2();
                            let th0 = util::mod_tau(prev_ch_th - chord_th);
                            (Some(th0), Some(self.segments[i].hb.bias0))
                        }
                        Some(Element::LineTo(..)) => {
                            let prev_seg = &self.segments[(i + n_seg - 1) % n_seg];
                            let prev_ch_th = prev_seg.chord().atan2();
                            let th0 = util::mod_tau(prev_ch_th - chord_th);
                            (Some(th0), Some(0.0))
                        }
                        _ => (None, None),
                    }
                };
                let (th1, bias1) = if let Some(p2) = p2 {
                    let v1 = Point::new(1.0, 0.0) - (a_inv * p2);
                    let (th1, bias1) = HyperBezier::params_for_v(v1);
                    (Some(-th1), Some(bias1))
                } else {
                    match self.next_el(i + 1) {
                        Some(Element::SplineTo(None, _, _, _)) => {
                            let th1 = util::mod_tau(chord_th - self.ths[th_ix]);
                            (Some(th1), None)
                        }
                        Some(Element::SplineTo(Some(p1), _, _, _)) => {
                            let next_ch_th = (*p1 - p3).atan2();
                            let th1 = util::mod_tau(chord_th - next_ch_th);
                            (Some(th1), Some(self.segments[i].hb.bias1))
                        }
                        Some(Element::LineTo(p1, _)) => {
                            let next_ch_th = (*p1 - p3).atan2();
                            let th1 = util::mod_tau(chord_th - next_ch_th);
                            (Some(th1), Some(0.0))
                        }
                        _ => (None, None),
                    }
                };
                let (th0, th1) = match (th0, th1) {
                    (Some(th0), Some(th1)) => (th0, th1),
                    (Some(th0), None) => (th0, simple_spline::endpoint_tangent(th0)),
                    (None, Some(th1)) => (simple_spline::endpoint_tangent(th1), th1),
                    (None, None) => continue,
                };
                let bias0 = bias0.unwrap_or_else(|| simple_spline::bias_for_theta(th0));
                let bias1 = bias1.unwrap_or_else(|| simple_spline::bias_for_theta(th1));
                let params = ThetaParams {
                    th0: -th0,
                    bias0,
                    th1: -th1,
                    bias1,
                };
                let hb = HyperBezier::solve_for_theta(&params);
                self.segments[i] = Segment::make(p0, p1, p2, p3, th0, th1, hb);
            }
        }
    }

    /// Iterate towards reducing error metric.
    ///
    /// Returns the absolute error (after arctan linearization).
    fn iterate(&mut self, iter_ix: usize) -> f64 {
        let mut th_ix = 0;
        let mut abs_err = 0.0;
        for i in 1..self.elements.len() {
            if self.elements[i].is_auto_p1()
                && self.prev_el(i).map(Element::is_auto_p2).unwrap_or(false)
            {
                let prev_seg = &self.segments[self.prev_ix(i) - 1];
                let prev_ch = prev_seg.chord().hypot();
                let seg = &self.segments[i - 1];
                let this_ch = seg.chord().hypot();
                let k_prev = prev_seg.k1;
                let k_this = seg.k0;
                let k_scale = (prev_ch * this_ch).sqrt();
                let k_err = (k_prev * k_scale).atan() - (k_this * k_scale).atan();
                abs_err += k_err.abs();

                // Compute error derivative by differencing. A more sophisticated approach
                // would be analytical derivatives.
                const EPSILON: f64 = 1e-3;
                let th1p = prev_seg.th1 + EPSILON;
                let params0 = ThetaParams {
                    th0: -prev_seg.th0,
                    bias0: prev_seg.hb.bias0,
                    th1: -th1p,
                    bias1: simple_spline::bias_for_theta(th1p),
                };
                let seg0p = HyperBezier::solve_for_theta(&params0);
                let k0p = seg0p.compute().k1 / prev_ch;

                let th0p = seg.th0 - EPSILON;
                let params1 = ThetaParams {
                    th0: -th0p,
                    bias0: simple_spline::bias_for_theta(th0p),
                    th1: -seg.th1,
                    bias1: seg.hb.bias1,
                };
                let seg1p = HyperBezier::solve_for_theta(&params1);
                let k1p = seg1p.compute().k0 / this_ch;

                let k_errp = (k0p * k_scale).atan() - (k1p * k_scale).atan();
                let derr = (k_errp - k_err) * (1.0 / EPSILON);
                //eprintln!("{}: err = {:.3}, derr = {:.3}", i, k_err, derr);
                self.dths[th_ix] = k_err / derr;
                th_ix += 1;
            }
        }
        let scale = (0.25 * (iter_ix as f64 + 1.0)).tanh();
        for (th, dth) in self.ths.iter_mut().zip(&self.dths) {
            *th += scale * dth;
        }
        abs_err
    }

    /// Iterate towards G2 continuity by adjusting bias values.
    fn adjust_tensions(&mut self, iter_ix: usize) {
        const MIN_BIAS: f64 = -0.9;
        let scale = (0.25 * (iter_ix as f64 + 1.0)).tanh();
        for i in 1..self.elements.len() {
            if self.elements[i].is_auto_p1()
                && self.prev_el(i).map(Element::is_given_p2).unwrap_or(false)
            {
                let prev_seg = &self.segments[self.prev_ix(i) - 1];
                let seg = &self.segments[i - 1];
                let this_ch = seg.chord().hypot();
                let bias = hyperbezier::compute_k_inv(prev_seg.k1 * this_ch / (seg.hb.k0 * seg.ch));
                let bias = bias.max(MIN_BIAS);
                let bias = seg.hb.bias0 + scale * (bias - seg.hb.bias0);
                self.segments[i - 1].hb.bias0 = bias;
            }
            if self.elements[i].is_auto_p2()
                && self.next_el(i).map(Element::is_given_p1).unwrap_or(false)
            {
                let next_seg = &self.segments[self.next_ix(i) - 1];
                let seg = &self.segments[i - 1];
                let this_ch = seg.chord().hypot();
                let bias = hyperbezier::compute_k_inv(next_seg.k0 * this_ch / (seg.hb.k1 * seg.ch));
                let bias = bias.max(MIN_BIAS);
                let bias = seg.hb.bias1 + scale * (bias - seg.hb.bias1);
                self.segments[i - 1].hb.bias1 = bias;
            }
        }
    }

    /// The previous element.
    ///
    /// This always gives a result as if the curve were closed, without checking
    /// if that's the case.
    fn prev_ix(&self, i: usize) -> usize {
        if i == 1 {
            self.elements.len() - 1
        } else {
            i - 1
        }
    }

    /// The next element.
    ///
    /// This always gives a result as if the curve were closed, without checking
    /// if that's the case.
    fn next_ix(&self, i: usize) -> usize {
        if i == self.elements.len() - 1 {
            1
        } else {
            i + 1
        }
    }

    /// The previous element that has a G2 continuity constraint.
    fn prev_el(&self, i: usize) -> Option<&Element> {
        let el = &self.elements[self.prev_ix(i)];
        if (i > 1 || self.is_closed) && el.is_smooth() {
            Some(el)
        } else {
            None
        }
    }

    /// The next element that has a G2 continuity constraint.
    fn next_el(&self, i: usize) -> Option<&Element> {
        if (i < self.elements.len() - 1 || self.is_closed) && self.elements[i].is_smooth() {
            Some(&self.elements[self.next_ix(i)])
        } else {
            None
        }
    }

    fn chord(&self, element_ix: usize) -> Vec2 {
        let seg = &self.segments[element_ix - 1];
        seg.p3 - seg.p0
    }
}

impl<'a> Spline<'a> {
    /// Return an owned version of this `Spline`, cloning its data if necessary.
    pub fn into_owned(self) -> Spline<'static> {
        let segments = self.segments.into_owned();
        Spline {
            segments: Cow::Owned(segments),
            is_closed: self.is_closed,
        }
    }

    /// The segments of the spline.
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Render the spline to a Bézier path.
    pub fn render(&self) -> BezPath {
        let mut path = BezPath::new();
        self.render_extend(&mut path);
        path
    }

    /// Render the spline, appending to the given path.
    pub fn render_extend(&self, path: &mut BezPath) {
        path.move_to(self.segments[0].p0);
        for segment in &*self.segments {
            segment.render(path);
        }
        if self.is_closed {
            path.close_path();
        }
    }
}

impl Element {
    fn is_smooth(&self) -> bool {
        match self {
            Element::LineTo(_, is_smooth) => *is_smooth,
            Element::SplineTo(_, _, _, is_smooth) => *is_smooth,
            _ => false,
        }
    }

    pub fn endpoint(&self) -> Point {
        match self {
            Element::MoveTo(p) => *p,
            Element::LineTo(p, _) => *p,
            Element::SplineTo(_, _, p, _) => *p,
        }
    }

    fn is_auto_p1(&self) -> bool {
        matches!(self, Element::SplineTo(None, _, _, _))
    }

    fn is_auto_p2(&self) -> bool {
        matches!(self, Element::SplineTo(_, None, _, _))
    }

    fn is_given_p1(&self) -> bool {
        matches!(self, Element::SplineTo(Some(_), _, _, _))
    }

    fn is_given_p2(&self) -> bool {
        matches!(self, Element::SplineTo(_, Some(_), _, _))
    }
}

impl Segment {
    /// Create a segment from a hyperbezier.
    fn make(
        p0: Point,
        p1: Option<Point>,
        p2: Option<Point>,
        p3: Point,
        th0: f64,
        th1: f64,
        hb: HyperBezier,
    ) -> Segment {
        let r = hb.compute();
        let v = p3 - p0;
        let a = Affine::new([v.x, v.y, -v.y, v.x, p0.x, p0.y]);
        let p1 = p1.unwrap_or_else(|| {
            let p1 = HyperBezier::v_for_params(th0, hb.bias0).to_point();
            a * p1
        });
        let p2 = p2.unwrap_or_else(|| {
            let p2 = Point::new(1.0, 0.0) - HyperBezier::v_for_params(-th1, hb.bias1);
            a * p2
        });
        let k_scale = v.hypot().recip();
        Segment {
            p0,
            p1,
            p2,
            p3,
            th0,
            th1,
            k0: r.k0 * k_scale,
            k1: r.k1 * k_scale,
            hb,
            ch: r.chord,
        }
    }

    fn line(p0: Point, p3: Point) -> Segment {
        Segment {
            p0,
            p1: p0,
            p2: p3,
            p3,
            th0: 0.0,
            th1: 0.0,
            k0: 0.0,
            k1: 0.0,
            hb: HyperBezier {
                k0: 0.0,
                bias0: 1.0,
                k1: 0.0,
                bias1: 1.0,
            },
            ch: 1.0,
        }
    }

    pub fn is_line(&self) -> bool {
        self.hb.k0 == 0.0 && self.hb.k1 == 0.0
    }

    fn chord(&self) -> Vec2 {
        self.p3 - self.p0
    }

    /// Render the segment to the bezier path.
    ///
    /// This does not include the initial moveto, so the caller needs to
    /// supply that separately.
    pub fn render(&self, path: &mut BezPath) {
        path.extend(self.render_elements())
    }

    /// Returns an iterator over the bezier elements that render this segment.
    pub fn render_elements<'a>(&'_ self) -> impl Iterator<Item = PathEl> + '_ {
        // we need to do some gymnastics to enesure we return the same concrete type in
        // both cases:
        let (line_part, spline_part) = if self.is_line() {
            (Some(PathEl::LineTo(self.p3)), None)
        } else {
            let p = self.p0;
            let d = self.p3 - p;
            let a = Affine::new([d.x, d.y, -d.y, d.x, p.x, p.y]);
            (
                None,
                Some(
                    self.hb
                        .render_elements(self.hb.render_subdivisions())
                        .skip(1)
                        .map(move |el| a * el),
                ),
            )
        };

        line_part
            .into_iter()
            .chain(spline_part.into_iter().flatten())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_spline_doesnt_crash() {
        let mut spec = SplineSpec::new();
        spec.solve();
    }

    #[test]
    fn render_elements_count() {
        let mut spec = SplineSpec::new();
        spec.move_to(Point::new(0., 0.));
        spec.spline_to(Some(Point::new(20., 40.)), None, Point::new(100., 0.), true);
        let spline = spec.solve();
        assert_eq!(spline.segments().len(), 1);
        let elements_count = spline.segments().first().unwrap().render_elements().count();
        assert!(elements_count < 64);
    }

    /// Return a [`BezPath`] representing this segment.
    pub fn to_bezier(&self) -> BezPath {
        let mut path = BezPath::new();
        path.move_to(self.p0);
        self.render(&mut path);
        path
    }
}

// Copyright 2024 the Spline Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

// A Javascript test bed for experimenting with parameter fitting.

const GAUSS_LEGENDRE_COEFFS_32 = [
    0.0965400885147278, -0.0483076656877383,
    0.0965400885147278, 0.0483076656877383,
    0.0956387200792749, -0.1444719615827965,
    0.0956387200792749, 0.1444719615827965,
    0.0938443990808046, -0.2392873622521371,
    0.0938443990808046, 0.2392873622521371,
    0.0911738786957639, -0.3318686022821277,
    0.0911738786957639, 0.3318686022821277,
    0.0876520930044038, -0.4213512761306353,
    0.0876520930044038, 0.4213512761306353,
    0.0833119242269467, -0.5068999089322294,
    0.0833119242269467, 0.5068999089322294,
    0.0781938957870703, -0.5877157572407623,
    0.0781938957870703, 0.5877157572407623,
    0.0723457941088485, -0.6630442669302152,
    0.0723457941088485, 0.6630442669302152,
    0.0658222227763618, -0.7321821187402897,
    0.0658222227763618, 0.7321821187402897,
    0.0586840934785355, -0.7944837959679424,
    0.0586840934785355, 0.7944837959679424,
    0.0509980592623762, -0.8493676137325700,
    0.0509980592623762, 0.8493676137325700,
    0.0428358980222267, -0.8963211557660521,
    0.0428358980222267, 0.8963211557660521,
    0.0342738629130214, -0.9349060759377397,
    0.0342738629130214, 0.9349060759377397,
    0.0253920653092621, -0.9647622555875064,
    0.0253920653092621, 0.9647622555875064,
    0.0162743947309057, -0.9856115115452684,
    0.0162743947309057, 0.9856115115452684,
    0.0070186100094701, -0.9972638618494816,
    0.0070186100094701, 0.9972638618494816,
];

class Point {
    constructor(x, y) {
        this.x = x;
        this.y = y;
    }

    lerp(p2, t) {
        return new Point(this.x + (p2.x - this.x) * t, this.y + (p2.y - this.y) * t);
    }

    dist(p2) {
        return Math.hypot(p2.x - this.x, p2.y - this.y);
    }

    hypot2() {
        return this.x * this.x + this.y * this.y;
    }

    hypot() {
        return Math.sqrt(this.hypot2());
    }

    dot(other) {
        return this.x * other.x + this.y * other.y;
    }

    cross(other) {
        return this.x * other.y - this.y * other.x;
    }

    plus(other) {
        return new Point(this.x + other.x, this.y + other.y);
    }

    minus(other) {
        return new Point(this.x - other.x, this.y - other.y);
    }

    atan2() {
        return Math.atan2(this.y, this.x);
    }
}

class CubicBez {
    /// Argument is array of coordinate values [x0, y0, x1, y1, x2, y2, x3, y3].
    constructor(coords) {
        this.c = coords;
    }

    static from_pts(p0, p1, p2, p3) {
        const c = new Float64Array(8);
        c[0] = p0.x;
        c[1] = p0.y;
        c[2] = p1.x;
        c[3] = p1.y;
        c[4] = p2.x;
        c[5] = p2.y;
        c[6] = p3.x;
        c[7] = p3.y;
        return new CubicBez(c);
    }

    p0() {
        return new Point(this.c[0], this.c[1]);
    }

    p1() {
        return new Point(this.c[2], this.c[3]);
    }

    p2() {
        return new Point(this.c[4], this.c[5]);
    }

    p3() {
        return new Point(this.c[6], this.c[7]);
    }

    to_svg_path() {
        const c = this.c;
        return `M${c[0]} ${c[1]}C${c[2]} ${c[3]} ${c[4]} ${c[5]} ${c[6]} ${c[7]}`
    }

    weightsum(c0, c1, c2, c3) {
        const x = c0 * this.c[0] + c1 * this.c[2] + c2 * this.c[4] + c3 * this.c[6];
        const y = c0 * this.c[1] + c1 * this.c[3] + c2 * this.c[5] + c3 * this.c[7];
        return new Point(x, y);
    }

    eval(t) {
        const mt = 1 - t;
        const c0 = mt * mt * mt;
        const c1 = 3 * mt * mt * t;
        const c2 = 3 * mt * t * t;
        const c3 = t * t * t;
        return this.weightsum(c0, c1, c2, c3);
    }

    eval_deriv(t) {
        const mt = 1 - t;
        const c0 = -3 * mt * mt;
        const c3 = 3 * t * t;
        const c1 = -6 * t * mt - c0;
        const c2 = 6 * t * mt - c3;
        return this.weightsum(c0, c1, c2, c3);
    }

    // quadratic bezier with matching endpoints and minimum max vector error
    midpoint_quadbez() {
        const p1 = this.weightsum(-0.25, 0.75, 0.75, -0.25);
        return new QuadBez(this.c[0], this.c[1], p1.x, p1.y, this.c[6], this.c[7]);
    }

    subsegment(t0, t1) {
        let c = new Float64Array(8);
        const p0 = this.eval(t0);
        const p3 = this.eval(t1);
        c[0] = p0.x;
        c[1] = p0.y;
        const scale = (t1 - t0) / 3;
        const d1 = this.eval_deriv(t0);
        c[2] = p0.x + scale * d1.x;
        c[3] = p0.y + scale * d1.y;
        const d2 = this.eval_deriv(t1);
        c[4] = p3.x - scale * d2.x;
        c[5] = p3.y - scale * d2.y;
        c[6] = p3.x;
        c[7] = p3.y;
        return new CubicBez(c);
    }

    area() {
        const c = this.c;
        return (c[0] * (6 * c[3] + 3 * c[5] + c[7])
            + 3 * (c[2] * (-2 * c[1] + c[5] + c[7]) - c[4] * (c[1] + c[3] - 2 * c[7]))
            - c[6] * (c[1] + 3 * c[3] + 6 * c[5]))
            * 0.05;
    }

    chord() {
        return new Line([this.c[0], this.c[1], this.c[6], this.c[7]]);
    }

    deriv() {
        const c = this.c;
        return new QuadBez(
            3 * (c[2] - c[0]), 3 * (c[3] - c[1]),
            3 * (c[4] - c[2]), 3 * (c[5] - c[3]),
            3 * (c[6] - c[4]), 3 * (c[7] - c[5])
        );
    }

    // A pretty good algorithm; kurbo does more sophisticated error analysis.
    arclen(accuracy) {
        return this.arclen_rec(accuracy, 0);
    }

    arclen_rec(accuracy, depth) {
        const c = this.c;
        const d03x = c[6] - c[0];
        const d03y = c[7] - c[1];
        const d01x = c[2] - c[0];
        const d01y = c[3] - c[1];
        const d12x = c[4] - c[2];
        const d12y = c[5] - c[3];
        const d23x = c[6] - c[4];
        const d23y = c[7] - c[5];
        const lp_lc = Math.hypot(d01x, d01y) + Math.hypot(d12x, d12y)
            + Math.hypot(d23x, d23y) - Math.hypot(d03x, d03y);
        const dd1x = d12x - d01x;
        const dd1y = d12y - d01y;
        const dd2x = d23x - d12x;
        const dd2y = d23y - d12y;
        const dmx = 0.25 * (d01x + d23x) + 0.5 * d12x;
        const dmy = 0.25 * (d01y + d23y) + 0.5 * d12y;
        const dm1x = 0.5 * (dd2x + dd1x);
        const dm1y = 0.5 * (dd2y + dd1y);
        const dm2x = 0.25 * (dd2x - dd1x);
        const dm2y = 0.25 * (dd2y - dd1y);
        const co_e = GAUSS_LEGENDRE_COEFFS_8;
        let est = 0;
        for (let i = 0; i < co_e.length; i += 2) {
            const xi = co_e[i + 1];
            const dx = dmx + dm1x * xi + dm2x * (xi * xi);
            const dy = dmy + dm1y * xi + dm2y * (xi * xi);
            const ddx = dm1x + dm2x * (2 * xi);
            const ddy = dm1y + dm2y * (2 * xi);
            est += co_e[i] * (ddx * ddx + ddy * ddy) / (dx * dx + dy * dy);
        }
        const est3 = est * est * est;
        const est_gauss8_err = Math.min(est3 * 2.5e-6, 3e-2) * lp_lc;
        let co = null;
        if (Math.min(est3 * 2.5e-6, 3e-2) * lp_lc < accuracy) {
            co = GAUSS_LEGENDRE_COEFFS_8_HALF;
        } else if (Math.min(est3 * est3 * 1.5e-11, 9e-3) * lp_lc < accuracy) {
            co = GAUSS_LEGENDRE_COEFFS_16_HALF;
        } else if (Math.min(est3 * est3 * est3 * 3.5e-16, 3.5e-3) * lp_lc < accuracy
            || depth >= 20)
        {
            co = GAUSS_LEGENDRE_COEFFS_24_HALF;
        } else {
            const c0 = this.subsegment(0, 0.5);
            const c1 = this.subsegment(0.5, 1);
            return c0.arclen_rec(accuracy * 0.5, depth + 1)
                + c1.arclen_rec(accuracy * 0.5, depth + 1);
        }
        let sum = 0;
        for (let i = 0; i < co.length; i += 2) {
            const xi = co[i + 1];
            const wi = co[i];
            const dx = dmx + dm2x * (xi * xi);
            const dy = dmy + dm2y * (xi * xi);
            const dp = Math.hypot(dx + dm1x * xi, dy + dm1y * xi);
            const dm = Math.hypot(dx - dm1x * xi, dy - dm1y * xi);
            sum += wi * (dp + dm);
        }
        return 1.5 * sum;
    }

    inv_arclen(s, accuracy) {
        if (s <= 0) {
            return 0;
        }
        const total_arclen = this.arclen(accuracy);
        if (s >= total_arclen) {
            return 1;
        }
        // For simplicity, measure arclen from 0 rather than stateful delta.
        const f = t => this.subsegment(0, t).arclen(accuracy) - s;
        const epsilon = accuracy / total_arclen;
        return solve_itp(f, 0, 1, epsilon, 1, 2, -s, total_arclen -s);
    }

    // Find intersections of ray from point p with tangent d
    intersect_ray(p, d) {
        const c = this.c
        const px0 = c[0];
        const px1 = 3 * c[2] - 3 * c[0];
        const px2 = 3 * c[4] - 6 * c[2] + 3 * c[0];
        const px3 = c[6] - 3 * c[4] + 3 * c[2] - c[0];
        const py0 = c[1];
        const py1 = 3 * c[3] - 3 * c[1];
        const py2 = 3 * c[5] - 6 * c[3] + 3 * c[1];
        const py3 = c[7] - 3 * c[5] + 3 * c[3] - c[1];
        const c0 = d.y * (px0 - p.x) - d.x * (py0 - p.y);
        const c1 = d.y * px1 - d.x * py1;
        const c2 = d.y * px2 - d.x * py2;
        const c3 = d.y * px3 - d.x * py3;
        return solve_cubic(c0, c1, c2, c3).filter(t => t > 0 && t < 1);
    }
}

// For curve \kappa = 1/(1-x^2)^2, in range -x..x, what is the expected height?
function forward_scale(x) {
    if (x == 0.0) { return 1.0; }
    if (Math.abs(x) == 1.0) { return 0.0; }
    //const a = (1 - x * x) * Math.atanh(x);
    //return 2 * a / (x + a);
    const u = 1 - x * x;
    return 2 * (Math.sqrt(u) - u) / (x * x);
}

// Simple inverse of forward_scale
function inv_scale(y) {
    return Math.sqrt((4 - 4 * y)/(y * y - 4 * y + 4));
    let a = 0;
    let b = 1;
    for (let i = 0; i < 20; i++) {
        let m = (a + b) * 0.5;
        if (forward_scale(m) > y) {
            a = m;
        } else {
            b = m;
        }
    }
    const c = Math.sqrt((4 - 4 * y)/(y * y - 4 * y + 4));
    console.log(c, (a + b) * 0.5);
    return (a + b) * 0.5;
}

// Compute slope / value ratio for endpoint
function inv_scale_slope(y) {
    return 16 * (1 - 1 * y) / (y * y);
}

function k_for_tension(t) {
    const b = 0.25;
    return 1 / (t * t * (b + (1 - b) * t));
}

function endk_for_quadratic(c, d) {
    const dis = 4 * c - d * d;
    //console.log('evaluating', c, d, 'dis', dis);
    const integral = (4 * c + 2 * d) / (dis * Math.sqrt(c + d + 1)) - (2 * d / dis);
    const k0 = 1 / integral;
    const k1 = k0 * Math.pow(c + d + 1, -1.5);
    const gamma = Math.cbrt(k1 / k0);
    const beta = 1 / (gamma * gamma) - 1;
    const inv_k0 = ((-2 * gamma - 2) * d + 4 * beta * gamma) / (-d * d - 4 * d + 4 * beta);
    //console.log('kk', inv_k0, 1/k0);
    //console.log('qu', 1/k0 * d * d + (-2 * gamma - 2 + 4/k0) * d + 4 * beta * (gamma - 1/k0));
    //console.log('dd', (-2 * gamma - 2) * d + 4 * beta * gamma);
    return {k0: k0, k1: k1};
}

function copysign(x, y) {
    const a = Math.abs(x);
    return y < 0 ? -a : a;
}

function solve_quadratic(c0, c1, c2) {
    const sc0 = c0 / c2;
    const sc1 = c1 / c2;
    if (!(isFinite(sc0) && isFinite(sc1))) {
        const root = -c0 / c1;
        if (isFinite(root)) {
            return [root];
        } else if (c0 == 0 && c1 == 0) {
            return [0];
        } else {
            return [];
        }
    }
    const arg = sc1 * sc1 - 4 * sc0;
    let root1 = 0;
    if (isFinite(arg)) {
        if (arg < 0) {
            return [];
        } else if (arg == 0) {
            return [-0.5 * sc1];
        }
        root1 = -.5 * (sc1 + copysign(Math.sqrt(arg), sc1));
    } else {
        root1 = -sc1;
    }
    const root2 = sc0 / root1;
    if (isFinite(root2)) {
        if (root2 > root1) {
            return [root1, root2];
        } else {
            return [root2, root1];
        }
    }
    return [root1];
}

// Solve quadratic parameters for given k0's normalized to integral
function quadratic_for_endk(k0, k1) {
    const gamma = Math.cbrt(k1 / k0);
    const beta = 1 / (gamma * gamma) - 1;
    const d = (gamma * k0 - 1) * (2 * gamma + 2) / gamma;
    /*
    const c0 = 4 * beta * (gamma - ik0);
    const c1 = -2 * gamma - 2 + 4 * ik0;
    const c2 = ik0;
    const roots = solve_quadratic(c0, c1, c2);
    let best_err = 1e9;
    let best_d = 0;
    for (var cand_d of roots) {
        let endk = endk_for_quadratic(beta - cand_d, cand_d);
        const err = Math.pow(endk.k0 - k0, 2) + Math.pow(endk.k1 - k1, 2);
        if (err < best_err) {
            best_d = cand_d;
            best_err = err;
        }
    }
    const d = best_d;
    const c = beta - d;
    const wrong_root = 4 * beta * gamma / (2 * gamma + 2);
    const other_root = c0 / (c2 * wrong_root);
    console.log('other root', other_root, fast);
    */
    return {c: beta - d, d: d};
}

// A curve defined by \kappa = (as + b) / (cs^2 + ds + e)^1.5
class Hb {
    constructor(a, b, c, d, e) {
        this.a = a;
        this.b = b;
        this.c = c;
        this.d = d;
        this.e = e;
        const denom = 2 / (4 * c - d * d);
        const beta0 = d * denom;
        this.num0 = a * (-1 - 0.5 * d * beta0) / c + b * beta0;
        this.num1 = (2 * b * c - d * a) * denom;

    }

    int_helper(t) {
        // assumes this.e = 1
        const q = this.c * t * t + this.d * t + 1;
        return (this.num0 + this.num1 * t) / Math.sqrt(q);
    }

    theta(s) {
        const analytical = this.int_helper(s) - this.num0;
        /*
        const n = 4;
        let theta = 0;
        let co = GAUSS_LEGENDRE_COEFFS_32;
        for (let j = 0; j < n; j++) {
            for (let i = 0; i < co.length; i += 2) {
                const xi = co[i + 1];
                const wi = co[i];
                const t = ((0.5 + 0.5 * xi) + j) / n * s;
                const q = this.c * t * t + this.d * t + this.e;
                const k = (this.a * t + this.b) / (q * Math.sqrt(q));
                theta += wi * k;
            }
        }
        const numerical = 0.5 * s * theta / n;
        console.log(numerical, analytical);
        */
        return analytical;
    }

    eval_q(t) {
        const q = this.c * t * t + this.d * t + this.e;
        const k = (this.a * t + this.b) / (q * Math.sqrt(q));
        return {q: q, k: k};
    }

    report_endpoints() {
        let co = GAUSS_LEGENDRE_COEFFS_32;
        let sum = 0;
        for (let i = 0; i < co.length; i += 2) {
            const xi = co[i + 1];
            const wi = co[i];
            const t = 0.5 + 0.5 * xi;
            const q = this.eval_q(t).q;
            sum += wi * Math.pow(q, -1.5);
        }
        const integral = 0.5 * sum;
        const q0 = this.eval_q(0);
        const q1 = this.eval_q(1);
        console.log(Math.pow(q0.q, -1.5) / integral, Math.pow(q1.q, -1.5) / integral);
    }

    integrate(s) {
        let x = 0;
        let y = 0;
        let co = GAUSS_LEGENDRE_COEFFS_32;
        for (let i = 0; i < co.length; i += 2) {
            const xi = co[i + 1];
            const wi = co[i];
            const t = (0.5 + 0.5 * xi) * s;
            const theta = this.theta(t);
            x += wi * Math.cos(theta);
            y += wi * Math.sin(theta);
        }
        return {x: 0.5 * s * x, y: 0.5 * s * y};
    }

    // Return list of points normalized to unit chord
    plot() {
        const p1 = this.integrate(1);
        const scale = 1.0 / (p1.x * p1.x + p1.y * p1.y);
        const x = scale * p1.x;
        const y = scale * p1.y;
        const n = 50;
        const result = [];
        for (let i = 0; i <= n; i++) {
            const s = i / n;
            const p = this.integrate(s);
            const p_rot = {x: p.x * x + p.y * y, y: p.y * x - p.x * y};
            result.push(p_rot);
        }
        return result;
    }
}

function make_hb(a, b, c0, c1) {
    const c = -(c0 + c1) * (c0 + c1);
    const d = 2 * c0 * (c0 + c1);
    const e = 1 - c0 * c0;
    return new Hb(a, b, c, d, e);
}

function calc_thetas(a, b, c, d, e) {
    const hb = new Hb(a, b, c, d, e);
    const p = hb.integrate(1);
    const th0 = Math.atan2(p.y, p.x);
    const th1 = hb.theta(1) - th0;
    return {th0: th0, th1: th1};
}

function solve_thetas(th0, th1, c, d, e) {
    let a = 0;
    let b = 0;
    for (let i = 0; i < 10; i++) {
        const ths = calc_thetas(a, b, c, d, e);
        const dth0 = ths.th0 - th0;
        const dth1 = ths.th1 - th1;
        if (Math.abs(dth0) < 1e-9 && Math.abs(dth1) < 1e-1) {
            break;
        }
        const EPS = 1e-6;
        const IEPS = 0.5 / EPS;
        const tap = calc_thetas(a + EPS, b, c, d, e);
        const tam = calc_thetas(a - EPS, b, c, d, e);
        const tbp = calc_thetas(a , b + EPS, c, d, e);
        const tbm = calc_thetas(a , b - EPS, c, d, e);
        const dth0da = IEPS * (tap.th0 - tam.th0);
        const dth1da = IEPS * (tap.th1 - tam.th1);
        const dth0db = IEPS * (tbp.th0 - tbm.th0);
        const dth1db = IEPS * (tbp.th1 - tbm.th1);
        const det = dth0da * dth1db - dth1da * dth0db;
        const da = (dth0 * dth1db - dth1 * dth0db) / det;
        const db = (dth0da * dth1 - dth1da * dth0) / det;
        a -= da;
        b -= db;
        //console.log(dth0, dth1, a, b);
    }
    return {a: a, b: b};
}

const svgNS = "http://www.w3.org/2000/svg";

// One manipulable cubic bezier
class CubicUi {
    constructor(ui, pts) {
        this.ui = ui
        this.pts = pts;
        this.curve = ui.make_stroke();
        this.curve.classList.add("quad");
        this.hull = ui.make_stroke();
        this.hull.classList.add("hull");
        this.handles = [];
        for (let pt of pts) {
            this.handles.push(ui.make_handle(pt));
        }
    }

    onPointerDown(e) {
        const pt = this.ui.getCoords(e);
        const x = pt.x;
        const y = pt.y;
        for (let i = 0; i < this.pts.length; i++) {
            if (Math.hypot(x - this.pts[i].x, y - this.pts[i].y) < 10) {
                this.current_obj = i;
                return true;
            }
        }
        return false;
    }

    onPointerMove(e) {
        const i = this.current_obj;
        const pt = this.ui.getCoords(e);
        this.pts[i] = pt;
        this.handles[i].setAttribute("cx", pt.x);
        this.handles[i].setAttribute("cy", pt.y);
    }

    getCubic() {
        const p0 = this.pts[0];
        const p1 = this.pts[1];
        const p2 = this.pts[2];
        const p3 = this.pts[3];
        let c = new Float64Array(8);
        c[0] = p0.x;
        c[1] = p0.y;
        c[2] = p1.x;
        c[3] = p1.y;
        c[4] = p2.x;
        c[5] = p2.y;
        c[6] = p3.x;
        c[7] = p3.y;
        return new CubicBez(c);
    }

    update() {
        const cb = this.getCubic();
        const pts = this.pts;
        this.curve.setAttribute("d", cb.to_svg_path());
        const h = `M${pts[0].x} ${pts[0].y}L${pts[1].x} ${pts[1].y}M${pts[2].x} ${pts[2].y}L${pts[3].x} ${pts[3].y}`;
        this.hull.setAttribute("d", h);
    }
}

class Ui {
    constructor(id) {
        this.root = document.getElementById(id);
        this.root.addEventListener("pointerdown", e => {
            this.root.setPointerCapture(e.pointerId);
            this.onPointerDown(e);
            e.preventDefault();
            e.stopPropagation();
        });
        this.root.addEventListener("pointermove", e => {
            this.onPointerMove(e);
            e.preventDefault();
            e.stopPropagation();
        });
        this.root.addEventListener("pointerup", e => {
            this.root.releasePointerCapture(e.pointerId);
            this.onPointerUp(e);
            e.preventDefault();
            e.stopPropagation();
        });

        const pts = [new Point(50, 300), new Point(100, 100), new Point(200, 100), new Point(550, 300)];
        this.cubic = new CubicUi(this, pts);

        this.hb = new Hb(0, 1, 0, 0, 1);
        this.main = this.make_stroke();
        this.update();
        // document.getElementById('a').addEventListener('input', e => this.update());
        // document.getElementById('b').addEventListener('input', e => this.update());
        // document.getElementById('c').addEventListener('input', e => this.update());
        // document.getElementById('d').addEventListener('input', e => this.update());
        // document.getElementById('e').addEventListener('input', e => this.update());
    }

    getCoords(e) {
        const rect = this.root.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const y = e.clientY - rect.top;
        return new Point(x, y);
    }

    onPointerDown(e) {
        const pt = this.getCoords(e);
        const x = pt.x;
        const y = pt.y;
        if (this.cubic.onPointerDown(e)) {
            this.current_obj = 'cubic';
            return;
        }
    }

    onPointerMove(e) {
        // Maybe use object oriented dispatch?
        if (this.current_obj == 'cubic') {
            this.cubic.onPointerMove(e);
            this.update();
        }
        const pt = this.getCoords(e);
    }

    onPointerUp(e) {
        this.current_obj = null;
    }

    make_handle(p) {
        const circle = this.plot(p.x, p.y, "blue", 4);
        circle.classList.add("handle");
        return circle;
    }

    make_stroke() {
        const path = document.createElementNS(svgNS, "path");
        path.setAttribute("fill", "none");
        path.setAttribute("stroke", "blue");
        this.root.appendChild(path);
        return path;
    }

    plot(x, y, color = "black", r = 2) {
        let circle = document.createElementNS(svgNS, "circle");
        circle.setAttribute("cx", x);
        circle.setAttribute("cy", y);
        circle.setAttribute("r", r);
        circle.setAttribute("fill", color)
        this.root.appendChild(circle);
        return circle;
    }

    update() {
        // const a = parseFloat(document.getElementById('a').value);
        // document.getElementById('aval').textContent = `${a}`;
        // const b = parseFloat(document.getElementById('b').value);
        // document.getElementById('bval').textContent = `${b}`;
        // const c_in = parseFloat(document.getElementById('c').value);
        // document.getElementById('cval').textContent = `${c_in}`;
        // const d_in = parseFloat(document.getElementById('d').value);
        // document.getElementById('dval').textContent = `${d_in}`;
        // const e_in = parseFloat(document.getElementById('e').value);
        // document.getElementById('eval').textContent = `${e_in}`;
        // const c0 = c_in / d_in;
        // const c1 = e_in / d_in;
        // const c = 3 * c0 + c1 - 4;
        // const d = 4 * (1 - c0);
        // const e = c0;
        this.cubic.update();
        this.control_to_hb();
        this.draw_hb();
    }

    control_to_hb() {
        const pts = this.cubic.pts;
        const chord = pts[3].minus(pts[0]).hypot();
        const dx0 = pts[1].x - pts[0].x;
        const dy0 = pts[1].y - pts[0].y;
        const dx1 = pts[2].x - pts[3].x;
        const dy1 = pts[2].y - pts[3].y;
        const th0 = Math.atan2(-dy0, dx0);
        const th1 = Math.atan2(-dy1, -dx1);
        let tens0 = Math.hypot(dx0, dy0) / chord * 1.5 * (Math.cos(th0) + 1);
        let tens1 = Math.hypot(dx1, dy1) / chord * 1.5 * (Math.cos(th1) + 1);
        let d0 = Math.hypot(dx0, dy0) / chord;
        let d1 = Math.hypot(dx1, dy1) / chord;
        var k0 = k_for_tension(tens0);
        var k1 = k_for_tension(tens1);
        let cbr = Math.pow(k0 / k1, 1. / 3.);
        function soft(x) {
            return Math.sqrt(0.5 * (1 + x * x));
        }
        k1 = k1 / soft(cbr);
        k0 = k0 / soft(1 / cbr);

        let dc = pts[2].minus(pts[1]).hypot() / chord;
        let kmid = Math.pow(dc, 1.5);
        let ratio = Math.pow(d0 / d1, 1.5);
        let blend = 0.5 + 0.5 * Math.tanh(3. - 10. * dc);
        k0 += blend * (kmid / ratio - k0);
        k1 += blend * (kmid * ratio - k1);
        const cd = quadratic_for_endk(k0, k1);
        //console.log('dc', dc, 'c', cd.c, 'd', cd.d);
        const endk = endk_for_quadratic(cd.c, cd.d);
        console.log(dc, k0, k1, blend);
        const ab = solve_thetas(th0, th1, cd.c, cd.d, 1);
        this.hb = new Hb(ab.a, ab.b, cd.c, cd.d, 1);
    }

    draw_hb() {
        const x0 = 50;
        const y0 = 300;
        const scale = 500;
        let s = ``;
        let cmd = 'M';
        for (let p of this.hb.plot()) {
            const x = x0 + scale * p.x;
            const y = y0 + scale * p.y;
            s += `${cmd}${x} ${y}`;
            cmd = 'L';
        }
        this.main.setAttribute('d', s);
    }
}

new Ui("s");

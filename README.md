# A spline for interactive curve design.

This crate implements a new spline designed and optimized for interactive design of 2D curves. A major motivation is fonts, but it can be used in other domains.

The work builds on previous iterations, notably the [Spiro] spline, and then another [research spline].

## Hyperbeziers

The major innovation of this spline is the "hyperbezier" curve family. Like cubic Béziers and the Spiro curve, it is a four-parameter curve family. In fact, it's closely based on Spiro and there is significant overlap of the parameter space, including Euler spirals.

There is a significant difference, however. In the Spiro curve family, curvature is bounded, so it is not capable of cusp-like behavior. Rather, when "pushed," Spiro tends to wiggly, Shmoo-like shapes. Béziers are of course capable of high curvature regions, as are elastica when placed under very high tension.

A good way to parametrize the hyperbezier is by tangent angle and "tension," which correlates strongly with curvature at the endpoint. At low tension, the hyperbezier is equivalent to the Spiro curve. A natural tension value produces the Euler spiral (curvature is a linear function of arclength). But for higher tension values, a different function takes over, which approaches a cusp at the endpoint as tension increases.

Unlike Béziers, the cusp happens *only* at the endpoint. Curvature maxima in the interior of a curve are ugly. With the hyperbezier, if the designer wants a sharp curvature maximum, simply place an on-curve point there.

A particular strength of the hyperbezier is smooth (G2-continuous) transitions from straight to curved sections. The hyperbezier is capable (unlike a cubic Bézier) of an S-shaped curve with zero curvature at both ends. It's also capable of a wide range of Euler spiral like behavior where one end has zero curvature and the other is a nice rounded shape (in the general case a designer would use at least two Béziers to create this effect).

The name "hyperbezier" clearly references its roots in the [cubic Bézier][A Primer on Bézier Curves], and the "hyper" part is a reference to the fact that the Euler spiral, an important section of its parameter space, is an instance of the [Hypergeometric function].

## Focus on UX

A persistent challenge with spline-based curve design is getting the UX right. Bézier curves are not easy to master, but the [pen tool] has become highly refined over time, and is an extremely productive interface for designers. A major motivation for this work is to retain the good parts of the Bézier UX.

In particular, the "control handle" maps to hyperbezier parameters in a natural, intuitive way. The tangent angle is obvious, and tension similarly dependent on the length of the control arm. So it's completely valid to use hyperbeziers simply as a drop-in replacement for Béziers.

The intended UX for use as an interpolating spline is simply to designate a control point as "auto." As is traditional, the spline solver solves these for G2 continuity. Where the tension is a free parameter (which generally happens when there is an "auto" point on either side of an on-curve point), it is assigned a reasonable default, in particular the Euler spiral value for small to medium deviations, and a value similar to the research spline as the deviation increases.

To further refine a curve, the designer can click on an auto point and drag it to the desired location. That gesture enforces tangents at extrema, and in general allows for fine tuning of tension, for example to make quadrants more superelliptical (a strength of Bézier editing and a relative weakness of Spiro).

Note: as of this release, the interpolating spline is still work in progress.

[Spiro]: https://github.com/raphlinus/spiro
[research spline]: https://github.com/raphlinus/spline-research
[pen tool]: https://medium.com/@trenti/the-mighty-pen-tool-6b44ff1c32d
[Hypergeometric function]: https://en.wikipedia.org/wiki/Hypergeometric_function
[A Primer on Bézier Curves]: https://pomax.github.io/bezierinfo/

/// Normalize an angle to the range -PI..PI.
pub fn mod_tau(x: f64) -> f64 {
    // Do this in terms of euclidean remainder instead?
    x - std::f64::consts::TAU * (x * (1.0 / std::f64::consts::TAU)).round()
}

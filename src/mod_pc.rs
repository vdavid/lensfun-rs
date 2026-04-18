//! Perspective correction kernels and Jacobi SVD.
//!
//! Port of `libs/lensfun/mod-pc.cpp`. The math comes verbatim from upstream:
//! a two-sided Jacobi SVD (Rasmussen 1996) drives an ellipse fit; the result
//! feeds a chain of rotations that map control points back into the world's
//! horizontals and verticals.
//!
//! # Float discipline
//!
//! All matrix work and the SVD itself runs in `f64`, mirroring upstream's
//! `double`. The kernel I/O (`apply_correction_kernel`, `apply_distortion_kernel`)
//! is `f32` in / `f32` out, matching the upstream callback signature.

use std::f64::consts::PI;

const PI_2: f64 = PI / 2.0;

/// Error returned when the Jacobi SVD fails to converge inside the iteration cap.
///
/// Upstream throws `svd_no_convergence`; we surface the same condition as a
/// `Result` so callers can downgrade the perspective correction silently.
#[derive(Debug, Clone, Copy)]
pub struct SvdNoConvergence;

impl std::fmt::Display for SvdNoConvergence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SVD: Iterations did not converge")
    }
}

impl std::error::Error for SvdNoConvergence {}

/// Run the two-sided Jacobi SVD on `m` (rows × n) and return the right-singular
/// vector belonging to the smallest singular value.
///
/// Mirrors upstream's `dvector svd (matrix M)` exactly. The matrix is padded
/// to `2n × n` rows where the bottom `n` rows start as the identity and end as
/// the right-singular vectors; we return the last column of those bottom rows
/// (i.e., the V column for the smallest singular value).
// Port of mod-pc.cpp:104-185.
pub fn svd(mut m: Vec<Vec<f64>>) -> Result<Vec<f64>, SvdNoConvergence> {
    let n = m[0].len();
    let mut s2 = vec![0.0_f64; n];
    let mut estimated_column_rank = n;
    let mut counter = n;
    let mut iterations: i32 = 0;
    let max_cycles: i32 = if n < 120 { 60 } else { (n / 2) as i32 };

    let epsilon = f64::EPSILON;
    let e2 = 10.0 * (n as f64) * epsilon.powi(2);
    let threshold = 0.2 * epsilon;

    // Pad to 2n rows. Existing rows (0..rows) are kept; new rows are zero,
    // then the bottom n rows get the identity (matching upstream's resize +
    // identity-fill loop).
    m.resize(2 * n, vec![0.0_f64; n]);
    for (i, row) in m.iter_mut().enumerate().skip(n).take(n) {
        row[i - n] = 1.0;
    }

    // Mirrors upstream's `iterations++ <= max_cycles` (post-increment): the
    // body runs while the pre-increment value of `iterations` is ≤ max_cycles,
    // i.e. up to max_cycles + 1 times. After exit, `iterations > max_cycles`
    // signals a failed convergence.
    loop {
        if counter == 0 {
            break;
        }
        let pre = iterations;
        iterations += 1;
        if pre > max_cycles {
            break;
        }
        counter = estimated_column_rank * (estimated_column_rank.saturating_sub(1)) / 2;
        for j in 0..estimated_column_rank.saturating_sub(1) {
            for k in (j + 1)..estimated_column_rank {
                let mut p = 0.0_f64;
                let mut q = 0.0_f64;
                let mut r = 0.0_f64;
                for row in m.iter().take(n) {
                    let x0 = row[j];
                    let y0 = row[k];
                    p += x0 * y0;
                    q += x0 * x0;
                    r += y0 * y0;
                }
                s2[j] = q;
                s2[k] = r;
                if q >= r {
                    if q <= e2 * s2[0] || p.abs() <= threshold * q {
                        counter -= 1;
                    } else {
                        let p_n = p / q;
                        let r_n = 1.0 - r / q;
                        let vt = (4.0 * p_n * p_n + r_n * r_n).sqrt();
                        let c0 = (0.5 * (1.0 + r_n / vt)).sqrt();
                        let s0 = p_n / (vt * c0);
                        for row in m.iter_mut().take(2 * n) {
                            let d1 = row[j];
                            let d2 = row[k];
                            row[j] = d1 * c0 + d2 * s0;
                            row[k] = -d1 * s0 + d2 * c0;
                        }
                    }
                } else {
                    let p_n = p / r;
                    let q_n = q / r - 1.0;
                    let vt = (4.0 * p_n * p_n + q_n * q_n).sqrt();
                    let mut s0 = (0.5 * (1.0 - q_n / vt)).sqrt();
                    if p_n < 0.0 {
                        s0 = -s0;
                    }
                    let c0 = p_n / (vt * s0);
                    for row in m.iter_mut().take(2 * n) {
                        let d1 = row[j];
                        let d2 = row[k];
                        row[j] = d1 * c0 + d2 * s0;
                        row[k] = -d1 * s0 + d2 * c0;
                    }
                }
            }
        }
        while estimated_column_rank > 2
            && s2[estimated_column_rank - 1] <= s2[0] * threshold + threshold * threshold
        {
            estimated_column_rank -= 1;
        }
    }

    if iterations > max_cycles + 1 {
        // Both `counter != 0` and the iteration cap were exceeded.
        return Err(SvdNoConvergence);
    }

    // Bottom n rows, last column → right-singular vector for smallest σ.
    let mut result = Vec::with_capacity(n);
    for row in m.iter().skip(n) {
        result.push(row[n - 1]);
    }
    Ok(result)
}

// Port of mod-pc.cpp:76-81.
fn normalize(x: f64, y: f64) -> [f64; 2] {
    let norm = (x * x + y * y).sqrt();
    [x / norm, y / norm]
}

/// Project `(x, y, z)` onto the plane at `plane_distance` from the origin
/// through the origin (central projection).
// Port of mod-pc.cpp:86-91.
fn central_projection(coords: [f64; 3], plane_distance: f64) -> (f64, f64) {
    let stretch = plane_distance / coords[2];
    (coords[0] * stretch, coords[1] * stretch)
}

// Port of mod-pc.cpp:189-192.
fn determinant(m: &[[f64; 3]; 3]) -> f64 {
    m[0][0] * m[1][1] * m[2][2] + m[0][1] * m[1][2] * m[2][0] + m[0][2] * m[1][0] * m[2][1]
        - m[0][2] * m[1][1] * m[2][0]
        - m[0][0] * m[1][2] * m[2][1]
        - m[0][1] * m[1][0] * m[2][2]
}

// Port of mod-pc.cpp:196-209.
fn inverse_matrix(m: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let det_inv = 1.0 / determinant(m);
    let mut r = [[0.0_f64; 3]; 3];
    r[0][0] = det_inv * (m[1][1] * m[2][2] - m[1][2] * m[2][1]);
    r[0][1] = det_inv * (m[0][2] * m[2][1] - m[0][1] * m[2][2]);
    r[0][2] = det_inv * (m[0][1] * m[1][2] - m[0][2] * m[1][1]);
    r[1][0] = det_inv * (m[1][2] * m[2][0] - m[1][0] * m[2][2]);
    r[1][1] = det_inv * (m[0][0] * m[2][2] - m[0][2] * m[2][0]);
    r[1][2] = det_inv * (m[0][2] * m[1][0] - m[0][0] * m[1][2]);
    r[2][0] = det_inv * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    r[2][1] = det_inv * (m[0][1] * m[2][0] - m[0][0] * m[2][1]);
    r[2][2] = det_inv * (m[0][0] * m[1][1] - m[0][1] * m[1][0]);
    r
}

// Port of mod-pc.cpp:211-269.
fn ellipse_analysis(
    x: &[f64],
    y: &[f64],
    f_normalized: f64,
    center_x_io: &mut f64,
    center_y_io: &mut f64,
) -> Result<(f64, f64), SvdNoConvergence> {
    let mut m: Vec<Vec<f64>> = Vec::with_capacity(5);
    for i in 0..5 {
        m.push(vec![x[i] * x[i], x[i] * y[i], y[i] * y[i], x[i], y[i], 1.0]);
    }
    let parameters = svd(m)?;
    let a = parameters[0];
    let b = parameters[1] / 2.0;
    let c = parameters[2];
    let d = parameters[3] / 2.0;
    let f = parameters[4] / 2.0;
    let g = parameters[5];

    let _d_disc = b * b - a * c;
    let x0 = (c * d - b * f) / _d_disc;
    let y0 = (a * f - b * d) / _d_disc;

    let mut phi = 0.5 * ((2.0 * b) / (a - c)).atan();
    if a > c {
        phi += PI_2;
    }

    let _n = 2.0 * (a * f * f + c * d * d + g * b * b - 2.0 * b * d * f - a * c * g) / _d_disc;
    let _s = ((a - c).powi(2) + 4.0 * b * b).sqrt();
    let _r = a + c;
    let mut a_ = (_n / (_s - _r)).sqrt();
    let mut b_ = (_n / (-_s - _r)).sqrt();
    if a_ < b_ {
        std::mem::swap(&mut a_, &mut b_);
        phi -= PI_2;
    }
    // Normalize to -π/2..π/2. Upstream uses C `fmod` which matches Rust `%`.
    phi = (phi + PI_2) % PI - PI_2;

    let mut radius_vertex = -f_normalized / ((a_ / b_).powi(2) - 1.0).sqrt();
    if (x[0] - x0) * (y[1] - y0) < (x[1] - x0) * (y[0] - y0) {
        radius_vertex *= -1.0;
    }

    let x_v = radius_vertex * phi.sin();
    let y_v = radius_vertex * phi.cos();
    *center_x_io = x0;
    *center_y_io = y0;
    Ok((x_v, y_v))
}

// Port of mod-pc.cpp:275-288.
fn intersection(x: &[f64], y: &[f64]) -> (f64, f64) {
    let a_n = x[0] * y[1] - y[0] * x[1];
    let b_n = x[2] * y[3] - y[2] * x[3];
    let c_n = (x[0] - x[1]) * (y[2] - y[3]) - (y[0] - y[1]) * (x[2] - x[3]);
    let nx = a_n * (x[2] - x[3]) - b_n * (x[0] - x[1]);
    let ny = a_n * (y[2] - y[3]) - b_n * (y[0] - y[1]);
    (nx / c_n, ny / c_n)
}

// Port of mod-pc.cpp:307-326.
fn rotate_rho_delta(rho: f64, delta: f64, x: f64, y: f64, z: f64) -> [f64; 3] {
    let a11 = rho.cos();
    let a12 = 0.0;
    let a13 = rho.sin();
    let a21 = rho.sin() * delta.sin();
    let a22 = delta.cos();
    let a23 = -rho.cos() * delta.sin();
    let a31 = -rho.sin() * delta.cos();
    let a32 = delta.sin();
    let a33 = rho.cos() * delta.cos();
    [
        a11 * x + a12 * y + a13 * z,
        a21 * x + a22 * y + a23 * z,
        a31 * x + a32 * y + a33 * z,
    ]
}

// Port of mod-pc.cpp:328-348.
fn rotate_rho_delta_rho_h(rho: f64, delta: f64, rho_h: f64, x: f64, y: f64, z: f64) -> [f64; 3] {
    let a11 = rho.cos() * rho_h.cos() - rho.sin() * delta.cos() * rho_h.sin();
    let a12 = delta.sin() * rho_h.sin();
    let a13 = rho.sin() * rho_h.cos() + rho.cos() * delta.cos() * rho_h.sin();
    let a21 = rho.sin() * delta.sin();
    let a22 = delta.cos();
    let a23 = -rho.cos() * delta.sin();
    let a31 = -rho.cos() * rho_h.sin() - rho.sin() * delta.cos() * rho_h.cos();
    let a32 = delta.sin() * rho_h.cos();
    let a33 = -rho.sin() * rho_h.sin() + rho.cos() * delta.cos() * rho_h.cos();
    [
        a11 * x + a12 * y + a13 * z,
        a21 * x + a22 * y + a23 * z,
        a31 * x + a32 * y + a33 * z,
    ]
}

// Port of mod-pc.cpp:350-375.
fn determine_rho_h(
    rho: f64,
    delta: f64,
    x: &[f64],
    y: &[f64],
    f_normalized: f64,
    center_x: f64,
    center_y: f64,
) -> f64 {
    let p0 = rotate_rho_delta(rho, delta, x[0], y[0], f_normalized);
    let p1 = rotate_rho_delta(rho, delta, x[1], y[1], f_normalized);
    let (x0, y0, z0) = (p0[0], p0[1], p0[2]);
    let (x1, y1, z1) = (p1[0], p1[1], p1[2]);
    if y0 == y1 {
        return if y0 == 0.0 { f64::NAN } else { 0.0 };
    }
    let temp = [x1 - x0, z1 - z0, y1 - y0];
    let (delta_x, delta_z) = central_projection(temp, -y0);
    let x_h = x0 + delta_x;
    let z_h = z0 + delta_z;
    let mut rho_h = if z_h == 0.0 {
        if x_h > 0.0 { 0.0 } else { PI }
    } else {
        PI_2 - (x_h / z_h).atan()
    };
    if rotate_rho_delta_rho_h(rho, delta, rho_h, center_x, center_y, f_normalized)[2] < 0.0 {
        rho_h -= PI;
    }
    rho_h
}

// Port of mod-pc.cpp:377-513.
#[allow(clippy::too_many_arguments)]
fn calculate_angles(
    x: &[f64],
    y: &[f64],
    f_normalized_io: &mut f64,
) -> Result<(f64, f64, f64, f64, f64, f64), SvdNoConvergence> {
    // Returns (rho, delta, rho_h, alpha, center_of_control_points_x, center_of_control_points_y)
    let n = x.len();

    let (mut center_x, mut center_y);
    if n == 6 {
        center_x = x[..4].iter().sum::<f64>() / 4.0;
        center_y = y[..4].iter().sum::<f64>() / 4.0;
    } else {
        center_x = x.iter().sum::<f64>() / n as f64;
        center_y = y.iter().sum::<f64>() / n as f64;
    }

    let (x_v, y_v) = if n == 5 || n == 7 {
        ellipse_analysis(
            &x[..5],
            &y[..5],
            *f_normalized_io,
            &mut center_x,
            &mut center_y,
        )?
    } else {
        let (xv, yv) = intersection(&x[..4], &y[..4]);
        if n == 8 {
            // Over-determined: prefer the fourth line over the focal length.
            let (x_h, y_h) = intersection(&x[4..8], &y[4..8]);
            let radicand = -x_h * xv - y_h * yv;
            if radicand >= 0.0 {
                *f_normalized_io = radicand.sqrt();
            }
        }
        (xv, yv)
    };

    let f_normalized = *f_normalized_io;
    let rho = (-x_v / f_normalized).atan();
    let mut delta = PI_2 - (-y_v / (x_v * x_v + f_normalized * f_normalized).sqrt()).atan();
    if rotate_rho_delta(rho, delta, center_x, center_y, f_normalized)[2] < 0.0 {
        delta -= PI;
    }

    let mut swapped_verticals_and_horizontals = false;

    let mut c = [0.0_f64; 2];
    match n {
        4 | 6 | 8 => {
            let a = normalize(x_v - x[0], y_v - y[0]);
            let b = normalize(x_v - x[2], y_v - y[2]);
            c[0] = a[0] + b[0];
            c[1] = a[1] + b[1];
        }
        5 => {
            c[0] = x_v - center_x;
            c[1] = y_v - center_y;
        }
        _ => {
            // 7
            c[0] = x[5] - x[6];
            c[1] = y[5] - y[6];
        }
    }

    let alpha;
    if n == 7 {
        let p5 = rotate_rho_delta(rho, delta, x[5], y[5], f_normalized);
        let (x5_, y5_) = central_projection(p5, f_normalized);
        let p6 = rotate_rho_delta(rho, delta, x[6], y[6], f_normalized);
        let (x6_, y6_) = central_projection(p6, f_normalized);
        let mut a = -(y6_ - y5_).atan2(x6_ - x5_);
        if c[0].abs() > c[1].abs() {
            // Find smallest rotation into horizontal — upstream uses C fmod.
            a = -((a - PI_2) % PI) - PI_2;
        } else {
            // Find smallest rotation into vertical
            a = -(a % PI) - PI_2;
        }
        alpha = a;
    } else if c[0].abs() > c[1].abs() {
        swapped_verticals_and_horizontals = true;
        alpha = if rho > 0.0 { PI_2 } else { -PI_2 };
    } else {
        alpha = 0.0;
    }

    let rho_h: f64;
    if n == 4 {
        let (x_perp, y_perp) = if swapped_verticals_and_horizontals {
            (
                vec![center_x, center_x],
                vec![center_y - 1.0, center_y + 1.0],
            )
        } else {
            (
                vec![center_x - 1.0, center_x + 1.0],
                vec![center_y, center_y],
            )
        };
        let r = determine_rho_h(
            rho,
            delta,
            &x_perp,
            &y_perp,
            f_normalized,
            center_x,
            center_y,
        );
        rho_h = if r.is_nan() { 0.0 } else { r };
    } else if n == 5 || n == 7 {
        rho_h = 0.0;
    } else {
        let r = determine_rho_h(
            rho,
            delta,
            &x[4..6],
            &y[4..6],
            f_normalized,
            center_x,
            center_y,
        );
        rho_h = if r.is_nan() {
            if n == 8 {
                determine_rho_h(
                    rho,
                    delta,
                    &x[6..8],
                    &y[6..8],
                    f_normalized,
                    center_x,
                    center_y,
                )
            } else {
                0.0
            }
        } else {
            r
        };
    }

    Ok((rho, delta, rho_h, alpha, center_x, center_y))
}

/// Build a 3×3 rotation matrix that combines y(ρ₁), x(δ), y(ρ₂) — modulated by
/// the `d` finetune parameter via quaternion interpolation.
// Port of mod-pc.cpp:519-570.
fn generate_rotation_matrix(rho_1: f64, delta: f64, rho_2: f64, d: f64) -> [[f64; 3]; 3] {
    let s_rho_2 = (rho_2 / 2.0).sin();
    let c_rho_2 = (rho_2 / 2.0).cos();
    let s_delta = (delta / 2.0).sin();
    let c_delta = (delta / 2.0).cos();
    let s_rho_1 = (rho_1 / 2.0).sin();
    let c_rho_1 = (rho_1 / 2.0).cos();

    let mut w = c_rho_2 * c_delta * c_rho_1 - s_rho_2 * c_delta * s_rho_1;
    let mut x = c_rho_2 * s_delta * c_rho_1 + s_rho_2 * s_delta * s_rho_1;
    let mut y = c_rho_2 * c_delta * s_rho_1 + s_rho_2 * c_delta * c_rho_1;
    let mut z = c_rho_2 * s_delta * s_rho_1 - s_rho_2 * s_delta * c_rho_1;

    let mut theta = 2.0 * w.acos();
    if theta > PI {
        theta -= 2.0 * PI;
    }
    let mut s_theta = (theta / 2.0).sin();
    x /= s_theta;
    y /= s_theta;
    z /= s_theta;

    const COMPRESSION: f64 = 10.0;
    theta *= if d <= 0.0 {
        d + 1.0
    } else {
        1.0 + (1.0 / COMPRESSION) * (COMPRESSION * d + 1.0).ln()
    };
    theta = theta.clamp(-0.9 * PI, 0.9 * PI);

    w = (theta / 2.0).cos();
    s_theta = (theta / 2.0).sin();
    x *= s_theta;
    y *= s_theta;
    z *= s_theta;

    let mut m = [[0.0_f64; 3]; 3];
    m[0][0] = 1.0 - 2.0 * y * y - 2.0 * z * z;
    m[0][1] = 2.0 * x * y - 2.0 * z * w;
    m[0][2] = 2.0 * x * z + 2.0 * y * w;
    m[1][0] = 2.0 * x * y + 2.0 * z * w;
    m[1][1] = 1.0 - 2.0 * x * x - 2.0 * z * z;
    m[1][2] = 2.0 * y * z - 2.0 * x * w;
    m[2][0] = 2.0 * x * z - 2.0 * y * w;
    m[2][1] = 2.0 * y * z + 2.0 * x * w;
    m[2][2] = 1.0 - 2.0 * x * x - 2.0 * y * y;
    m
}

/// Direction of the perspective callback — forward (correction) or reverse
/// (distortion). Mirrors upstream's two callback variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Correct an already-distorted image (`Modifier::reverse == false`).
    Correction,
    /// Simulate the perspective distortion (`Modifier::reverse == true`).
    Distortion,
}

/// Pre-computed state for one perspective-correction pass.
///
/// Mirrors upstream's `lfCoordPerspCallbackData`. The 3×3 matrix and the
/// `(delta_a, delta_b)` shift are folded so the per-pixel kernel only does a
/// homogeneous transform.
#[derive(Debug, Clone, Copy)]
pub struct PerspectiveState {
    /// Direction the matrix encodes — `Correction` runs in forward mode,
    /// `Distortion` in reverse mode.
    pub direction: Direction,
    /// Folded 3×3 transform. Stored in `f32` to match upstream `float A[3][3]`.
    pub a: [[f32; 3]; 3],
    /// Image-center shift, applied differently depending on `direction`.
    pub delta_a: f32,
    /// Image-center shift, applied differently depending on `direction`.
    pub delta_b: f32,
}

/// Build the perspective-correction state from control points already in
/// normalized coordinates.
///
/// Returns `None` if the user-supplied data lies in the degenerate set
/// (`count` outside 4..=8, the SVD diverges, or `center_coords[2] <= 0` after
/// rotation). Callers should treat that as "no perspective callback added".
// Port of mod-pc.cpp:572-710.
pub fn build_perspective_state(
    x_norm: &[f64],
    y_norm: &[f64],
    d_in: f32,
    reverse: bool,
) -> Option<PerspectiveState> {
    let count = x_norm.len();
    if !(4..=8).contains(&count) {
        return None;
    }
    let d = d_in.clamp(-1.0, 1.0) as f64;

    let mut f_normalized = 1.0_f64;
    let (rho, delta, rho_h, alpha, ccpx, ccpy) =
        match calculate_angles(x_norm, y_norm, &mut f_normalized) {
            Ok(v) => v,
            Err(_) => return None,
        };

    // Transform center point to get shift.
    let z = rotate_rho_delta_rho_h(rho, delta, rho_h, 0.0, 0.0, f_normalized)[2];
    let use_control_center = z <= 0.0 || f_normalized / z > 10.0;

    let mut a = generate_rotation_matrix(rho, delta, rho_h, d);
    let center_coords: [f64; 3] = if use_control_center {
        [
            a[0][0] * ccpx + a[0][1] * ccpy + a[0][2] * f_normalized,
            a[1][0] * ccpx + a[1][1] * ccpy + a[1][2] * f_normalized,
            a[2][0] * ccpx + a[2][1] * ccpy + a[2][2] * f_normalized,
        ]
    } else {
        [
            a[0][2] * f_normalized,
            a[1][2] * f_normalized,
            a[2][2] * f_normalized,
        ]
    };
    if center_coords[2] <= 0.0 {
        return None;
    }
    let mapping_scale = f_normalized / center_coords[2];

    // Backward (lookup) rotation, then post-multiply by R_z(α).
    {
        let a_ = generate_rotation_matrix(-rho_h, -delta, -rho, d);
        a[0][0] = alpha.cos() * a_[0][0] + alpha.sin() * a_[0][1];
        a[0][1] = -alpha.sin() * a_[0][0] + alpha.cos() * a_[0][1];
        a[0][2] = a_[0][2];
        a[1][0] = alpha.cos() * a_[1][0] + alpha.sin() * a_[1][1];
        a[1][1] = -alpha.sin() * a_[1][0] + alpha.cos() * a_[1][1];
        a[1][2] = a_[1][2];
        a[2][0] = alpha.cos() * a_[2][0] + alpha.sin() * a_[2][1];
        a[2][1] = -alpha.sin() * a_[2][0] + alpha.cos() * a_[2][1];
        a[2][2] = a_[2][2];
    }

    let (mut delta_a, mut delta_b) = central_projection(center_coords, f_normalized);
    {
        let old = delta_a;
        delta_a = alpha.cos() * delta_a + alpha.sin() * delta_b;
        delta_b = -alpha.sin() * old + alpha.cos() * delta_b;
    }

    let mut a_out = [[0.0_f32; 3]; 3];
    let direction;
    if !reverse {
        direction = Direction::Correction;
        a_out[0][0] = (a[0][0] * mapping_scale) as f32;
        a_out[0][1] = (a[0][1] * mapping_scale) as f32;
        a_out[0][2] = (a[0][2] * mapping_scale * center_coords[2]) as f32;
        a_out[1][0] = (a[1][0] * mapping_scale) as f32;
        a_out[1][1] = (a[1][1] * mapping_scale) as f32;
        a_out[1][2] = (a[1][2] * mapping_scale * center_coords[2]) as f32;
        a_out[2][0] = (a[2][0] / center_coords[2]) as f32;
        a_out[2][1] = (a[2][1] / center_coords[2]) as f32;
        a_out[2][2] = a[2][2] as f32;
    } else {
        direction = Direction::Distortion;
        let inv = inverse_matrix(&a);
        a_out[0][0] = inv[0][0] as f32;
        a_out[0][1] = inv[0][1] as f32;
        a_out[0][2] = (inv[0][2] * mapping_scale * center_coords[2]) as f32;
        a_out[1][0] = inv[1][0] as f32;
        a_out[1][1] = inv[1][1] as f32;
        a_out[1][2] = (inv[1][2] * mapping_scale * center_coords[2]) as f32;
        a_out[2][0] = (inv[2][0] / center_coords[2]) as f32;
        a_out[2][1] = (inv[2][1] / center_coords[2]) as f32;
        a_out[2][2] = (inv[2][2] * mapping_scale) as f32;
    }

    Some(PerspectiveState {
        direction,
        a: a_out,
        delta_a: (delta_a / mapping_scale) as f32,
        delta_b: (delta_b / mapping_scale) as f32,
    })
}

/// Apply the perspective-correction kernel in place to a row of normalized
/// `[x0, y0, x1, y1, ...]` coordinates. Out-of-range pixels (`z' <= 0`) are
/// flagged with the upstream sentinel `1.6e16`.
// Port of mod-pc.cpp:712-732.
pub fn apply_correction_kernel(state: &PerspectiveState, row: &mut [f32]) {
    let a = &state.a;
    let n = row.len() / 2;
    for i in 0..n {
        let x = (row[2 * i] + state.delta_a) as f64;
        let y = (row[2 * i + 1] + state.delta_b) as f64;
        let z_ = a[2][0] as f64 * x + a[2][1] as f64 * y + a[2][2] as f64;
        if z_ > 0.0 {
            let z_inv = 1.0 / z_;
            row[2 * i] =
                ((a[0][0] as f64 * x + a[0][1] as f64 * y + a[0][2] as f64) * z_inv) as f32;
            row[2 * i + 1] =
                ((a[1][0] as f64 * x + a[1][1] as f64 * y + a[1][2] as f64) * z_inv) as f32;
        } else {
            row[2 * i] = 1.6e16_f32;
            row[2 * i + 1] = 1.6e16_f32;
        }
    }
}

/// Apply the perspective-distortion kernel in place. Used in reverse mode.
// Port of mod-pc.cpp:734-758.
pub fn apply_distortion_kernel(state: &PerspectiveState, row: &mut [f32]) {
    let a = &state.a;
    let n = row.len() / 2;
    for i in 0..n {
        let x = row[2 * i] as f64;
        let y = row[2 * i + 1] as f64;
        let z_ = a[2][0] as f64 * x + a[2][1] as f64 * y + a[2][2] as f64;
        if z_ > 0.0 {
            let z_inv = 1.0 / z_;
            let mut nx = (a[0][0] as f64 * x + a[0][1] as f64 * y + a[0][2] as f64) * z_inv;
            let mut ny = (a[1][0] as f64 * x + a[1][1] as f64 * y + a[1][2] as f64) * z_inv;
            nx -= state.delta_a as f64;
            ny -= state.delta_b as f64;
            row[2 * i] = nx as f32;
            row[2 * i + 1] = ny as f32;
        } else {
            row[2 * i] = 1.6e16_f32;
            row[2 * i + 1] = 1.6e16_f32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reproduce the upstream SVD test from
    /// `tests/test_modifier_coord_perspective_correction.cpp:70`.
    #[test]
    fn svd_matches_upstream_5x6() {
        let x = [1.0_f64, 2.0, 3.0, 2.0, 1.0];
        let y = [1.0_f64, 2.0, 2.0, 0.0, 1.5];
        let mut m = Vec::with_capacity(5);
        for i in 0..5 {
            m.push(vec![x[i] * x[i], x[i] * y[i], y[i] * y[i], x[i], y[i], 1.0]);
        }
        let result = svd(m).unwrap();
        let eps = f64::EPSILON * 5.0;
        let expected = [
            0.04756514941544937,
            0.09513029883089875,
            0.1902605976617977,
            -0.4280863447390447,
            -0.5707817929853928,
            0.6659120918162917,
        ];
        for (i, (got, want)) in result.iter().zip(expected.iter()).enumerate() {
            assert!(
                (got - want).abs() <= eps,
                "i={i}: got {got}, want {want}, diff {}",
                (got - want).abs()
            );
        }
    }

    #[test]
    fn svd_identity_returns_unit_basis() {
        // 3×3 identity → smallest σ = 1; the right-singular vector is the
        // last column of V = identity, which is (0, 0, 1).
        let m = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];
        let r = svd(m).unwrap();
        assert!((r[0]).abs() < 1e-12);
        assert!((r[1]).abs() < 1e-12);
        assert!((r[2].abs() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn determinant_identity_is_one() {
        let m = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        assert!((determinant(&m) - 1.0).abs() < 1e-15);
    }

    #[test]
    #[allow(clippy::needless_range_loop)]
    fn inverse_then_multiply_is_identity() {
        let m = [[1.0, 2.0, 3.0], [0.0, 1.0, 4.0], [5.0, 6.0, 0.0]];
        let inv = inverse_matrix(&m);
        let mut prod = [[0.0_f64; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                for k in 0..3 {
                    prod[i][j] += m[i][k] * inv[k][j];
                }
            }
        }
        for i in 0..3 {
            for j in 0..3 {
                let want = if i == j { 1.0 } else { 0.0 };
                assert!((prod[i][j] - want).abs() < 1e-12);
            }
        }
    }
}

//! Geometry projection conversions (port of `mod-coord.cpp` geometry section, lines 782-1226).
//!
//! Convert between rectilinear, fisheye (equidistant / orthographic / equisolid /
//! stereographic / Thoby), equirectangular, and panoramic projections. Pure per-pixel
//! functions — the higher-level `Modifier` composes them with focal-length and image-
//! dimension scaling.
//!
//! Naming convention follows the upstream `ModifyCoord_Geom_<dst>_<src>` layout: each
//! function maps a normalized coordinate from the *source* projection to the *destination*
//! projection. So [`fisheye_rect`] takes a rectilinear coord and returns a fisheye coord
//! (mirroring `ModifyCoord_Geom_FishEye_Rect`).
//!
//! All kernels mirror the upstream float discipline: `f32` for I/O, `f64` for trig.

// Upstream defines, mirrored bit-exact.
const EPSLN: f64 = 1.0e-10;
const THOBY_K1_PARM: f64 = 1.47_f32 as f64;
const THOBY_K2_PARM: f64 = 0.713_f32 as f64;

// ============================ rectilinear ↔ fisheye =========================

/// Port of `ModifyCoord_Geom_FishEye_Rect` (mod-coord.cpp:782).
///
/// Maps a rectilinear coordinate to a fisheye-equidistant coordinate.
pub fn fisheye_rect(x: f32, y: f32) -> (f32, f32) {
    let r = (x * x + y * y).sqrt();
    let rho: f32 = if r >= (std::f32::consts::PI / 2.0) {
        1.6e16_f32
    } else if r == 0.0 {
        1.0
    } else {
        r.tan() / r
    };
    (rho * x, rho * y)
}

/// Port of `ModifyCoord_Geom_Rect_FishEye` (mod-coord.cpp:805).
///
/// Maps a fisheye-equidistant coordinate to a rectilinear coordinate.
pub fn rect_fisheye(x: f32, y: f32) -> (f32, f32) {
    let r = (x * x + y * y).sqrt();
    let theta: f32 = if r == 0.0 { 1.0 } else { r.atan() / r };
    (theta * x, theta * y)
}

// =========================== rectilinear ↔ panoramic ========================

/// Port of `ModifyCoord_Geom_Panoramic_Rect` (mod-coord.cpp:824).
pub fn panoramic_rect(x: f32, y: f32) -> (f32, f32) {
    // Upstream uses double-precision libm `tan` / `cos` on `float` inputs.
    let xd = x as f64;
    let yd = y as f64;
    (xd.tan() as f32, (yd / xd.cos()) as f32)
}

/// Port of `ModifyCoord_Geom_Rect_Panoramic` (mod-coord.cpp:839).
pub fn rect_panoramic(x: f32, y: f32) -> (f32, f32) {
    let xd = x as f64;
    let yd = y as f64;
    let new_x = xd.atan();
    // Upstream reads back `iocoord [0]` (now `atan(x)`), still as `float`.
    let new_x_f32 = new_x as f32;
    let new_y = yd * (new_x_f32 as f64).cos();
    (new_x_f32, new_y as f32)
}

// ============================ panoramic ↔ fisheye ===========================

/// Port of `ModifyCoord_Geom_FishEye_Panoramic` (mod-coord.cpp:853).
pub fn fisheye_panoramic(x: f32, y: f32) -> (f32, f32) {
    let xd = x as f64;
    let yd = y as f64;
    let r = (xd * xd + yd * yd).sqrt();
    let s = if r == 0.0 { 1.0 } else { r.sin() / r };

    let vx = r.cos();
    let vy = s * xd;

    let out_x = vy.atan2(vx);
    let out_y = s * yd / (vx * vx + vy * vy).sqrt();
    (out_x as f32, out_y as f32)
}

/// Port of `ModifyCoord_Geom_Panoramic_FishEye` (mod-coord.cpp:873).
pub fn panoramic_fisheye(x: f32, y: f32) -> (f32, f32) {
    let xd = x as f64;
    let yd = y as f64;
    let s = xd.sin();
    let r = (s * s + yd * yd).sqrt();
    let theta = if r == 0.0 { 0.0 } else { r.atan2(xd.cos()) / r };

    ((theta * s) as f32, (theta * yd) as f32)
}

// ========================= rectilinear ↔ equirectangular ====================

/// Port of `ModifyCoord_Geom_ERect_Rect` (mod-coord.cpp:896).
pub fn erect_rect(x: f32, y: f32) -> (f32, f32) {
    let mut x = x;
    let mut theta = -(y as f64) + std::f64::consts::PI / 2.0;
    if theta < 0.0 {
        theta = -theta;
        x += std::f32::consts::PI;
    }
    if theta > std::f64::consts::PI {
        theta = 2.0 * std::f64::consts::PI - theta;
        x += std::f32::consts::PI;
    }
    let xd = x as f64;
    (xd.tan() as f32, (1.0 / (theta.tan() * xd.cos())) as f32)
}

/// Port of `ModifyCoord_Geom_Rect_ERect` (mod-coord.cpp:921).
pub fn rect_erect(x: f32, y: f32) -> (f32, f32) {
    let xd = x as f64;
    let yd = y as f64;
    let out_x = xd.atan2(1.0);
    let out_y = yd.atan2((1.0 + xd * xd).sqrt());
    (out_x as f32, out_y as f32)
}

// =========================== fisheye ↔ equirectangular ======================

/// Port of `ModifyCoord_Geom_ERect_FishEye` (mod-coord.cpp:934).
pub fn erect_fisheye(x: f32, y: f32) -> (f32, f32) {
    let mut x = x;
    let mut theta = -(y as f64) + std::f64::consts::PI / 2.0;
    if theta < 0.0 {
        theta = -theta;
        x += std::f32::consts::PI;
    }
    if theta > std::f64::consts::PI {
        theta = 2.0 * std::f64::consts::PI - theta;
        x += std::f32::consts::PI;
    }
    let xd = x as f64;
    let s = theta.sin();
    let vx = s * xd.sin();
    let vy = theta.cos();

    let r = (vx * vx + vy * vy).sqrt();

    let theta = r.atan2(s * xd.cos());

    let r = 1.0 / r;
    ((theta * vx * r) as f32, (theta * vy * r) as f32)
}

/// Port of `ModifyCoord_Geom_FishEye_ERect` (mod-coord.cpp:968).
pub fn fisheye_erect(x: f32, y: f32) -> (f32, f32) {
    let xd = x as f64;
    let yd = y as f64;
    let r = (xd * xd + yd * yd).sqrt();
    let s = if r == 0.0 { 1.0 } else { r.sin() / r };

    let vx = r.cos();
    let vy = s * xd;

    let out_x = vy.atan2(vx);
    let out_y = (s * yd / (vx * vx + vy * vy).sqrt()).atan();
    (out_x as f32, out_y as f32)
}

// ========================= panoramic ↔ equirectangular ======================

/// Port of `ModifyCoord_Geom_ERect_Panoramic` (mod-coord.cpp:987).
///
/// Upstream only updates `iocoord[1]`; `iocoord[0]` is left untouched.
pub fn erect_panoramic(x: f32, y: f32) -> (f32, f32) {
    (x, (y as f64).tan() as f32)
}

/// Port of `ModifyCoord_Geom_Panoramic_ERect` (mod-coord.cpp:994).
///
/// Upstream only updates `iocoord[1]`; `iocoord[0]` is left untouched.
pub fn panoramic_erect(x: f32, y: f32) -> (f32, f32) {
    (x, (y as f64).atan() as f32)
}

// ====================== orthographic ↔ equirectangular ======================

/// Port of `ModifyCoord_Geom_Orthographic_ERect` (mod-coord.cpp:1003).
pub fn orthographic_erect(x: f32, y: f32) -> (f32, f32) {
    let xd = x as f64;
    let yd = y as f64;
    let r = (xd * xd + yd * yd).sqrt();
    let theta = if r < 1.0 {
        r.asin()
    } else {
        std::f64::consts::PI / 2.0
    };
    let phi = yd.atan2(xd);
    let s = if theta == 0.0 {
        1.0
    } else {
        theta.sin() / theta
    };

    let vx = theta.cos();
    let vy = s * theta * phi.cos();

    let out_x = vy.atan2(vx);
    let out_y = (s * theta * phi.sin() / (vx * vx + vy * vy).sqrt()).atan();
    (out_x as f32, out_y as f32)
}

/// Port of `ModifyCoord_Geom_ERect_Orthographic` (mod-coord.cpp:1031).
pub fn erect_orthographic(x: f32, y: f32) -> (f32, f32) {
    let mut x = x;
    let mut theta = -(y as f64) + std::f64::consts::PI / 2.0;
    if theta < 0.0 {
        theta = -theta;
        x += std::f32::consts::PI;
    }
    if theta > std::f64::consts::PI {
        theta = 2.0 * std::f64::consts::PI - theta;
        x += std::f32::consts::PI;
    }
    let xd = x as f64;
    let s = theta.sin();
    let vx = s * xd.sin();
    let vy = theta.cos();

    let theta = (vx * vx + vy * vy).sqrt().atan2(s * xd.cos());
    let x = vy.atan2(vx);
    let rho = theta.sin();
    ((rho * x.cos()) as f32, (rho * x.sin()) as f32)
}

// ====================== stereographic ↔ equirectangular =====================

/// Port of `ModifyCoord_Geom_Stereographic_ERect` (mod-coord.cpp:1065).
pub fn stereographic_erect(x: f32, y: f32) -> (f32, f32) {
    let xd = x as f64;
    let yd = y as f64;
    let rh = (xd * xd + yd * yd).sqrt();
    let c = 2.0 * (rh / 2.0).atan();
    let sinc = c.sin();
    let cosc = c.cos();

    let mut out_x: f32 = 0.0;
    let out_y: f32;
    if rh.abs() <= EPSLN {
        out_y = 1.6e16_f32;
    } else {
        out_y = (yd * sinc / rh).asin() as f32;
        if cosc.abs() >= EPSLN || xd.abs() >= EPSLN {
            out_x = (xd * sinc).atan2(cosc * rh) as f32;
        } else {
            out_x = 1.6e16_f32;
        }
    }
    (out_x, out_y)
}

/// Port of `ModifyCoord_Geom_ERect_Stereographic` (mod-coord.cpp:1098).
pub fn erect_stereographic(lon: f32, lat: f32) -> (f32, f32) {
    let lon = lon as f64;
    let lat = lat as f64;
    let cosphi = lat.cos();
    let ksp = 2.0 / (1.0 + cosphi * lon.cos());

    ((ksp * cosphi * lon.sin()) as f32, (ksp * lat.sin()) as f32)
}

// ======================== equisolid ↔ equirectangular =======================

/// Port of `ModifyCoord_Geom_Equisolid_ERect` (mod-coord.cpp:1114).
pub fn equisolid_erect(x: f32, y: f32) -> (f32, f32) {
    let xd = x as f64;
    let yd = y as f64;
    let r = (xd * xd + yd * yd).sqrt();
    let theta = if r < 2.0 {
        2.0 * (r / 2.0).asin()
    } else {
        std::f64::consts::PI / 2.0
    };
    let phi = yd.atan2(xd);
    let s = if theta == 0.0 {
        1.0
    } else {
        theta.sin() / theta
    };

    let vx = theta.cos();
    let vy = s * theta * phi.cos();

    let out_x = vy.atan2(vx);
    let out_y = (s * theta * phi.sin() / (vx * vx + vy * vy).sqrt()).atan();
    (out_x as f32, out_y as f32)
}

/// Port of `ModifyCoord_Geom_ERect_Equisolid` (mod-coord.cpp:1141).
pub fn erect_equisolid(lambda: f32, phi: f32) -> (f32, f32) {
    let lambda = lambda as f64;
    let phi = phi as f64;
    if (phi.cos() * lambda.cos() + 1.0).abs() <= EPSLN {
        (1.6e16_f32, 1.6e16_f32)
    } else {
        let k1 = (2.0 / (1.0 + phi.cos() * lambda.cos())).sqrt();
        (
            (k1 * phi.cos() * lambda.sin()) as f32,
            (k1 * phi.sin()) as f32,
        )
    }
}

// ========================== Thoby ↔ equirectangular =========================

/// Port of `ModifyCoord_Geom_Thoby_ERect` (mod-coord.cpp:1167).
pub fn thoby_erect(x: f32, y: f32) -> (f32, f32) {
    let xd = x as f64;
    let yd = y as f64;
    let rho = (xd * xd + yd * yd).sqrt();
    // Mirrors upstream's `rho<-K || rho > K` predicate (mod-coord.cpp:1176).
    #[allow(clippy::manual_range_contains)]
    let outside = rho < -THOBY_K1_PARM || rho > THOBY_K1_PARM;
    if outside {
        (1.6e16_f32, 1.6e16_f32)
    } else {
        let theta = (rho / THOBY_K1_PARM).asin() / THOBY_K2_PARM;
        let phi = yd.atan2(xd);
        let s = if theta == 0.0 {
            1.0
        } else {
            theta.sin() / theta
        };

        let vx = theta.cos();
        let vy = s * theta * phi.cos();

        let out_x = vy.atan2(vx);
        let out_y = (s * theta * phi.sin() / (vx * vx + vy * vy).sqrt()).atan();
        (out_x as f32, out_y as f32)
    }
}

/// Port of `ModifyCoord_Geom_ERect_Thoby` (mod-coord.cpp:1196).
pub fn erect_thoby(x: f32, y: f32) -> (f32, f32) {
    let mut x = x;
    let mut theta = -(y as f64) + std::f64::consts::PI / 2.0;
    if theta < 0.0 {
        theta = -theta;
        x += std::f32::consts::PI;
    }
    if theta > std::f64::consts::PI {
        theta = 2.0 * std::f64::consts::PI - theta;
        x += std::f32::consts::PI;
    }
    let xd = x as f64;
    let s = theta.sin();
    let vx = s * xd.sin();
    let vy = theta.cos();
    let theta = (vx * vx + vy * vy).sqrt().atan2(s * xd.cos());
    let x = vy.atan2(vx);
    let rho = THOBY_K1_PARM * (theta * THOBY_K2_PARM).sin();

    ((rho * x.cos()) as f32, (rho * x.sin()) as f32)
}

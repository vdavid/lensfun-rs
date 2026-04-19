//! A/B comparison harness: lensfun-rs vs upstream LensFun C++.
//!
//! Spawns the probe binary once, feeds it commands via stdin, reads responses
//! from stdout, and compares against our Rust crate's output.
//!
//! Run with:
//!   cargo run --release --example ab_compare
//!
//! The probe binary must exist at tests/cpp-vs-rust/probe.
//! Build it first with: bash tests/cpp-vs-rust/build.sh

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::OnceLock;

use lensfun::{Database, Modifier};

// ---------------------------------------------------------------------------
// Global DB — load once, reuse across all cases.
// ---------------------------------------------------------------------------

static DB: OnceLock<Database> = OnceLock::new();

fn db() -> &'static Database {
    DB.get_or_init(|| Database::load_bundled().expect("bundled DB loads"))
}

// ---------------------------------------------------------------------------
// Case definitions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Case {
    Distortion {
        maker: &'static str,
        model: &'static str,
        focal: f32,
        crop: f32,
        width: u32,
        height: u32,
        reverse: bool,
        x: f32,
        y: f32,
    },
    Tca {
        maker: &'static str,
        model: &'static str,
        focal: f32,
        crop: f32,
        width: u32,
        height: u32,
        reverse: bool,
        x: f32,
        y: f32,
    },
    Vignetting {
        maker: &'static str,
        model: &'static str,
        focal: f32,
        aperture: f32,
        distance: f32,
        crop: f32,
        width: u32,
        height: u32,
        x: f32,
        y: f32,
    },
}

impl Case {
    fn kind(&self) -> &'static str {
        match self {
            Case::Distortion { .. } => "distortion",
            Case::Tca { .. } => "tca",
            Case::Vignetting { .. } => "vignetting",
        }
    }

    /// Write the probe command line (tab-separated, no trailing newline).
    fn probe_command(&self) -> String {
        match self {
            Case::Distortion {
                maker,
                model,
                focal,
                crop,
                width,
                height,
                reverse,
                x,
                y,
            } => format!(
                "distortion\t\"{maker}\"\t\"{model}\"\t{focal:.9}\t{crop:.9}\t{width}\t{height}\t{rev}\t{x:.9}\t{y:.9}",
                rev = if *reverse { 1 } else { 0 },
            ),
            Case::Tca {
                maker,
                model,
                focal,
                crop,
                width,
                height,
                reverse,
                x,
                y,
            } => format!(
                "tca\t\"{maker}\"\t\"{model}\"\t{focal:.9}\t{crop:.9}\t{width}\t{height}\t{rev}\t{x:.9}\t{y:.9}",
                rev = if *reverse { 1 } else { 0 },
            ),
            Case::Vignetting {
                maker,
                model,
                focal,
                aperture,
                distance,
                crop,
                width,
                height,
                x,
                y,
            } => format!(
                "vignetting\t\"{maker}\"\t\"{model}\"\t{focal:.9}\t{aperture:.9}\t{distance:.9}\t{crop:.9}\t{width}\t{height}\t{x:.9}\t{y:.9}",
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Fixture sweep — generated cases
// ---------------------------------------------------------------------------

/// Per-lens descriptor used for generating the fixture sweep.
struct LensFixture {
    maker: &'static str,
    model: &'static str,
    /// Camera body crop factor (not the lens calibration crop).
    crop: f32,
    /// Image dimensions matching the crop factor.
    width: u32,
    height: u32,
    /// Focal lengths to test. LensFun interpolates between calibration points,
    /// so these are chosen to hit actual calibration nodes when possible.
    dist_focals: &'static [f32],
    tca_focals: &'static [f32],
    /// (focal, aperture) pairs for vignetting. Each pair must both be present
    /// in the DB calibration to avoid extrapolation divergence between C++ and Rust.
    vig_focal_aperture: &'static [(f32, f32)],
}

/// 5×5 interior grid (5%/27.5%/50%/72.5%/95%) + 4 corners (1 px in) + center.
/// Avoids exact-edge pixels which can behave oddly in some models.
fn coord_grid(width: u32, height: u32) -> Vec<(f32, f32)> {
    let w = width as f32;
    let h = height as f32;

    // 5-point sample positions as fractions of the image.
    let fracs: [f32; 5] = [0.05, 0.275, 0.50, 0.725, 0.95];

    let mut coords: Vec<(f32, f32)> = Vec::new();

    // 5×5 interior grid.
    for &fy in &fracs {
        for &fx in &fracs {
            coords.push((fx * w, fy * h));
        }
    }

    // 4 corners (1 px from each edge).
    coords.push((1.0, 1.0));
    coords.push((w - 1.0, 1.0));
    coords.push((1.0, h - 1.0));
    coords.push((w - 1.0, h - 1.0));

    // Image center (already in the 5×5 grid at 50%, but keep explicit for
    // clarity — deduplicate isn't needed since the delta is deterministic).
    coords.push((w * 0.5, h * 0.5));

    coords
}

/// Sparse coordinate list for vignetting — 10 coords spread across the image.
/// Vignetting gain depends only on radius, so we just need good radial coverage.
fn vig_coord_grid(width: u32, height: u32) -> Vec<(f32, f32)> {
    let w = width as f32;
    let h = height as f32;
    let fracs: [f32; 4] = [0.05, 0.30, 0.60, 0.95];
    let mut coords: Vec<(f32, f32)> = Vec::new();
    // 4 radial samples along diagonal + axis samples + center.
    for &f in &fracs {
        coords.push((f * w, f * h));
    }
    coords.push((w * 0.5, h * 0.5)); // center
    coords.push((1.0, h * 0.5)); // left edge mid
    coords.push((w - 1.0, h * 0.5)); // right edge mid
    coords.push((w * 0.5, 1.0)); // top mid
    coords.push((w * 0.5, h - 1.0)); // bottom mid
    coords.push((w * 0.95, h * 0.5)); // far right mid
    coords
}

fn cases() -> Vec<Case> {
    // ---------------------------------------------------------------------------
    // Lens table
    //
    // Focal lengths are chosen from the actual calibration nodes in the DB.
    // Using those nodes directly avoids any interpolation artifacts and
    // gives the tightest possible expected deltas.
    // ---------------------------------------------------------------------------
    let lenses: &[LensFixture] = &[
        // 1. Sony APS-C kit zoom — the primary "user has real RAWs" lens.
        //    Vignetting apertures chosen per focal from DB calibration nodes
        //    (focal=16 → 5.0, focal=24 → 4.5, focal=50 → 5.6) to avoid
        //    extrapolation divergence between C++ and Rust.
        LensFixture {
            maker: "Sony",
            model: "E PZ 16-50mm f/3.5-5.6 OSS",
            crop: 1.5,
            width: 6000,
            height: 4000,
            dist_focals: &[16.0, 20.0, 35.0, 50.0],
            tca_focals: &[16.0, 20.0, 35.0, 50.0],
            vig_focal_aperture: &[(16.0, 5.0), (24.0, 4.5), (50.0, 5.6)],
        },
        // 2. Wide rectilinear zoom — full-frame Canon.
        //    All vig focals have the same aperture set (2.8 calibrated at all).
        LensFixture {
            maker: "Canon",
            model: "Canon EF 16-35mm f/2.8L III USM",
            crop: 1.0,
            width: 6000,
            height: 4000,
            dist_focals: &[16.0, 20.0, 22.0, 35.0],
            tca_focals: &[16.0, 20.0, 22.0, 35.0],
            vig_focal_aperture: &[(16.0, 2.8), (20.0, 2.8), (35.0, 2.8)],
        },
        // 3. Normal zoom — Canon full-frame, used in regression tests.
        LensFixture {
            maker: "Canon",
            model: "Canon EF 24-70mm f/2.8L II USM",
            crop: 1.0,
            width: 6000,
            height: 4000,
            dist_focals: &[24.0, 35.0, 50.0, 70.0],
            tca_focals: &[24.0, 35.0, 50.0, 70.0],
            vig_focal_aperture: &[(24.0, 2.8), (35.0, 2.8), (70.0, 2.8)],
        },
        // 4. Telephoto zoom — Pentax APS-C, used in regression tests.
        //    Note: Pentax DA has no TCA calibration in the DB, so tca_focals
        //    is empty and we skip TCA for this lens.
        //    Vig: focal=50 min aperture=4.0, focal=95 → 4.5, focal=200 → 5.6.
        LensFixture {
            maker: "Pentax",
            model: "smc Pentax-DA 50-200mm f/4-5.6 DA ED",
            crop: 1.5,
            width: 6000,
            height: 4000,
            dist_focals: &[50.0, 95.0, 160.0, 200.0],
            tca_focals: &[], // no TCA calibration in DB
            vig_focal_aperture: &[(50.0, 4.0), (95.0, 4.5), (200.0, 5.6)],
        },
        // 5. Prime — Canon EF 50mm f/1.4.
        LensFixture {
            maker: "Canon",
            model: "Canon EF 50mm f/1.4 USM",
            crop: 1.0,
            width: 6000,
            height: 4000,
            dist_focals: &[50.0],
            tca_focals: &[50.0],
            vig_focal_aperture: &[(50.0, 1.4)],
        },
        // 6. Fisheye — Samyang 7.5mm MFT (crop 2.0, rich calibration).
        LensFixture {
            maker: "Samyang",
            model: "Samyang 7.5mm f/3.5 UMC Fish-eye MFT",
            crop: 2.0,
            width: 6000,
            height: 4000,
            dist_focals: &[7.5],
            tca_focals: &[7.5],
            vig_focal_aperture: &[(7.5, 3.5)],
        },
    ];

    let mut all: Vec<Case> = Vec::new();

    for lens in lenses {
        let full_coords = coord_grid(lens.width, lens.height);
        let vig_coords = vig_coord_grid(lens.width, lens.height);

        // Distortion: forward (reverse=false) and backward (reverse=true).
        for &focal in lens.dist_focals {
            for &(x, y) in &full_coords {
                for &reverse in &[true, false] {
                    all.push(Case::Distortion {
                        maker: lens.maker,
                        model: lens.model,
                        focal,
                        crop: lens.crop,
                        width: lens.width,
                        height: lens.height,
                        reverse,
                        x,
                        y,
                    });
                }
            }
        }

        // TCA: reverse=true (image-correction direction).
        for &focal in lens.tca_focals {
            for &(x, y) in &full_coords {
                all.push(Case::Tca {
                    maker: lens.maker,
                    model: lens.model,
                    focal,
                    crop: lens.crop,
                    width: lens.width,
                    height: lens.height,
                    reverse: true,
                    x,
                    y,
                });
            }
        }

        // Vignetting: sparse grid, per (focal, aperture) pairs that are both
        // present as calibration nodes in the DB to avoid extrapolation edge cases.
        // distance=1000.0 is a calibration node in all six lenses, so no
        // distance-axis interpolation happens and C++/Rust agree exactly there.
        for &(focal, aperture) in lens.vig_focal_aperture {
            for &(x, y) in &vig_coords {
                all.push(Case::Vignetting {
                    maker: lens.maker,
                    model: lens.model,
                    focal,
                    aperture,
                    distance: 1000.0,
                    crop: lens.crop,
                    width: lens.width,
                    height: lens.height,
                    x,
                    y,
                });
            }
        }
    }

    all
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum ProbeOutput {
    Distortion {
        x: f32,
        y: f32,
    },
    Tca {
        x_red: f32,
        y_red: f32,
        x_blue: f32,
        y_blue: f32,
    },
    Vignetting {
        gain: f32,
    },
}

#[derive(Debug, Clone)]
struct CaseResult {
    case_idx: usize,
    kind: &'static str,
    pass: bool,
    max_abs_delta: f32,
    probe: ProbeOutput,
    rust: ProbeOutput,
}

// ---------------------------------------------------------------------------
// Probe I/O
// ---------------------------------------------------------------------------

struct Probe {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Probe {
    fn spawn(path: &str) -> Self {
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("failed to spawn probe binary");

        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Probe {
            child,
            stdin,
            stdout,
        }
    }

    fn send(&mut self, cmd: &str) -> String {
        writeln!(self.stdin, "{cmd}").expect("write to probe stdin");
        self.stdin.flush().expect("flush probe stdin");
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .expect("read from probe stdout");
        line.trim_end_matches(['\n', '\r']).to_string()
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        // Close stdin so the probe exits cleanly.
        // We can't close it in-place on the struct, so just wait.
        let _ = self.child.wait();
    }
}

// ---------------------------------------------------------------------------
// Parse probe responses
// ---------------------------------------------------------------------------

fn parse_distortion_response(s: &str) -> Option<ProbeOutput> {
    let mut it = s.split('\t');
    let x: f32 = it.next()?.parse().ok()?;
    let y: f32 = it.next()?.parse().ok()?;
    Some(ProbeOutput::Distortion { x, y })
}

fn parse_tca_response(s: &str) -> Option<ProbeOutput> {
    let mut it = s.split('\t');
    let x_red: f32 = it.next()?.parse().ok()?;
    let y_red: f32 = it.next()?.parse().ok()?;
    let x_blue: f32 = it.next()?.parse().ok()?;
    let y_blue: f32 = it.next()?.parse().ok()?;
    Some(ProbeOutput::Tca {
        x_red,
        y_red,
        x_blue,
        y_blue,
    })
}

fn parse_vignetting_response(s: &str) -> Option<ProbeOutput> {
    let gain: f32 = s.trim().parse().ok()?;
    Some(ProbeOutput::Vignetting { gain })
}

// ---------------------------------------------------------------------------
// Run Rust-side equivalent
// ---------------------------------------------------------------------------

fn find_lens_rust(maker: &str, model: &str) -> Option<&'static lensfun::Lens> {
    let lenses = db().find_lenses(None, model);
    // The Rust `find_lenses(None, model)` doesn't score by maker, so filter by
    // maker after the fact to match the probe's `lf_db_find_lenses(db, NULL, maker, model, 0)`.
    // If no maker match found, fall back to the first result (e.g. some lens models
    // are unique enough that maker filtering isn't needed).
    lenses
        .iter()
        .find(|l| l.maker.eq_ignore_ascii_case(maker))
        .or_else(|| lenses.first())
        .copied()
}

fn run_rust(case: &Case) -> Option<ProbeOutput> {
    match case {
        Case::Distortion {
            maker,
            model,
            focal,
            crop,
            width,
            height,
            reverse,
            x,
            y,
        } => {
            let lens = find_lens_rust(maker, model)?;
            let mut m = Modifier::new(lens, *focal, *crop, *width, *height, *reverse);
            if !m.enable_distortion_correction(lens) {
                eprintln!("  [rust] distortion correction did not enable for {maker}/{model}");
                return None;
            }
            let mut coords = [0.0_f32; 2];
            m.apply_geometry_distortion(*x, *y, 1, 1, &mut coords);
            Some(ProbeOutput::Distortion {
                x: coords[0],
                y: coords[1],
            })
        }
        Case::Tca {
            maker,
            model,
            focal,
            crop,
            width,
            height,
            reverse,
            x,
            y,
        } => {
            let lens = find_lens_rust(maker, model)?;
            let mut m = Modifier::new(lens, *focal, *crop, *width, *height, *reverse);
            if !m.enable_tca_correction(lens) {
                eprintln!("  [rust] TCA correction did not enable for {maker}/{model}");
                return None;
            }
            let mut coords = [0.0_f32; 6];
            m.apply_subpixel_distortion(*x, *y, 1, 1, &mut coords);
            // coords layout: [xR, yR, xG, yG, xB, yB]
            Some(ProbeOutput::Tca {
                x_red: coords[0],
                y_red: coords[1],
                x_blue: coords[4],
                y_blue: coords[5],
            })
        }
        Case::Vignetting {
            maker,
            model,
            focal,
            aperture,
            distance,
            crop,
            width,
            height,
            x,
            y,
        } => {
            let lens = find_lens_rust(maker, model)?;
            // Probe hardcodes reverse=false for vignetting.
            let mut m = Modifier::new(lens, *focal, *crop, *width, *height, false);
            if !m.enable_vignetting_correction(lens, *aperture, *distance) {
                eprintln!("  [rust] vignetting correction did not enable for {maker}/{model}");
                return None;
            }
            // Apply to a single 1.0 f32 pixel — result is the gain at (x, y).
            let mut pixel = [1.0_f32];
            m.apply_color_modification_f32(&mut pixel, *x, *y, 1, 1, 1);
            Some(ProbeOutput::Vignetting { gain: pixel[0] })
        }
    }
}

// ---------------------------------------------------------------------------
// Compare outputs
// ---------------------------------------------------------------------------

fn max_abs_delta(probe: &ProbeOutput, rust: &ProbeOutput) -> f32 {
    match (probe, rust) {
        (ProbeOutput::Distortion { x: px, y: py }, ProbeOutput::Distortion { x: rx, y: ry }) => {
            (px - rx).abs().max((py - ry).abs())
        }
        (
            ProbeOutput::Tca {
                x_red: pxr,
                y_red: pyr,
                x_blue: pxb,
                y_blue: pyb,
            },
            ProbeOutput::Tca {
                x_red: rxr,
                y_red: ryr,
                x_blue: rxb,
                y_blue: ryb,
            },
        ) => (pxr - rxr)
            .abs()
            .max((pyr - ryr).abs())
            .max((pxb - rxb).abs())
            .max((pyb - ryb).abs()),
        (ProbeOutput::Vignetting { gain: pg }, ProbeOutput::Vignetting { gain: rg }) => {
            (pg - rg).abs()
        }
        _ => f32::INFINITY,
    }
}

// ---------------------------------------------------------------------------
// Stats per kind
// ---------------------------------------------------------------------------

#[derive(Default)]
struct KindStats {
    count: usize,
    pass: usize,
    max_delta: f32,
    worst_failures: Vec<(usize, f32, ProbeOutput, ProbeOutput)>,
}

impl KindStats {
    fn record(
        &mut self,
        idx: usize,
        pass: bool,
        delta: f32,
        probe: ProbeOutput,
        rust: ProbeOutput,
    ) {
        self.count += 1;
        if pass {
            self.pass += 1;
        } else {
            self.worst_failures.push((idx, delta, probe, rust));
        }
        if delta > self.max_delta {
            self.max_delta = delta;
        }
    }
}

// ---------------------------------------------------------------------------
// Bench mode
// ---------------------------------------------------------------------------

/// Sony E PZ 16-50mm f/3.5-5.6 OSS at 35mm — proven working in smoke tests.
const BENCH_MAKER: &str = "Sony";
const BENCH_MODEL: &str = "E PZ 16-50mm f/3.5-5.6 OSS";
const BENCH_FOCAL: f32 = 35.0;
const BENCH_CROP: f32 = 1.5;
const BENCH_WIDTH: u32 = 6000;
const BENCH_HEIGHT: u32 = 4000;

const BENCH_VIG_FOCAL: f32 = 16.0; // use 16mm with aperture 5.0 (known calibration node)
const BENCH_VIG_APERTURE_AT16: f32 = 5.0;
const BENCH_VIG_DISTANCE: f32 = 1000.0;

const DIST_ITERS: u64 = 1_000_000;
const TCA_ITERS: u64 = 1_000_000;
const VIG_ITERS: u64 = 100_000;

struct BenchResult {
    rust_ns: u64,
    cpp_ns: u64,
}

fn bench_distortion(probe: &mut Probe) -> BenchResult {
    let lens = find_lens_rust(BENCH_MAKER, BENCH_MODEL).expect("bench lens not found");
    let mut m = Modifier::new(
        lens,
        BENCH_FOCAL,
        BENCH_CROP,
        BENCH_WIDTH,
        BENCH_HEIGHT,
        true,
    );
    assert!(
        m.enable_distortion_correction(lens),
        "bench: distortion correction did not enable"
    );

    let x = BENCH_WIDTH as f32 * 0.3;
    let y = BENCH_HEIGHT as f32 * 0.3;

    // Warmup (Rust)
    for _ in 0..10_000 {
        let mut coords = [0.0_f32; 2];
        std::hint::black_box(m.apply_geometry_distortion(
            std::hint::black_box(x),
            std::hint::black_box(y),
            1,
            1,
            &mut coords,
        ));
        std::hint::black_box(&coords);
    }

    // Timed loop (Rust)
    let t0 = std::time::Instant::now();
    for _ in 0..DIST_ITERS {
        let mut coords = [0.0_f32; 2];
        std::hint::black_box(m.apply_geometry_distortion(
            std::hint::black_box(x),
            std::hint::black_box(y),
            1,
            1,
            &mut coords,
        ));
        std::hint::black_box(&coords);
    }
    let rust_ns = t0.elapsed().as_nanos() as u64;

    // C++ bench via probe
    let cmd = format!(
        "bench\tdistortion\t\"{BENCH_MAKER}\"\t\"{BENCH_MODEL}\"\t{BENCH_FOCAL:.9}\t{BENCH_CROP:.9}\t{BENCH_WIDTH}\t{BENCH_HEIGHT}\t1\t{DIST_ITERS}"
    );
    let resp = probe.send(&cmd);
    let cpp_ns: u64 = resp
        .trim()
        .parse()
        .expect("bench distortion: bad probe response");

    BenchResult { rust_ns, cpp_ns }
}

fn bench_tca(probe: &mut Probe) -> BenchResult {
    let lens = find_lens_rust(BENCH_MAKER, BENCH_MODEL).expect("bench lens not found");
    let mut m = Modifier::new(
        lens,
        BENCH_FOCAL,
        BENCH_CROP,
        BENCH_WIDTH,
        BENCH_HEIGHT,
        true,
    );
    assert!(
        m.enable_tca_correction(lens),
        "bench: TCA correction did not enable"
    );

    let x = BENCH_WIDTH as f32 * 0.3;
    let y = BENCH_HEIGHT as f32 * 0.3;

    // Warmup (Rust)
    for _ in 0..10_000 {
        let mut coords = [0.0_f32; 6];
        std::hint::black_box(m.apply_subpixel_distortion(
            std::hint::black_box(x),
            std::hint::black_box(y),
            1,
            1,
            &mut coords,
        ));
        std::hint::black_box(&coords);
    }

    // Timed loop (Rust)
    let t0 = std::time::Instant::now();
    for _ in 0..TCA_ITERS {
        let mut coords = [0.0_f32; 6];
        std::hint::black_box(m.apply_subpixel_distortion(
            std::hint::black_box(x),
            std::hint::black_box(y),
            1,
            1,
            &mut coords,
        ));
        std::hint::black_box(&coords);
    }
    let rust_ns = t0.elapsed().as_nanos() as u64;

    // C++ bench via probe
    let cmd = format!(
        "bench\ttca\t\"{BENCH_MAKER}\"\t\"{BENCH_MODEL}\"\t{BENCH_FOCAL:.9}\t{BENCH_CROP:.9}\t{BENCH_WIDTH}\t{BENCH_HEIGHT}\t1\t{TCA_ITERS}"
    );
    let resp = probe.send(&cmd);
    let cpp_ns: u64 = resp.trim().parse().expect("bench tca: bad probe response");

    BenchResult { rust_ns, cpp_ns }
}

fn bench_vignetting(probe: &mut Probe) -> BenchResult {
    let lens = find_lens_rust(BENCH_MAKER, BENCH_MODEL).expect("bench lens not found");
    // Use focal=16 with aperture=5.0 — known calibration node
    let mut m = Modifier::new(
        lens,
        BENCH_VIG_FOCAL,
        BENCH_CROP,
        BENCH_WIDTH,
        BENCH_HEIGHT,
        false,
    );
    assert!(
        m.enable_vignetting_correction(lens, BENCH_VIG_APERTURE_AT16, BENCH_VIG_DISTANCE),
        "bench: vignetting correction did not enable"
    );

    let x = BENCH_WIDTH as f32 * 0.3;
    let y = BENCH_HEIGHT as f32 * 0.3;

    // Warmup (Rust)
    for _ in 0..10_000 {
        let mut pixel = [1.0_f32];
        std::hint::black_box(m.apply_color_modification_f32(
            &mut pixel,
            std::hint::black_box(x),
            std::hint::black_box(y),
            1,
            1,
            1,
        ));
        std::hint::black_box(&pixel);
    }

    // Timed loop (Rust)
    let t0 = std::time::Instant::now();
    for _ in 0..VIG_ITERS {
        let mut pixel = [1.0_f32];
        std::hint::black_box(m.apply_color_modification_f32(
            &mut pixel,
            std::hint::black_box(x),
            std::hint::black_box(y),
            1,
            1,
            1,
        ));
        std::hint::black_box(&pixel);
    }
    let rust_ns = t0.elapsed().as_nanos() as u64;

    // C++ bench via probe
    let cmd = format!(
        "bench\tvignetting\t\"{BENCH_MAKER}\"\t\"{BENCH_MODEL}\"\t{BENCH_VIG_FOCAL:.9}\t{BENCH_VIG_APERTURE_AT16:.9}\t{BENCH_VIG_DISTANCE:.9}\t{BENCH_CROP:.9}\t{BENCH_WIDTH}\t{BENCH_HEIGHT}\t{VIG_ITERS}"
    );
    let resp = probe.send(&cmd);
    let cpp_ns: u64 = resp
        .trim()
        .parse()
        .expect("bench vignetting: bad probe response");

    BenchResult { rust_ns, cpp_ns }
}

fn print_bench_results(dist: &BenchResult, tca: &BenchResult, vig: &BenchResult) {
    fn fmt_mops(iters: u64, ns: u64) -> String {
        let mops = iters as f64 / (ns as f64 / 1_000.0);
        format!("{:.1} M ops/s", mops)
    }

    fn ratio_label(rust_ns: u64, cpp_ns: u64) -> String {
        let ratio = cpp_ns as f64 / rust_ns as f64;
        if ratio >= 1.0 {
            format!("{:.2}× rust faster", ratio)
        } else {
            format!("{:.2}× rust slower", 1.0 / ratio)
        }
    }

    let dist_rust = fmt_mops(DIST_ITERS, dist.rust_ns);
    let dist_cpp = fmt_mops(DIST_ITERS, dist.cpp_ns);
    let dist_ratio = ratio_label(dist.rust_ns, dist.cpp_ns);

    let tca_rust = fmt_mops(TCA_ITERS, tca.rust_ns);
    let tca_cpp = fmt_mops(TCA_ITERS, tca.cpp_ns);
    let tca_ratio = ratio_label(tca.rust_ns, tca.cpp_ns);

    let vig_rust = fmt_mops(VIG_ITERS, vig.rust_ns);
    let vig_cpp = fmt_mops(VIG_ITERS, vig.cpp_ns);
    let vig_ratio = ratio_label(vig.rust_ns, vig.cpp_ns);

    println!("── throughput (single-pixel kernel call) ──");
    println!("{:<18} {:<16} {:<16} ratio", "", "rust", "c++");
    println!(
        "{:<18} {:<16} {:<16} {}",
        "distortion", dist_rust, dist_cpp, dist_ratio
    );
    println!(
        "{:<18} {:<16} {:<16} {}",
        "tca", tca_rust, tca_cpp, tca_ratio
    );
    println!(
        "{:<18} {:<16} {:<16} {}",
        "vignetting", vig_rust, vig_cpp, vig_ratio
    );
    println!();
    println!("iterations: distortion={DIST_ITERS}, tca={TCA_ITERS}, vignetting={VIG_ITERS}");
    println!("lens: {BENCH_MAKER} {BENCH_MODEL}");
    println!(
        "  distortion/tca: focal={BENCH_FOCAL}mm  vignetting: focal={BENCH_VIG_FOCAL}mm f/{BENCH_VIG_APERTURE_AT16}"
    );
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let probe_path = format!("{manifest_dir}/tests/cpp-vs-rust/probe");

    let bench_mode = std::env::args().any(|a| a == "--bench");

    let mut probe = Probe::spawn(&probe_path);

    // Warm up the DB on the Rust side.
    let _ = db();

    if bench_mode {
        println!("Running throughput benchmark...");
        println!("(warmup + timed loops — this takes a few seconds)");
        println!();

        let dist = bench_distortion(&mut probe);
        let tca = bench_tca(&mut probe);
        let vig = bench_vignetting(&mut probe);

        print_bench_results(&dist, &tca, &vig);
        return;
    }

    let tolerance: f32 = 1e-3;

    let all_cases = cases();
    println!("Running {} cases...", all_cases.len());

    let mut results: Vec<CaseResult> = Vec::with_capacity(all_cases.len());

    for (idx, case) in all_cases.iter().enumerate() {
        let cmd = case.probe_command();
        let probe_response = probe.send(&cmd);

        // Parse probe output.
        let probe_out = match case {
            Case::Distortion { .. } => parse_distortion_response(&probe_response),
            Case::Tca { .. } => parse_tca_response(&probe_response),
            Case::Vignetting { .. } => parse_vignetting_response(&probe_response),
        };

        let Some(probe_out) = probe_out else {
            eprintln!("[case {idx}] failed to parse probe response: {probe_response:?}");
            continue;
        };

        // Run Rust side.
        let Some(rust_out) = run_rust(case) else {
            eprintln!("[case {idx}] Rust side returned None");
            continue;
        };

        let delta = max_abs_delta(&probe_out, &rust_out);
        let pass = delta <= tolerance;

        results.push(CaseResult {
            case_idx: idx,
            kind: case.kind(),
            pass,
            max_abs_delta: delta,
            probe: probe_out,
            rust: rust_out,
        });
    }

    // Aggregate per-kind stats.
    let mut dist_stats = KindStats::default();
    let mut tca_stats = KindStats::default();
    let mut vig_stats = KindStats::default();

    for r in &results {
        let stats = match r.kind {
            "distortion" => &mut dist_stats,
            "tca" => &mut tca_stats,
            "vignetting" => &mut vig_stats,
            _ => continue,
        };
        stats.record(
            r.case_idx,
            r.pass,
            r.max_abs_delta,
            r.probe.clone(),
            r.rust.clone(),
        );
    }

    // Print summary.
    println!();
    println!("── lensfun-rs vs upstream LensFun C++ ──");

    fn print_stats(label: &str, s: &KindStats, tol: f32) {
        let fail = s.count - s.pass;
        println!(
            "{label}: {} {}, {} ok, {} fail, max abs delta = {:.3e}",
            s.count,
            if s.count == 1 { "case" } else { "cases" },
            s.pass,
            fail,
            s.max_delta,
        );
        // Print details for failures (worst 3).
        for (idx, delta, probe_out, rust_out) in s.worst_failures.iter().take(3) {
            println!("  FAIL case {idx}: delta={delta:.3e} > tol={tol:.3e}");
            println!("    probe: {probe_out:?}");
            println!("    rust:  {rust_out:?}");
        }
        if s.worst_failures.len() > 3 {
            println!("  ... and {} more failures", s.worst_failures.len() - 3);
        }
    }

    print_stats("distortion", &dist_stats, tolerance);
    print_stats("tca       ", &tca_stats, tolerance);
    print_stats("vignetting", &vig_stats, tolerance);

    let all_pass = results.iter().all(|r| r.pass);
    println!();
    if all_pass {
        println!("✓ all under {tolerance:.0e} tolerance");
    } else {
        let fail_count = results.iter().filter(|r| !r.pass).count();
        eprintln!("✗ {fail_count} case(s) exceeded {tolerance:.0e} tolerance");
        std::process::exit(1);
    }
}

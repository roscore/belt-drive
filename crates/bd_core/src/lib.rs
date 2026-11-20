//! Timing-belt reduction model (pure Rust, host-testable, no web dependencies).
//!
//! Textbook relations only: pitch-circle geometry, the open-belt length equation and its
//! inverse, wrap angle, teeth in mesh, effective / centrifugal / tight / slack tensions,
//! span vibration frequency and linear-elastic span stiffness. Units: mm, N, N·m, rpm at the
//! API; SI inside formulas. All profile values are generic "typical" estimates.
use std::f64::consts::PI;

/// Generic belt profile (values are typical estimates, not catalogue data).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    pub name_en: &'static str,
    pub name_ko: &'static str,
    /// Tooth pitch (mm).
    pub pitch: f64,
    /// Mass per metre of belt, per mm of width (kg/m/mm).
    pub mass: f64,
    /// Allowable working tension per mm of width (N/mm).
    pub allow: f64,
    /// Tensile stiffness EA per mm of width (N per unit strain, per mm).
    pub ea: f64,
    /// Minimum recommended pulley teeth.
    pub zmin: u32,
    /// Maximum belt speed (m/s).
    pub vmax: f64,
    /// Tooth height (mm), used only for drawing.
    pub tooth_h: f64,
}

pub const PROFILES: [Profile; 4] = [
    Profile { name_en: "GT-style 2 mm (typical)", name_ko: "GT형 2 mm (일반값)", pitch: 2.0, mass: 0.0013, allow: 6.0, ea: 700.0, zmin: 12, vmax: 40.0, tooth_h: 0.75 },
    Profile { name_en: "GT-style 3 mm (typical)", name_ko: "GT형 3 mm (일반값)", pitch: 3.0, mass: 0.0024, allow: 12.0, ea: 1300.0, zmin: 14, vmax: 40.0, tooth_h: 1.14 },
    Profile { name_en: "HTD-style 5 mm (typical)", name_ko: "HTD형 5 mm (일반값)", pitch: 5.0, mass: 0.0040, allow: 20.0, ea: 2200.0, zmin: 16, vmax: 35.0, tooth_h: 2.06 },
    Profile { name_en: "T-style 5 mm steel cord (typical)", name_ko: "T형 5 mm 스틸 코드 (일반값)", pitch: 5.0, mass: 0.0025, allow: 15.0, ea: 6000.0, zmin: 12, vmax: 30.0, tooth_h: 1.2 },
];

/// Search widths (mm).
pub const WIDTHS: [f64; 7] = [6.0, 9.0, 10.0, 12.0, 15.0, 20.0, 25.0];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StageIn {
    pub z1: u32,
    pub z2: u32,
    /// Target centre distance (mm).
    pub c: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Input {
    pub profile: usize,
    pub stages: Vec<StageIn>,
    /// Belt width (mm), same for all stages.
    pub width: f64,
    /// Motor peak torque (N·m).
    pub motor_torque: f64,
    /// Motor max speed (rpm).
    pub motor_rpm: f64,
    /// Required joint output peak torque (N·m).
    pub out_torque: f64,
    /// Pretension as a fraction of the belt's allowable working tension (allow × b).
    pub pre_factor: f64,
    pub service: f64,
}

impl Default for Input {
    fn default() -> Self {
        Input {
            profile: 1,
            stages: vec![StageIn { z1: 16, z2: 48, c: 45.0 }, StageIn { z1: 18, z2: 54, c: 50.0 }],
            width: 15.0,
            motor_torque: 0.35,
            motor_rpm: 4000.0,
            out_torque: 3.0,
            pre_factor: 0.5,
            service: 1.4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

/// Which check a status line belongs to (text lives in the UI for i18n).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckKind {
    Geometry,
    MinTeeth,
    TeethInMesh,
    SlackSide,
    Width,
    Speed,
    MotorTorque,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Check {
    /// Stage index, or `None` for drive-level checks.
    pub stage: Option<usize>,
    pub kind: CheckKind,
    pub status: Status,
    /// Value shown with the message (meaning depends on `kind`).
    pub value: f64,
    pub limit: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct StageOut {
    pub z1: u32,
    pub z2: u32,
    /// Pitch diameters (mm).
    pub d1: f64,
    pub d2: f64,
    pub ratio: f64,
    /// Belt tooth count and pitch length (mm).
    pub belt_teeth: u32,
    pub belt_len: f64,
    /// Real centre distance (mm) for the chosen belt.
    pub c: f64,
    /// Straight span length (mm).
    pub span: f64,
    /// Wrap angle on the small pulley (rad).
    pub wrap: f64,
    pub teeth_in_mesh: u32,
    /// Torque on the driver pulley at the design load (N·m).
    pub torque_in: f64,
    /// Effective, centrifugal, pre-, tight and slack tensions (N).
    pub te: f64,
    pub tc: f64,
    pub ti: f64,
    pub t1: f64,
    pub t2: f64,
    /// Belt speed at max motor rpm (m/s) and the motor rpm that reaches the profile limit.
    pub speed: f64,
    pub rpm_limit: f64,
    pub width_req: f64,
    /// Span transverse natural frequency at pretension (Hz).
    pub freq: f64,
    /// Torsional stiffness at this stage's driven shaft (N·m/rad).
    pub k_rot: f64,
    /// Radial shaft load (N).
    pub shaft_load: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Output {
    pub stages: Vec<StageOut>,
    pub ratio: f64,
    /// Design output torque (N·m): max of the requirement and what the motor can deliver.
    pub load: f64,
    /// Output stiffness (N·m/rad) and deflection at the design load (arcmin).
    pub k_out: f64,
    pub defl_arcmin: f64,
    /// Max output speed (rpm) and motor torque × ratio (N·m).
    pub out_rpm: f64,
    pub out_capacity: f64,
    /// Overall package length along the drive (mm).
    pub package: f64,
    pub checks: Vec<Check>,
    pub status: Status,
}

pub fn pitch_diameter(z: u32, p: f64) -> f64 {
    z as f64 * p / PI
}

/// Open-belt pitch length (mm) for pitch diameters `d`, `dd` and centre distance `c`.
/// `None` if the centre distance is too short for the pulleys.
pub fn belt_length(d: f64, dd: f64, c: f64) -> Option<f64> {
    let e = (dd - d).abs() / 2.0;
    if c <= e {
        return None;
    }
    Some(2.0 * (c * c - e * e).sqrt() + PI * (d + dd) / 2.0 + 2.0 * e * (e / c).asin())
}

/// Inverse of [`belt_length`]: centre distance for a belt of pitch length `len` (Newton).
pub fn centre_distance(d: f64, dd: f64, len: f64) -> Option<f64> {
    let e = (dd - d).abs() / 2.0;
    let wrap_len = PI * (d + dd) / 2.0;
    // Smallest length the equation allows is at c = e (spans of zero length).
    if len <= wrap_len + PI * e {
        return None;
    }
    let mut c = ((len - wrap_len) / 2.0).max(e * 1.0001 + 1e-9);
    for _ in 0..60 {
        let f = belt_length(d, dd, c)? - len;
        // dL/dc = 2·sqrt(c² − e²) / c
        let df = 2.0 * (c * c - e * e).sqrt() / c;
        let next = (c - f / df.max(1e-9)).max(e + (c - e) * 0.1);
        if (next - c).abs() < 1e-12 {
            return Some(next);
        }
        c = next;
    }
    Some(c)
}

/// Wrap angle on the small pulley (rad).
pub fn wrap_angle(d: f64, dd: f64, c: f64) -> f64 {
    PI - 2.0 * ((dd - d).abs() / (2.0 * c)).clamp(-1.0, 1.0).asin()
}

/// Effective tension (N) for torque `t` (N·m) on a pulley of pitch diameter `d` (mm).
pub fn effective_tension(t: f64, d: f64) -> f64 {
    2.0 * t / (d / 1000.0)
}

/// Transverse natural frequency (Hz) of a span of length `l` (mm), tension `t` (N),
/// mass per metre `m` (kg/m): f = 1/(2L)·sqrt(T/m').
pub fn span_frequency(l: f64, t: f64, m: f64) -> f64 {
    if t <= 0.0 || m <= 0.0 || l <= 0.0 {
        return 0.0;
    }
    (t / m).sqrt() / (2.0 * l / 1000.0)
}

/// Point on a two-pulley belt loop at arc length `s` (mm) from the start of the lower span.
/// Small pulley at the origin (pitch radius `r1`), large at (`c`, 0) with radius `r2`.
/// Order: lower span → arc on pulley 2 → upper span → arc on pulley 1 (counter-clockwise).
/// Returns (x, y, tangent x, tangent y, segment 0..3).
pub fn belt_point(r1: f64, r2: f64, c: f64, s: f64) -> (f64, f64, f64, f64, usize) {
    let b = ((r2 - r1) / c).clamp(-1.0, 1.0).asin();
    let ls = (c * c - (r2 - r1).powi(2)).max(0.0).sqrt();
    let a2 = r2 * (PI + 2.0 * b);
    let a1 = r1 * (PI - 2.0 * b);
    let total = 2.0 * ls + a1 + a2;
    let mut s = s.rem_euclid(total);
    let (sb, cb) = b.sin_cos();
    if s < ls {
        // lower span, from pulley 1 toward pulley 2
        let p0 = (-r1 * sb, -r1 * cb);
        let p1 = (c - r2 * sb, -r2 * cb);
        let (tx, ty) = ((p1.0 - p0.0) / ls.max(1e-12), (p1.1 - p0.1) / ls.max(1e-12));
        return (p0.0 + tx * s, p0.1 + ty * s, tx, ty, 0);
    }
    s -= ls;
    if s < a2 {
        let a = -PI / 2.0 - b + s / r2;
        return (c + r2 * a.cos(), r2 * a.sin(), -a.sin(), a.cos(), 1);
    }
    s -= a2;
    if s < ls {
        let p0 = (c - r2 * sb, r2 * cb);
        let p1 = (-r1 * sb, r1 * cb);
        let (tx, ty) = ((p1.0 - p0.0) / ls.max(1e-12), (p1.1 - p0.1) / ls.max(1e-12));
        return (p0.0 + tx * s, p0.1 + ty * s, tx, ty, 2);
    }
    s -= ls;
    let a = PI / 2.0 + b + s / r1;
    (r1 * a.cos(), r1 * a.sin(), -a.sin(), a.cos(), 3)
}

/// Tight / slack tensions (N) for pretension `ti` and effective tension `te`.
pub fn side_tensions(ti: f64, te: f64) -> (f64, f64) {
    (ti + te / 2.0, ti - te / 2.0)
}

fn worst(a: Status, b: Status) -> Status {
    a.max(b)
}

/// Evaluate a drive. Loads: the design output torque is the larger of the joint requirement
/// and motor torque × ratio (the belt must survive what the motor can push through it).
pub fn evaluate(inp: &Input) -> Output {
    let pr = PROFILES[inp.profile.min(PROFILES.len() - 1)];
    let b = inp.width.max(0.1);
    let mut checks = vec![];
    let ratio: f64 = inp.stages.iter().map(|s| s.z2 as f64 / s.z1.max(1) as f64).product();
    let capacity = inp.motor_torque * ratio;
    let load = inp.out_torque.max(capacity);
    checks.push(Check {
        stage: None,
        kind: CheckKind::MotorTorque,
        status: if capacity + 1e-9 >= inp.out_torque { Status::Ok } else { Status::Warn },
        value: capacity,
        limit: inp.out_torque,
    });
    let m = pr.mass * b;
    let ta = pr.allow * b;
    let ti = inp.pre_factor * ta;
    let mut stages = vec![];
    let mut rpm = inp.motor_rpm;
    // Torque on each stage's driver, at the design load reflected back from the output.
    let mut t_in = load / ratio;
    let mut package_end: f64 = 0.0;
    let mut x0: f64 = 0.0;
    let mut package_start: f64 = 0.0;
    for (i, st) in inp.stages.iter().enumerate() {
        let (z1, z2) = (st.z1.max(1), st.z2.max(1));
        let d1 = pitch_diameter(z1, pr.pitch);
        let d2 = pitch_diameter(z2, pr.pitch);
        let r = z2 as f64 / z1 as f64;
        let (ds, dl) = (d1.min(d2), d1.max(d2));
        let mut o = StageOut { z1, z2, d1, d2, ratio: r, torque_in: t_in, ti, ..Default::default() };
        // Pulleys must clear each other (outer diameters approximated by pitch diameter + 1 mm).
        let c_min = (d1 + d2) / 2.0 + 1.0;
        let geo_ok = belt_length(ds, dl, st.c.max(c_min)).and_then(|l| {
            let n = (l / pr.pitch).round().max(1.0) as u32;
            let mut n = n;
            // Never round below the clearance limit.
            while centre_distance(ds, dl, n as f64 * pr.pitch).map_or(true, |c| c < c_min) {
                n += 1;
            }
            centre_distance(ds, dl, n as f64 * pr.pitch).map(|c| (n, c))
        });
        let geo_status = if geo_ok.is_some() && st.c >= c_min - 1e-9 { Status::Ok } else { Status::Warn };
        checks.push(Check { stage: Some(i), kind: CheckKind::Geometry, status: geo_status, value: st.c, limit: c_min });
        if let Some((n, c)) = geo_ok {
            o.belt_teeth = n;
            o.belt_len = n as f64 * pr.pitch;
            o.c = c;
            o.span = (c * c - ((dl - ds) / 2.0).powi(2)).sqrt();
            o.wrap = wrap_angle(ds, dl, c);
        }
        let zs = z1.min(z2);
        o.teeth_in_mesh = (zs as f64 * o.wrap / (2.0 * PI)).floor() as u32;
        o.te = effective_tension(t_in, d1);
        o.speed = PI * d1 / 1000.0 * rpm / 60.0;
        o.tc = m * o.speed * o.speed;
        let (t1, t2) = side_tensions(ti, o.te);
        o.t1 = t1;
        o.t2 = t2;
        o.rpm_limit = pr.vmax * 60.0 / (PI * d1 / 1000.0);
        let per_mm = pr.allow - pr.mass * o.speed * o.speed;
        o.width_req = if per_mm > 0.0 { o.te * inp.service / per_mm } else { f64::INFINITY };
        o.freq = span_frequency(o.span, ti, m);
        // Span stiffness k = EA / L for each span; both act while the slack side stays taut.
        let k_span = if o.span > 0.0 { pr.ea * b / (o.span / 1000.0) } else { 0.0 };
        let spans = if t2 > 0.0 { 2.0 } else { 1.0 };
        o.k_rot = spans * k_span * (d2 / 2000.0).powi(2);
        o.shaft_load = (t1 * t1 + t2 * t2 - 2.0 * t1 * t2 * o.wrap.cos()).max(0.0).sqrt();

        let st_of = |ok: bool, warn: bool| if ok { Status::Ok } else if warn { Status::Warn } else { Status::Fail };
        checks.push(Check { stage: Some(i), kind: CheckKind::MinTeeth, status: st_of(zs >= pr.zmin, true), value: zs as f64, limit: pr.zmin as f64 });
        let tim = o.teeth_in_mesh as f64;
        checks.push(Check { stage: Some(i), kind: CheckKind::TeethInMesh, status: st_of(tim >= 6.0, tim >= 3.0), value: tim, limit: 6.0 });
        checks.push(Check { stage: Some(i), kind: CheckKind::SlackSide, status: st_of(t2 > 0.0, false), value: t2, limit: 0.0 });
        checks.push(Check { stage: Some(i), kind: CheckKind::Width, status: st_of(b + 1e-9 >= o.width_req, false), value: o.width_req, limit: b });
        checks.push(Check { stage: Some(i), kind: CheckKind::Speed, status: st_of(o.speed <= pr.vmax, false), value: o.speed, limit: pr.vmax });

        package_start = package_start.min(x0 - d1 / 2.0);
        package_end = package_end.max(x0 + o.c + d2 / 2.0).max(x0 + d1 / 2.0);
        x0 += o.c;
        rpm /= r;
        t_in *= r;
        stages.push(o);
    }
    // Series springs: stage i's stiffness reflected to the output by the square of later ratios.
    let mut compliance = 0.0;
    let mut later = 1.0;
    for s in stages.iter().rev() {
        compliance += if s.k_rot > 0.0 { 1.0 / (s.k_rot * later * later) } else { f64::INFINITY };
        later *= s.ratio;
    }
    let k_out = if compliance > 0.0 { 1.0 / compliance } else { 0.0 };
    let defl = if k_out > 0.0 { load / k_out } else { f64::INFINITY };
    let status = checks.iter().fold(Status::Ok, |a, c| worst(a, c.status));
    Output {
        stages,
        ratio,
        load,
        k_out,
        defl_arcmin: defl.to_degrees() * 60.0,
        out_rpm: rpm,
        out_capacity: capacity,
        package: package_end - package_start,
        checks,
        status,
    }
}

/// Tight / slack tension (N) of the last stage versus output torque (N·m), sampled at `n` points.
pub fn tension_curve(inp: &Input, t_max: f64, n: usize) -> Vec<(f64, f64, f64)> {
    let out = evaluate(inp);
    let Some(last) = out.stages.last() else { return vec![] };
    let r = last.ratio;
    (0..n)
        .map(|k| {
            let t = t_max * k as f64 / (n.max(2) - 1) as f64;
            let (t1, t2) = side_tensions(last.ti, effective_tension(t / r, last.d1));
            (t, t1, t2)
        })
        .collect()
}

/// Output torque (N·m) at which the last stage's slack side reaches zero tension.
pub fn jump_torque(inp: &Input) -> f64 {
    let out = evaluate(inp);
    out.stages.last().map_or(0.0, |s| s.ti * s.d1 / 1000.0 * s.ratio)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub stages: Vec<StageIn>,
    pub width: f64,
    pub ratio: f64,
    pub err: f64,
    pub package: f64,
    pub passes: bool,
}

/// Centre distance used for a search candidate: the stage's current target C, or the
/// clearance minimum plus 3 pitches when that is larger.
fn search_c(z1: u32, z2: u32, p: f64, c_hint: f64) -> f64 {
    let c_min = (pitch_diameter(z1, p) + pitch_diameter(z2, p)) / 2.0 + 1.0 + 3.0 * p;
    c_hint.max(c_min)
}

/// Search pulley teeth (z1 ∈ [zmin..40], z2 ∈ [z1..120]) and widths for a total ratio within
/// `tol` (fraction). For each geometry the narrowest passing width is kept. Returns up to
/// `limit` candidates that pass every check, sorted by (|ratio error|, package length).
pub fn search(base: &Input, target: f64, tol: f64, n_stages: usize, limit: usize) -> Vec<Candidate> {
    let pr = PROFILES[base.profile.min(PROFILES.len() - 1)];
    let p = pr.pitch;
    let hint = |i: usize| base.stages.get(i).map_or(0.0, |s| s.c);
    let mut out: Vec<Candidate> = vec![];
    let mut try_stages = |stages: Vec<StageIn>, out: &mut Vec<Candidate>| {
        for &w in WIDTHS.iter() {
            let inp = Input { stages: stages.clone(), width: w, ..base.clone() };
            let o = evaluate(&inp);
            if o.status == Status::Ok {
                let err = (o.ratio - target).abs() / target;
                out.push(Candidate { stages, width: w, ratio: o.ratio, err, package: o.package, passes: true });
                return;
            }
        }
    };
    let zlo = pr.zmin;
    for z1 in zlo..=40 {
        for z2 in z1..=120 {
            let r1 = z2 as f64 / z1 as f64;
            let s1 = StageIn { z1, z2, c: search_c(z1, z2, p, hint(0)) };
            if n_stages <= 1 {
                if ((r1 - target) / target).abs() <= tol {
                    try_stages(vec![s1], &mut out);
                }
                continue;
            }
            let need = target / r1;
            if need < 1.0 - tol {
                break;
            }
            for y1 in zlo..=40 {
                let mid = (y1 as f64 * need).round() as u32;
                for y2 in mid.saturating_sub(1)..=mid + 1 {
                    if y2 < y1 || y2 > 120 {
                        continue;
                    }
                    let r = r1 * y2 as f64 / y1 as f64;
                    if ((r - target) / target).abs() <= tol {
                        let s2 = StageIn { z1: y1, z2: y2, c: search_c(y1, y2, p, hint(1)) };
                        try_stages(vec![s1, s2], &mut out);
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| {
        b.passes
            .cmp(&a.passes)
            .then(a.err.total_cmp(&b.err))
            .then(a.package.total_cmp(&b.package))
            .then(a.width.total_cmp(&b.width))
    });
    out.truncate(limit);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) {
        assert!((a - b).abs() < tol, "{a} vs {b}");
    }

    #[test]
    fn pitch_diameter_20t_2mm() {
        close(pitch_diameter(20, 2.0), 12.732, 1e-3);
    }

    #[test]
    fn equal_pulleys_length_is_2c_plus_pi_d() {
        let d = pitch_diameter(30, 3.0);
        close(belt_length(d, d, 80.0).unwrap(), 160.0 + PI * d, 1e-9);
        assert!(belt_length(10.0, 50.0, 15.0).is_none(), "too short");
    }

    #[test]
    fn centre_distance_round_trip() {
        for &(d, dd, c) in &[(12.7, 38.2, 40.0), (15.0, 90.0, 46.0), (20.0, 20.0, 100.0), (10.0, 110.0, 300.0)] {
            let l = belt_length(d, dd, c).unwrap();
            close(centre_distance(d, dd, l).unwrap(), c, 1e-6);
            close(centre_distance(dd, d, l).unwrap(), c, 1e-6);
        }
        assert!(centre_distance(10.0, 50.0, 50.0).is_none());
    }

    #[test]
    fn wrap_angle_equal_and_unequal() {
        close(wrap_angle(20.0, 20.0, 50.0), PI, 1e-12);
        let w = wrap_angle(20.0, 60.0, 50.0);
        assert!(w < PI && w > PI / 2.0);
        close(w, PI - 2.0 * (0.4f64).asin(), 1e-12);
    }

    #[test]
    fn effective_tension_is_2t_over_d() {
        close(effective_tension(1.0, 20.0), 100.0, 1e-9);
        close(effective_tension(0.5, 12.732), 2.0 * 0.5 / 0.012732, 1e-6);
    }

    #[test]
    fn low_pretension_flags_slack_side() {
        let mut inp = Input { stages: vec![StageIn { z1: 20, z2: 60, c: 60.0 }], ..Input::default() };
        inp.pre_factor = 0.02;
        let o = evaluate(&inp);
        assert!(o.stages[0].t2 <= 0.0);
        assert!(o.checks.iter().any(|c| c.kind == CheckKind::SlackSide && c.status == Status::Fail));
        assert_eq!(o.status, Status::Fail);
        inp.pre_factor = 0.9;
        inp.out_torque = 0.5;
        inp.motor_torque = 0.1;
        let o = evaluate(&inp);
        assert!(o.stages[0].t2 > 0.0);
        assert!(o.checks.iter().any(|c| c.kind == CheckKind::SlackSide && c.status == Status::Ok));
    }

    #[test]
    fn span_frequency_scales() {
        let f = span_frequency(100.0, 50.0, 0.03);
        close(span_frequency(100.0, 200.0, 0.03), 2.0 * f, 1e-9);
        close(span_frequency(200.0, 50.0, 0.03), f / 2.0, 1e-9);
        close(f, (50.0f64 / 0.03).sqrt() / 0.2, 1e-9);
    }

    #[test]
    fn stiffness_doubles_with_width() {
        for stages in [vec![StageIn { z1: 20, z2: 60, c: 60.0 }], Input::default().stages] {
            let inp = Input { stages, width: 10.0, pre_factor: 0.8, out_torque: 0.2, motor_torque: 0.01, ..Input::default() };
            let a = evaluate(&inp);
            let b = evaluate(&Input { width: 20.0, ..inp });
            assert!(a.stages.iter().chain(&b.stages).all(|s| s.t2 > 0.0));
            close(b.k_out / a.k_out, 2.0, 1e-9);
        }
    }

    #[test]
    fn belt_path_closes_and_matches_length() {
        let (r1, r2, c) = (8.0, 25.0, 60.0);
        let l = belt_length(2.0 * r1, 2.0 * r2, c).unwrap();
        let (x0, y0, ..) = belt_point(r1, r2, c, 0.0);
        let (x1, y1, ..) = belt_point(r1, r2, c, l - 1e-9);
        close(x0, x1, 1e-6);
        close(y0, y1, 1e-6);
        // Numerical arc length of the path equals the open-belt length.
        let n = 20000;
        let mut acc = 0.0;
        let mut prev = belt_point(r1, r2, c, 0.0);
        for k in 1..=n {
            let p = belt_point(r1, r2, c, l * k as f64 / n as f64);
            acc += ((p.0 - prev.0).powi(2) + (p.1 - prev.1).powi(2)).sqrt();
            prev = p;
        }
        close(acc, l, 1e-3);
    }

    #[test]
    fn search_results_pass_and_are_sorted() {
        for stages in [1, 2] {
            // One stage of 4:1 delivers 1.4 N·m from the default motor; ask for 1.2 N·m.
            let base = Input { out_torque: if stages == 1 { 1.2 } else { 3.0 }, ..Input::default() };
            let target = if stages == 1 { 4.0 } else { 9.0 };
            let res = search(&base, target, 0.03, stages, 12);
            assert!(!res.is_empty(), "{stages} stage(s)");
            assert!(res.len() <= 12);
            for c in &res {
                let o = evaluate(&Input { stages: c.stages.clone(), width: c.width, ..base.clone() });
                assert_eq!(o.status, Status::Ok);
                assert!(c.passes && c.err <= 0.03 + 1e-12 && c.stages.len() == stages);
            }
            for w in res.windows(2) {
                assert!(w[0].err < w[1].err || (w[0].err == w[1].err && w[0].package <= w[1].package));
            }
        }
    }

    #[test]
    fn default_example_is_ok() {
        let o = evaluate(&Input::default());
        assert_eq!(o.status, Status::Ok, "{:#?}", o.checks);
    }
}

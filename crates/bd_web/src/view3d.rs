//! 3D scene: toothed pulleys, flanges, shafts, motor, frame and an animated tooth-block belt
//! for 1 or 2 stages, with tension colouring, exaggerated sag / deflection, shaft-load arrows,
//! orbit camera presets and direct manipulation (drag driven pulley → C, drag output rim → load).
use crate::gl::{self, Geo, Mesh, Paint, Renderer};
use crate::math::{self, Orbit, OrbitCam, M4, V};
use bd_core::{belt_point, Input, Output, Profile, PROFILES};
use std::f32::consts::{PI, TAU};
use web_sys::HtmlCanvasElement;

/// Scene units per millimetre.
pub const S: f32 = 0.02;

#[derive(Clone, Copy, Default)]
pub struct Toggles {
    pub tension: bool,
    pub defl: bool,
    pub force: bool,
    pub dims: bool,
    pub play: bool,
}

#[derive(Clone, Copy, Default)]
struct StageGeo {
    z1: u32,
    z2: u32,
    r1: f32,
    r2: f32,
    c: f32,
    len: f32,
    ratio: f32,
    ti: f32,
    te: f32,
    /// Driver centre x (mm) and plane z (mm).
    x0: f32,
    z: f32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Target {
    /// Driven pulley of a stage: drag along the centre line to change C.
    Centre(usize),
    /// Output pulley rim: drag to rotate / load the output.
    Rim,
}

pub struct Scene {
    pub r: Renderer,
    pub cam: OrbitCam,
    home: Orbit,
    stages: Vec<StageGeo>,
    profile: Profile,
    width: f32,
    ta: f32,
    load: f32,
    defl_rad: f32,
    height: f32,
    key: String,
    /// One mesh per pulley: (stage, is driven, mesh).
    pulleys: Vec<(usize, bool, Mesh)>,
    statics: Vec<(Mesh, M4, [f32; 4])>,
    unit_cube: Geo,
    belt: Option<Mesh>,
    arrow: Mesh,
    /// Motor angle (rad) and load fraction of the design load.
    pub theta: f32,
    pub lambda: f32,
    pub hover: Option<Target>,
    pub toggles: Toggles,
    pub vp: M4,
    pub size: (f32, f32),
}

fn rz_basis(phi: f32, s: f32, t: V) -> M4 {
    // Local prism frame (outline in xz, extrusion along y) → world (outline in xy, axis along z),
    // rotated by `phi` about z.
    let (sn, cs) = phi.sin_cos();
    math::basis([cs, sn, 0.0], [0.0, 0.0, 1.0], [sn, -cs, 0.0], [s; 3], t)
}

fn circle(r: f32, n: usize) -> Vec<[f32; 2]> {
    (0..n).map(|i| {
        let a = i as f32 / n as f32 * TAU;
        [r * a.cos(), -r * a.sin()]
    }).collect()
}

/// Toothed pulley outline (mm) with grooves centred at angles 2πk/z, plus a matching bore loop.
fn pulley_geo(z: u32, pr: &Profile, width: f32) -> Geo {
    let p = pr.pitch as f32;
    let r = z as f32 * p / TAU;
    let ro = r - 0.25;
    let ri = ro - pr.tooth_h as f32;
    let bore = (r * 0.25).clamp(1.5, 6.0);
    let mut outer = vec![];
    let step = TAU / z as f32;
    let gw = 0.5 * p / r * 0.5; // half groove width (angle) at the rim
    let gb = gw * 0.65; // half width at the groove bottom
    for k in 0..z {
        let a = k as f32 * step;
        for (da, rad) in [(-gw, ro), (-gb, ri), (gb, ri), (gw, ro), (step * 0.5, ro)] {
            let aa = a + da;
            outer.push([rad * aa.cos(), -rad * aa.sin()]);
        }
    }
    let inner = circle(bore, outer.len());
    let h = width + 1.0;
    let mut g = gl::prism(&inner, &outer, h);
    // Flanges on both faces.
    for y0 in [-0.8, h] {
        let n = 48;
        // Ring flanges, so the toothed face stays visible.
        let f = gl::prism(&circle((ri - 1.2).max(bore + 0.5), n), &circle(ro + 1.6, n), 0.8);
        for (i, v) in f.pos.iter().enumerate() {
            g.pos.push(if i % 3 == 1 { v + y0 } else { *v });
        }
        g.nrm.extend(f.nrm);
    }
    // Hub on the back face.
    let n = 32;
    let hub = gl::prism(&circle(bore, n), &circle((bore * 2.2).min(ri - 0.5).max(bore + 1.0), n), (width * 0.5).max(4.0));
    for (i, v) in hub.pos.iter().enumerate() {
        g.pos.push(if i % 3 == 1 { v - 0.8 - (width * 0.5).max(4.0) } else { *v });
    }
    g.nrm.extend(hub.nrm);
    g
}

fn arrow_geo() -> Geo {
    let c = gl::cylinder(12);
    let shaft = math::basis([1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [0.35, 0.75, 0.35], [0.0, 0.375, 0.0]);
    let w = 0.45;
    let head = gl::frustum([0.0, 1.0, 0.0], &[[-w, 0.75, -w], [w, 0.75, -w], [w, 0.75, w], [-w, 0.75, w]]);
    gl::batch(&[(&c, shaft, [1.0; 3]), (&head, math::IDENTITY, [1.0; 3])])
}

fn mix(a: V, b: V, t: f32) -> V {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

impl Scene {
    pub fn new(canvas: &HtmlCanvasElement) -> Option<Scene> {
        let r = Renderer::new(canvas)?;
        let arrow = r.mesh(&arrow_geo())?;
        let home = Orbit { target: [0.0; 3], yaw: 0.5, pitch: 0.35, dist: 6.0 };
        let mut s = Scene {
            r,
            cam: OrbitCam::new(home),
            home,
            stages: vec![],
            profile: PROFILES[0],
            width: 10.0,
            ta: 1.0,
            load: 0.0,
            defl_rad: 0.0,
            height: 50.0,
            key: String::new(),
            pulleys: vec![],
            statics: vec![],
            unit_cube: gl::cube(),
            belt: None,
            arrow,
            theta: 0.0,
            lambda: 1.0,
            hover: None,
            toggles: Toggles { tension: true, dims: true, play: true, ..Toggles::default() },
            vp: math::IDENTITY,
            size: (1.0, 1.0),
        };
        s.r.cell = 0.2;
        s.r.fog = [12.0, 45.0];
        Some(s)
    }

    pub fn total_ratio(&self) -> f32 {
        self.stages.iter().map(|s| s.ratio).product::<f32>().max(1e-6)
    }

    /// Rebuild geometry for a new design. Camera framing is kept unless `reframe`.
    pub fn set_design(&mut self, inp: &Input, out: &Output, reframe: bool) {
        let pr = PROFILES[inp.profile.min(PROFILES.len() - 1)];
        self.profile = pr;
        self.width = inp.width as f32;
        self.ta = (pr.allow * inp.width) as f32;
        self.load = out.load as f32;
        self.defl_rad = (out.defl_arcmin / 60.0).to_radians() as f32;
        let mut x0 = 0.0;
        let pitch_z = self.width + 14.0;
        self.stages = out.stages.iter().enumerate().map(|(i, s)| {
            let g = StageGeo {
                z1: s.z1,
                z2: s.z2,
                r1: (s.d1 / 2.0) as f32,
                r2: (s.d2 / 2.0) as f32,
                c: s.c as f32,
                len: s.belt_len as f32,
                ratio: s.ratio as f32,
                ti: s.ti as f32,
                te: s.te as f32,
                x0,
                z: -(i as f32) * pitch_z,
            };
            x0 += g.c;
            g
        }).collect();
        let rmax = self.stages.iter().map(|s| s.r1.max(s.r2)).fold(10.0, f32::max);
        self.height = rmax + 22.0;
        let key = format!("{}|{}|{:?}", inp.profile, inp.width, self.stages.iter().map(|s| (s.z1, s.z2, s.c.to_bits())).collect::<Vec<_>>());
        if key != self.key {
            self.key = key;
            self.rebuild();
        }
        if reframe {
            let (lo, hi) = self.extent();
            let zmid = self.stages.last().map_or(0.0, |s| s.z) / 2.0;
            let size = (hi - lo).max(2.0 * self.height);
            self.home = Orbit { target: [(lo + hi) / 2.0 * S, self.height * S, zmid * S], yaw: 0.55, pitch: 0.32, dist: size * S * 1.7 };
            self.cam.set_goal(self.home);
        }
        self.belt_dirty();
    }

    fn extent(&self) -> (f32, f32) {
        let lo = self.stages.iter().map(|s| s.x0 - s.r1).fold(0.0, f32::min);
        let hi = self.stages.iter().map(|s| (s.x0 + s.c + s.r2).max(s.x0 + s.r1)).fold(0.0, f32::max);
        (lo - 12.0, hi + 12.0)
    }

    fn rebuild(&mut self) {
        for (_, _, m) in self.pulleys.drain(..) {
            self.r.free(m);
        }
        for (m, ..) in self.statics.drain(..) {
            self.r.free(m);
        }
        let pr = self.profile;
        for (i, s) in self.stages.clone().iter().enumerate() {
            for (driven, z) in [(false, s.z1), (true, s.z2)] {
                if let Some(m) = self.r.mesh(&pulley_geo(z, &pr, self.width)) {
                    self.pulleys.push((i, driven, m));
                }
            }
        }
        // Frame: base plate, back plate, shafts, motor can.
        let (lo, hi) = self.extent();
        let zb = self.stages.last().map_or(0.0, |s| s.z) - self.width / 2.0 - 12.0;
        let h = self.height;
        let rmax = self.stages.iter().map(|s| s.r1.max(s.r2)).fold(10.0, f32::max);
        let cube = gl::cube();
        let frame = [0.42, 0.46, 0.52, 1.0];
        let add = |r: &Renderer, g: &Geo, m: M4, c: [f32; 4], v: &mut Vec<(Mesh, M4, [f32; 4])>| {
            if let Some(mesh) = r.mesh(g) {
                v.push((mesh, m, c));
            }
        };
        let mut statics = vec![];
        let front = self.width / 2.0 + 8.0;
        let base = math::basis([1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [(hi - lo) * S, 6.0 * S, (front - zb + 10.0) * S], [(lo + hi) / 2.0 * S, 3.0 * S, (front + zb - 10.0) / 2.0 * S]);
        add(&self.r, &cube, base, frame, &mut statics);
        let ph = h + rmax + 8.0;
        let back = math::basis([1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [(hi - lo) * S, ph * S, 5.0 * S], [(lo + hi) / 2.0 * S, ph / 2.0 * S, (zb - 2.5) * S]);
        add(&self.r, &cube, back, frame, &mut statics);
        let cyl = gl::cylinder(24);
        let steel = [0.78, 0.80, 0.84, 1.0];
        let mut shafts: Vec<(f32, f32, f32)> = vec![]; // x, front z, radius
        for (i, s) in self.stages.iter().enumerate() {
            let rb = |r: f32| (r * 0.25).clamp(1.5, 6.0);
            if i == 0 {
                shafts.push((s.x0, s.z + self.width / 2.0 + 3.0, rb(s.r1)));
            }
            let zf = if i + 1 == self.stages.len() { s.z + self.width / 2.0 + 3.0 } else { self.stages[i + 1].z + self.width / 2.0 + 3.0 + (self.width + 14.0) };
            shafts.push((s.x0 + s.c, zf, rb(s.r2).max(rb(self.stages.get(i + 1).map_or(0.0, |n| n.r1)))));
        }
        for (x, zf, rad) in shafts {
            let len = zf - zb;
            let m = math::basis([1., 0., 0.], [0., 0., 1.], [0., -1., 0.], [2.0 * rad * S, len * S, 2.0 * rad * S], [x * S, h * S, (zb + len / 2.0) * S]);
            add(&self.r, &cyl, m, steel, &mut statics);
        }
        // Motor can behind the back plate on the input shaft.
        if let Some(s0) = self.stages.first() {
            let mr = (s0.r1 * 2.2).clamp(14.0, 40.0).min(h - 8.0);
            let ml = mr * 1.3;
            let m = math::basis([1., 0., 0.], [0., 0., 1.], [0., -1., 0.], [2.0 * mr * S, ml * S, 2.0 * mr * S], [s0.x0 * S, h * S, (zb - 5.0 - ml / 2.0) * S]);
            add(&self.r, &cyl, m, [0.16, 0.17, 0.19, 1.0], &mut statics);
            let cap = math::basis([1., 0., 0.], [0., 0., 1.], [0., -1., 0.], [2.0 * mr * 0.9 * S, 2.0 * S, 2.0 * mr * 0.9 * S], [s0.x0 * S, h * S, (zb - 5.0 - ml - 1.0) * S]);
            add(&self.r, &cyl, cap, [0.85, 0.55, 0.15, 1.0], &mut statics);
        }
        self.statics = statics;
    }

    fn belt_dirty(&mut self) {
        if let Some(m) = self.belt.take() {
            self.r.free(m);
        }
    }

    /// Angle of the driver of stage `i` (rad, counter-clockwise from +x seen from the front).
    fn driver_angle(&self, i: usize) -> f32 {
        let div: f32 = self.stages[..i].iter().map(|s| s.ratio).product();
        self.theta / div
    }

    fn output_extra(&self) -> f32 {
        // Exaggerated so the design load shows as ~12°.
        if self.toggles.defl { -self.lambda * 0.21 } else { 0.0 }
    }

    fn tension_colour(&self, t: f32, ti: f32) -> V {
        let base = [0.13, 0.13, 0.14];
        if !self.toggles.tension {
            return base;
        }
        if t >= ti {
            mix(base, [0.95, 0.22, 0.15], 0.25 + (t - ti) / (0.5 * self.ta).max(1e-3))
        } else {
            mix(base, [0.20, 0.45, 1.0], 0.25 + (ti - t) / ti.max(1e-3))
        }
    }

    fn build_belt(&self) -> Geo {
        let pr = self.profile;
        let p = pr.pitch as f32;
        let th = pr.tooth_h as f32;
        let band = 0.25 * p;
        let mut parts: Vec<(M4, V)> = vec![];
        for (i, s) in self.stages.iter().enumerate() {
            let te = s.te * self.lambda;
            let (t1, t2) = (s.ti + te / 2.0, s.ti - te / 2.0);
            let shift = self.driver_angle(i) * s.r1;
            let n = (s.len / p).round().max(1.0) as usize;
            let ls = (s.c * s.c - (s.r2 - s.r1).powi(2)).max(0.0).sqrt();
            let a2 = s.r2 * (PI + 2.0 * ((s.r2 - s.r1) / s.c).asin());
            let sag = if self.toggles.defl {
                let q = if t2 > 0.0 { (te / 2.0) / s.ti.max(1e-3) } else { 2.0 };
                0.07 * ls * q.clamp(0.0, 2.0)
            } else {
                0.0
            };
            for k in 0..n {
                let sk = k as f32 * p + shift;
                let (x, y, tx, ty, seg) = belt_point(s.r1 as f64, s.r2 as f64, s.c as f64, sk as f64);
                let (mut x, mut y, tx, ty) = (x as f32, y as f32, tx as f32, ty as f32);
                let (nx, ny) = (ty, -tx); // outward
                let local = (sk.rem_euclid(s.len)) as f32;
                let tension = match seg {
                    0 => t2,
                    1 => t2 + (t1 - t2) * ((local - ls) / a2).clamp(0.0, 1.0),
                    2 => t1,
                    _ => t1 + (t2 - t1) * ((local - 2.0 * ls - a2) / (s.len - 2.0 * ls - a2).max(1e-3)).clamp(0.0, 1.0),
                };
                if seg == 0 && sag > 0.0 {
                    let u = (local / ls.max(1e-3)).clamp(0.0, 1.0);
                    let d = sag * (PI * u).sin();
                    x += nx * d;
                    y += ny * d;
                }
                let col = self.tension_colour(tension, s.ti);
                let wx = s.x0 + x;
                let wy = self.height + y;
                let xa = [tx, ty, 0.0];
                let ya = [nx, ny, 0.0];
                let za = [0.0, 0.0, -1.0];
                let c_band = [(wx + nx * band / 2.0) * S, (wy + ny * band / 2.0) * S, s.z * S];
                parts.push((math::basis(xa, ya, za, [p * 1.04 * S, band * S, self.width * S], c_band), col));
                let tc = [(wx - nx * th / 2.0) * S, (wy - ny * th / 2.0) * S, s.z * S];
                parts.push((math::basis(xa, ya, za, [p * 0.42 * S, th * S, self.width * S], tc), mix(col, [0.0; 3], 0.25)));
            }
        }
        let refs: Vec<(&Geo, M4, V)> = parts.iter().map(|(m, c)| (&self.unit_cube, *m, *c)).collect();
        gl::batch(&refs)
    }

    fn pulley_model(&self, i: usize, driven: bool) -> M4 {
        let s = &self.stages[i];
        let pr = self.profile;
        let (x, r, phi0, ang) = if driven {
            let mut a = self.driver_angle(i) * s.r1 / s.r2;
            if i + 1 == self.stages.len() {
                a += self.output_extra();
            }
            // Groove phase so belt teeth sit in grooves (see belt_point segment 1 start).
            let b = ((s.r2 - s.r1) / s.c).asin();
            let ls = (s.c * s.c - (s.r2 - s.r1).powi(2)).max(0.0).sqrt();
            (s.x0 + s.c, s.r2, -PI / 2.0 - b - ls / s.r2, a)
        } else {
            let b = ((s.r2 - s.r1) / s.c).asin();
            let ls = (s.c * s.c - (s.r2 - s.r1).powi(2)).max(0.0).sqrt();
            let a2 = s.r2 * (PI + 2.0 * b);
            let s4 = 2.0 * ls + a2;
            (s.x0, s.r1, PI / 2.0 + b - s4 / s.r1, self.driver_angle(i))
        };
        let _ = (r, pr);
        rz_basis(phi0 + ang, S, [x * S, self.height * S, (s.z - self.width / 2.0 - 0.5) * S])
    }

    /// Advance animation and camera; returns true if a redraw is needed.
    pub fn step(&mut self, dt: f32, speed: f32) -> bool {
        let mut moving = self.cam.step(dt);
        if self.toggles.play {
            self.theta += dt * speed * TAU;
            self.belt_dirty();
            moving = true;
        }
        moving
    }

    pub fn render(&mut self, w: i32, h: i32) {
        let aspect = w as f32 / h.max(1) as f32;
        let o = self.cam.cur;
        let eye = o.eye();
        let view = math::look_at(eye, o.target, [0.0, 1.0, 0.0]);
        // Widen the vertical field of view on portrait screens so the drive still fits across.
        let fovy = if aspect < 1.3 { 2.0 * ((0.4f32).tan() * 1.3 / aspect).atan() } else { 0.8 };
        let proj = math::perspective(fovy, aspect, (o.dist * 0.02).max(0.01), o.dist * 30.0 + 50.0);
        self.vp = math::mul(&proj, &view);
        self.r.begin(&view, &proj, eye, [0, 0, w, h]);
        if self.belt.is_none() {
            let g = self.build_belt();
            self.belt = self.r.mesh(&g);
        }
        for (m, model, _) in &self.statics {
            self.r.shadow(m, model);
        }
        for (i, d, m) in &self.pulleys {
            self.r.shadow(m, &self.pulley_model(*i, *d));
        }
        if let Some(b) = &self.belt {
            self.r.shadow(b, &math::IDENTITY);
        }
        for (m, model, c) in &self.statics {
            self.r.draw(m, model, Paint::lit(*c));
        }
        let n = self.stages.len();
        for (i, d, m) in &self.pulleys {
            let hot = *d && (self.hover == Some(Target::Centre(*i)) || (i + 1 == n && self.hover == Some(Target::Rim)));
            let col = if *d { [0.72, 0.74, 0.78, 1.0] } else { [0.62, 0.66, 0.72, 1.0] };
            let mut p = Paint::lit(col);
            p.glow = if hot { 0.28 } else { 0.0 };
            self.r.draw(m, &self.pulley_model(*i, *d), p);
        }
        if let Some(b) = &self.belt {
            self.r.draw(b, &math::IDENTITY, Paint::lit([1.0; 4]));
        }
        if self.toggles.force {
            self.draw_forces();
        }
    }

    /// Shaft load vectors on each pulley (belt pulls the pulleys toward each other).
    fn draw_forces(&self) {
        let fmax = self.stages.iter().map(|s| s.ti * 2.0 + s.te).fold(1.0, f32::max);
        for s in &self.stages {
            let te = s.te * self.lambda;
            let (t1, t2) = (s.ti + te / 2.0, (s.ti - te / 2.0).max(0.0));
            let b = ((s.r2 - s.r1) / s.c).asin();
            let (sb, cb) = b.sin_cos();
            let ls = (s.c * s.c - (s.r2 - s.r1).powi(2)).max(1e-3).sqrt();
            // Span directions from pulley 1 toward pulley 2: upper (tight), lower (slack).
            let up = [(s.c - (s.r2 - s.r1) * sb) / ls, ((s.r2 - s.r1) * cb) / ls];
            let lo = [(s.c - (s.r2 - s.r1) * sb) / ls, (-(s.r2 - s.r1) * cb) / ls];
            let f = [t1 * up[0] + t2 * lo[0], t1 * up[1] + t2 * lo[1]];
            for (x, sign) in [(s.x0, 1.0), (s.x0 + s.c, -1.0)] {
                let mag = (f[0] * f[0] + f[1] * f[1]).sqrt();
                if mag < 1e-3 {
                    continue;
                }
                let d = [f[0] / mag * sign, f[1] / mag * sign, 0.0];
                let size = s.r1.max(s.r2);
                let len = size * (0.5 + 1.0 * mag / fmax);
                let w = (size * 0.12).clamp(1.5, 5.0);
                let m = math::basis([d[1], -d[0], 0.0], d, [0.0, 0.0, 1.0], [w * S, len * S, w * S], [x * S, self.height * S, (s.z + self.width / 2.0 + 6.0) * S]);
                let mut paint = Paint::lit([1.0, 0.75, 0.1, 1.0]);
                paint.glow = 0.15;
                self.r.draw(&self.arrow, &m, paint);
            }
        }
    }

    /// Screen position (CSS px) of a world point, if in front of the camera.
    pub fn project(&self, p: V) -> Option<(f32, f32)> {
        let r = |i: usize| self.vp[i] * p[0] + self.vp[4 + i] * p[1] + self.vp[8 + i] * p[2] + self.vp[12 + i];
        let w = r(3);
        if w <= 1e-4 {
            return None;
        }
        Some(((r(0) / w * 0.5 + 0.5) * self.size.0, (0.5 - r(1) / w * 0.5) * self.size.1))
    }

    /// Dimension labels: (world point, text).
    pub fn labels(&self) -> Vec<(V, String)> {
        let mut v = vec![];
        for s in &self.stages {
            let zf = (s.z + self.width / 2.0 + 2.0) * S;
            v.push(([(s.x0 + s.c / 2.0) * S, (self.height + s.r1.max(s.r2) * 0.15) * S, zf], format!("C {:.1} mm", s.c)));
            v.push(([s.x0 * S, (self.height + s.r1 + 6.0) * S, zf], format!("{}T  ⌀{:.1}", s.z1, 2.0 * s.r1)));
            v.push(([(s.x0 + s.c) * S, (self.height + s.r2 + 6.0) * S, zf], format!("{}T  ⌀{:.1}", s.z2, 2.0 * s.r2)));
            v.push(([(s.x0 + s.c / 2.0) * S, (self.height - s.r1.max(s.r2) - 6.0) * S, zf], format!("{}T belt  {:.0} mm", (s.len / self.profile.pitch as f32).round(), s.len)));
        }
        v
    }

    /// Centre line segments (world) drawn as reference in the HUD layer.
    pub fn centre_points(&self) -> Vec<(V, V)> {
        self.stages.iter().map(|s| {
            let zf = (s.z + self.width / 2.0 + 2.0) * S;
            ([s.x0 * S, self.height * S, zf], [(s.x0 + s.c) * S, self.height * S, zf])
        }).collect()
    }

    fn ray(&self, x: f32, y: f32) -> Option<(V, V)> {
        let inv = math::invert(&self.vp)?;
        Some(math::ndc_ray(&inv, x / self.size.0 * 2.0 - 1.0, 1.0 - y / self.size.1 * 2.0))
    }

    /// Hit point (mm, in the drive plane of stage `i`) under the cursor.
    fn plane_hit(&self, i: usize, x: f32, y: f32) -> Option<(f32, f32)> {
        let (o, d) = self.ray(x, y)?;
        let s = self.stages.get(i)?;
        let p = math::ray_plane(o, d, [0.0, 0.0, (s.z + self.width / 2.0) * S], [0.0, 0.0, 1.0])?;
        Some((p[0] / S, p[1] / S - self.height))
    }

    pub fn pick(&self, x: f32, y: f32) -> Option<Target> {
        let n = self.stages.len();
        for (i, s) in self.stages.iter().enumerate() {
            let Some((hx, hy)) = self.plane_hit(i, x, y) else { continue };
            let d = ((hx - s.x0 - s.c).powi(2) + hy * hy).sqrt();
            if d < s.r2 + 3.0 {
                return Some(if i + 1 == n && d > 0.6 * s.r2 { Target::Rim } else { Target::Centre(i) });
            }
        }
        None
    }

    /// Centre distance (mm) the cursor asks for while dragging stage `i`'s driven pulley.
    pub fn drag_centre(&self, i: usize, x: f32, y: f32) -> Option<f32> {
        let (hx, _) = self.plane_hit(i, x, y)?;
        Some(hx - self.stages[i].x0)
    }

    /// Cursor angle (rad) around the output pulley centre.
    pub fn output_angle(&self, x: f32, y: f32) -> Option<f32> {
        let i = self.stages.len().checked_sub(1)?;
        let s = &self.stages[i];
        let (hx, hy) = self.plane_hit(i, x, y)?;
        Some(hy.atan2(hx - s.x0 - s.c))
    }

    pub fn reset_view(&mut self) {
        self.cam.set_goal(self.home);
    }

    /// Camera presets: 0 perspective, 1 front, 2 side, 3 driver pulley close-up.
    pub fn preset(&mut self, k: u32) {
        let h = self.home;
        let o = match k {
            1 => Orbit { yaw: 0.0, pitch: 0.0, ..h },
            2 => Orbit { yaw: PI / 2.0, pitch: 0.05, dist: h.dist * 0.9, ..h },
            3 => match self.stages.first() {
                Some(s) => Orbit { target: [s.x0 * S, self.height * S, s.z * S], yaw: 0.45, pitch: 0.3, dist: (s.r1 * 5.0 + 20.0) * S },
                None => h,
            },
            _ => h,
        };
        self.cam.set_goal(o);
    }

    /// Camera right / up vectors (world) for panning.
    pub fn cam_axes(&self) -> (V, V) {
        let v = math::look_at(self.cam.cur.eye(), self.cam.cur.target, [0.0, 1.0, 0.0]);
        ([v[0], v[4], v[8]], [v[1], v[5], v[9]])
    }

    pub fn defl_rad(&self) -> f32 {
        self.defl_rad
    }

    pub fn load(&self) -> f32 {
        self.load
    }
}

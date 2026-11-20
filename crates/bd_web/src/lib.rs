//! Web UI for belt-drive: inputs, HUD, checks, plots, reduction search and the 3D view.
//! Everything runs client-side; the only storage is optional localStorage for language and hint.
pub mod gl;
pub mod i18n;
pub mod math;
pub mod plot;
pub mod view3d;

use bd_core::{evaluate, jump_torque, search, tension_curve, Candidate, CheckKind, Input, Output, StageIn, Status, PROFILES};
use i18n::{t, Lang};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use view3d::{Scene, Target};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Document, Element, HtmlCanvasElement, HtmlElement, HtmlInputElement, HtmlSelectElement, PointerEvent, WheelEvent};

#[derive(Clone, Copy, PartialEq)]
enum Drag {
    None,
    Orbit,
    Pan,
    Centre(usize),
    Rim { start_angle: f32, start_lambda: f32 },
}

struct App {
    doc: Document,
    lang: Lang,
    dark: bool,
    inp: Input,
    out: Output,
    scene: Option<Scene>,
    results: Vec<Candidate>,
    pointers: HashMap<i32, (f32, f32)>,
    last: (f32, f32),
    pinch: Option<(f32, (f32, f32))>,
    drag: Drag,
    last_t: f64,
    speed: f32,
    dirty: bool,
}

fn el(doc: &Document, id: &str) -> Element {
    doc.get_element_by_id(id).unwrap_or_else(|| doc.create_element("div").unwrap())
}
fn input(doc: &Document, id: &str) -> Option<HtmlInputElement> {
    doc.get_element_by_id(id)?.dyn_into().ok()
}
fn select(doc: &Document, id: &str) -> Option<HtmlSelectElement> {
    doc.get_element_by_id(id)?.dyn_into().ok()
}
fn num(doc: &Document, id: &str, def: f64) -> f64 {
    input(doc, id).and_then(|e| e.value().trim().parse::<f64>().ok()).filter(|v| v.is_finite()).unwrap_or(def)
}
fn set_num(doc: &Document, id: &str, v: f64, dec: usize) {
    if let Some(e) = input(doc, id) {
        e.set_value(&format!("{v:.dec$}"));
    }
}
fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}
fn status_class(s: Status) -> &'static str {
    match s {
        Status::Ok => "s-ok",
        Status::Warn => "s-warn",
        Status::Fail => "s-fail",
    }
}
fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

const STAGE_IDS: [[&str; 3]; 2] = [["z1a", "z2a", "ca"], ["z1b", "z2b", "cb"]];

impl App {
    fn read_inputs(&mut self) {
        let d = &self.doc;
        let n = select(d, "stages").map_or(2, |s| s.value().parse().unwrap_or(2)).clamp(1, 2);
        let def = Input::default();
        let stages = (0..n)
            .map(|i| {
                let ids = STAGE_IDS[i];
                let ds = def.stages[i];
                StageIn {
                    z1: num(d, ids[0], ds.z1 as f64).round().clamp(6.0, 300.0) as u32,
                    z2: num(d, ids[1], ds.z2 as f64).round().clamp(6.0, 400.0) as u32,
                    c: num(d, ids[2], ds.c).clamp(1.0, 5000.0),
                }
            })
            .collect();
        self.inp = Input {
            profile: select(d, "profile").map_or(1, |s| s.value().parse().unwrap_or(1)).min(PROFILES.len() - 1),
            stages,
            width: num(d, "width", def.width).clamp(1.0, 200.0),
            motor_torque: num(d, "mt", def.motor_torque).max(0.0),
            motor_rpm: num(d, "rpm", def.motor_rpm).max(0.0),
            out_torque: num(d, "ot", def.out_torque).max(0.0),
            pre_factor: num(d, "pf", def.pre_factor).clamp(0.0, 3.0),
            service: num(d, "sf", def.service).clamp(1.0, 5.0),
        };
        let _ = el(d, "card2").dyn_into::<HtmlElement>().map(|e| e.style().set_property("display", if n == 2 { "" } else { "none" }));
    }

    fn write_inputs(&self) {
        let d = &self.doc;
        if let Some(s) = select(d, "stages") {
            s.set_value(&self.inp.stages.len().to_string());
        }
        set_num(d, "width", self.inp.width, 0);
        for (i, st) in self.inp.stages.iter().enumerate() {
            set_num(d, STAGE_IDS[i][0], st.z1 as f64, 0);
            set_num(d, STAGE_IDS[i][1], st.z2 as f64, 0);
            set_num(d, STAGE_IDS[i][2], st.c, 1);
        }
    }

    fn recompute(&mut self, reframe: bool) {
        self.out = evaluate(&self.inp);
        if let Some(s) = self.scene.as_mut() {
            s.set_design(&self.inp, &self.out, reframe);
        }
        self.hud();
        self.checks();
        self.plots();
        self.dirty = true;
    }

    fn hud(&self) {
        let (l, o) = (self.lang, &self.out);
        let mut h = String::new();
        let mut row = |k: &str, v: String| h.push_str(&format!("<span class=\"k\">{}</span><span>{}</span>", t(l, k), v));
        let j = |f: &dyn Fn(&bd_core::StageOut) -> String| o.stages.iter().map(f).collect::<Vec<_>>().join(" | ");
        row("u_ratio", format!("{:.3} : 1", o.ratio));
        row("u_belt", j(&|s| format!("{}T / {:.0} mm", s.belt_teeth, s.belt_len)));
        row("u_c", j(&|s| format!("{:.2} mm", s.c)));
        row("u_mesh", j(&|s| s.teeth_in_mesh.to_string()));
        let lam = self.scene.as_ref().map_or(1.0, |s| s.lambda as f64);
        row("u_t12", j(&|s| {
            let te = s.te * lam;
            format!("{:.0} / {:.0} N", s.ti + te / 2.0, s.ti - te / 2.0)
        }));
        row("u_width", j(&|s| format!("{:.1} / {:.0} mm", s.width_req, self.inp.width)));
        row("u_rpm", format!("{:.0} / {:.0}", self.inp.motor_rpm, o.motor_rpm_limit));
        row("u_freq", j(&|s| format!("{:.0} Hz", s.freq)));
        row("u_k", format!("{:.0} N·m/rad", o.k_out));
        row("u_defl", format!("{:.1} arcmin", o.defl_arcmin * lam));
        row("u_shaft", j(&|s| format!("{:.0} N", s.shaft_load)));
        row("u_load", format!("{:.2} N·m{}", o.load * lam, if (lam - 1.0).abs() > 1e-3 { format!(" ({:.0}%)", lam * 100.0) } else { String::new() }));
        let st = match o.status {
            Status::Ok => "st_ok",
            Status::Warn => "st_warn",
            Status::Fail => "st_fail",
        };
        h.push_str(&format!("<span class=\"st {}\">{}</span>", status_class(o.status), t(l, st)));
        el(&self.doc, "hud").set_inner_html(&h);
    }

    fn checks(&self) {
        let l = self.lang;
        let mut h = String::new();
        for c in &self.out.checks {
            let key = match c.kind {
                CheckKind::Geometry => "k_geo",
                CheckKind::MinTeeth => "k_zmin",
                CheckKind::TeethInMesh => "k_mesh",
                CheckKind::SlackSide => "k_slack",
                CheckKind::Width => "k_width",
                CheckKind::Speed => "k_speed",
                CheckKind::MotorTorque => "k_motor",
            };
            let f = |v: f64| if v.is_finite() { if v.abs() >= 100.0 { format!("{v:.0}") } else { format!("{v:.1}") } } else { "∞".into() };
            let msg = t(l, key).replace("{v}", &f(c.value)).replace("{l}", &f(c.limit));
            let who = c.stage.map_or(t(l, "drive").to_string(), |i| format!("{}", i + 1));
            let sk = match c.status {
                Status::Ok => "ok",
                Status::Warn => "warn",
                Status::Fail => "fail",
            };
            h.push_str(&format!("<li><b class=\"{}\">{}</b><span>[{}] {}</span></li>", status_class(c.status), t(l, sk), who, esc(&msg)));
        }
        el(&self.doc, "checks").set_inner_html(&h);
    }

    fn plots(&self) {
        let (c1, c2) = if self.dark { ("#f0a030", "#4aa8ff") } else { ("#c87000", "#1a6fd1") };
        if let Some(cv) = self.doc.get_element_by_id("plot-k").and_then(|e| e.dyn_into::<HtmlCanvasElement>().ok()) {
            let mut k = vec![];
            let mut d = vec![];
            let mut b = 3.0;
            let bmax = (self.inp.width * 2.0).max(30.0);
            while b <= bmax + 1e-9 {
                let o = evaluate(&Input { width: b, ..self.inp.clone() });
                k.push((b, o.k_out));
                d.push((b, o.defl_arcmin));
                b += 1.0;
            }
            let p = plot::Plot {
                series: vec![plot::Series { pts: k, color: c1, right: false }, plot::Series { pts: d, color: c2, right: true }],
                marks: vec![(self.inp.width, self.out.k_out, false, c1), (self.inp.width, self.out.defl_arcmin, true, c2)],
                vlines: vec![],
                xlabel: "b (mm)".into(),
                dark: self.dark,
            };
            plot::draw(&cv, &p);
        }
        if let Some(cv) = self.doc.get_element_by_id("plot-t").and_then(|e| e.dyn_into::<HtmlCanvasElement>().ok()) {
            let jt = jump_torque(&self.inp);
            let tmax = (self.out.load * 1.6).max(jt * 1.2).max(0.1);
            let curve = tension_curve(&self.inp, tmax, 40);
            let t1: Vec<_> = curve.iter().map(|c| (c.0, c.1)).collect();
            let t2: Vec<_> = curve.iter().map(|c| (c.0, c.2)).collect();
            let last = self.out.stages.last().copied().unwrap_or_default();
            let lam = self.scene.as_ref().map_or(1.0, |s| s.lambda as f64);
            let te = last.te * lam;
            let fail = if self.dark { "#ff5d5d" } else { "#d12c2c" };
            let p = plot::Plot {
                series: vec![plot::Series { pts: t1, color: c1, right: false }, plot::Series { pts: t2, color: c2, right: false }],
                marks: vec![(self.out.load * lam, last.ti + te / 2.0, false, c1), (self.out.load * lam, last.ti - te / 2.0, false, c2)],
                vlines: vec![(jt, fail)],
                xlabel: "T_out (N·m)".into(),
                dark: self.dark,
            };
            plot::draw(&cv, &p);
        }
    }

    fn apply_lang(&self) {
        let l = self.lang;
        let d = &self.doc;
        if let Some(root) = d.document_element() {
            let _ = root.set_attribute("lang", if l == Lang::Ko { "ko" } else { "en" });
        }
        if let Ok(list) = d.query_selector_all("[data-i18n]") {
            for i in 0..list.length() {
                if let Some(e) = list.item(i).and_then(|n| n.dyn_into::<Element>().ok()) {
                    let k = e.get_attribute("data-i18n").unwrap_or_default();
                    e.set_text_content(Some(t(l, &k)));
                }
            }
        }
        if let Ok(list) = d.query_selector_all("[data-tipk]") {
            for i in 0..list.length() {
                if let Some(e) = list.item(i).and_then(|n| n.dyn_into::<Element>().ok()) {
                    let k = e.get_attribute("data-tipk").unwrap_or_default();
                    let _ = e.set_attribute("data-tip", t(l, &k));
                    // CSS ::after tooltips do not render on form controls; give those a native title.
                    if matches!(e.tag_name().as_str(), "SELECT" | "INPUT") {
                        let _ = e.set_attribute("title", t(l, &k));
                    }
                    let _ = e.set_attribute("aria-label", t(l, &k));
                }
            }
        }
        if let Some(s) = select(d, "profile") {
            let cur = s.value();
            let opts: String = PROFILES
                .iter()
                .enumerate()
                .map(|(i, p)| format!("<option value=\"{i}\">{}</option>", if l == Lang::Ko { p.name_ko } else { p.name_en }))
                .collect();
            s.set_inner_html(&opts);
            s.set_value(if cur.is_empty() { "1" } else { &cur });
        }
        self.hud();
        self.checks();
        self.render_results();
    }

    fn run_search(&mut self) {
        let d = &self.doc;
        let target = num(d, "s-ratio", 9.0).max(1.0);
        let tol = num(d, "s-tol", 3.0).clamp(0.0, 50.0) / 100.0;
        let n = select(d, "s-stages").map_or(2, |s| s.value().parse().unwrap_or(2));
        self.results = search(&self.inp, target, tol, n, 12);
        self.render_results();
    }

    fn render_results(&self) {
        let l = self.lang;
        let d = &self.doc;
        let msg = if self.results.is_empty() { "" } else { t(l, "s_found") };
        el(d, "s-msg").set_text_content(Some(msg));
        let head = format!("<tr><th>{}</th><th>{}</th><th>{}</th><th>{}</th></tr>", t(l, "h_teeth"), t(l, "h_ratio"), t(l, "h_width"), t(l, "h_pkg"));
        let table = el(d, "s-res");
        if let Ok(Some(th)) = table.query_selector("thead") {
            th.set_inner_html(if self.results.is_empty() { "" } else { &head });
        }
        let rows: String = self
            .results
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let teeth = c.stages.iter().map(|s| format!("{}:{}", s.z1, s.z2)).collect::<Vec<_>>().join(" · ");
                format!("<tr data-row=\"{i}\"><td>{teeth}</td><td>{:.3}</td><td>{:.0}</td><td>{:.0}</td></tr>", c.ratio, c.width, c.package)
            })
            .collect();
        if let Ok(Some(tb)) = table.query_selector("tbody") {
            tb.set_inner_html(&rows);
        }
    }

    fn searched_empty(&self) {
        if self.results.is_empty() {
            el(&self.doc, "s-msg").set_text_content(Some(t(self.lang, "s_none")));
        }
    }

    fn load_result(&mut self, i: usize) {
        if let Some(c) = self.results.get(i).cloned() {
            self.inp.stages = c.stages.clone();
            self.inp.width = c.width;
            // Show the real centre distance of the chosen belt in the inputs.
            let o = evaluate(&self.inp);
            for (s, so) in self.inp.stages.iter_mut().zip(&o.stages) {
                s.c = (so.c * 10.0).round() / 10.0;
            }
            self.write_inputs();
            self.read_inputs();
            self.recompute(true);
        }
    }

    fn labels(&self) {
        let d = &self.doc;
        let host = el(d, "labels");
        let Some(sc) = self.scene.as_ref().filter(|s| s.toggles.dims) else {
            host.set_inner_html("");
            return;
        };
        let mut h = String::new();
        for (p, text) in sc.labels() {
            if let Some((x, y)) = sc.project(p) {
                let text = text.replace(" belt ", &format!(" {} ", t(self.lang, "belt")));
                h.push_str(&format!("<div class=\"dim\" style=\"left:{x:.0}px;top:{y:.0}px\">{}</div>", esc(&text)));
            }
        }
        host.set_inner_html(&h);
    }
}

type Shared = Rc<RefCell<App>>;

fn on<E: wasm_bindgen::convert::FromWasmAbi + 'static>(target: &web_sys::EventTarget, ev: &str, f: impl FnMut(E) + 'static) {
    let c = Closure::<dyn FnMut(E)>::new(f);
    let _ = target.add_event_listener_with_callback(ev, c.as_ref().unchecked_ref());
    c.forget();
}

fn toggle_button(doc: &Document, id: &str, on_: bool) {
    let e = el(doc, id);
    let _ = if on_ { e.class_list().add_1("on") } else { e.class_list().remove_1("on") };
}

#[wasm_bindgen(start)]
pub fn start() {
    let Some(win) = web_sys::window() else { return };
    let Some(doc) = win.document() else { return };
    let saved = storage().and_then(|s| s.get_item("bd_lang").ok().flatten());
    let nav = win.navigator().language().unwrap_or_default();
    let lang = match saved.as_deref().unwrap_or(&nav) {
        l if l.starts_with("ko") => Lang::Ko,
        _ => Lang::En,
    };
    let dark = !win.match_media("(prefers-color-scheme: light)").ok().flatten().is_some_and(|m| m.matches());
    let canvas: HtmlCanvasElement = match doc.get_element_by_id("view").and_then(|e| e.dyn_into().ok()) {
        Some(c) => c,
        None => return,
    };
    let mut scene = Scene::new(&canvas);
    if let Some(s) = scene.as_mut() {
        s.r.theme = if dark { gl::DARK } else { gl::LIGHT };
    } else {
        let _ = el(&doc, "nogl").dyn_into::<HtmlElement>().map(|e| e.style().set_property("display", "flex"));
        let _ = canvas.dyn_ref::<HtmlElement>().map(|e| e.style().set_property("display", "none"));
    }
    let app = Rc::new(RefCell::new(App {
        doc: doc.clone(),
        lang,
        dark,
        inp: Input::default(),
        out: evaluate(&Input::default()),
        scene,
        results: vec![],
        pointers: HashMap::new(),
        last: (0.0, 0.0),
        pinch: None,
        drag: Drag::None,
        last_t: 0.0,
        speed: 0.25,
        dirty: true,
    }));
    {
        let mut a = app.borrow_mut();
        a.apply_lang();
        a.write_inputs();
        a.read_inputs();
        a.recompute(true);
        if let Some(s) = a.scene.as_mut() {
            s.cam.cur = s.cam.goal;
        }
    }
    if storage().and_then(|s| s.get_item("bd_hint").ok().flatten()).is_some() {
        let _ = el(&doc, "hint").class_list().add_1("hidden");
    }
    wire_inputs(&app, &doc);
    wire_toolbar(&app, &doc);
    wire_canvas(&app, &canvas);
    {
        let a = app.clone();
        on(&win, "resize", move |_: web_sys::Event| {
            let mut a = a.borrow_mut();
            a.plots();
            a.dirty = true;
        });
    }
    start_loop(app, canvas);
}

fn wire_inputs(app: &Shared, doc: &Document) {
    for id in ["profile", "stages", "width", "mt", "rpm", "ot", "pf", "sf", "z1a", "z2a", "ca", "z1b", "z2b", "cb"] {
        let a = app.clone();
        let reframe = matches!(id, "profile" | "stages");
        let ev = if id == "profile" || id == "stages" { "change" } else { "input" };
        on(&el(doc, id), ev, move |_: web_sys::Event| {
            let mut a = a.borrow_mut();
            a.read_inputs();
            a.recompute(reframe);
        });
    }
    {
        let a = app.clone();
        on(&el(doc, "s-go"), "click", move |_: web_sys::Event| {
            let mut a = a.borrow_mut();
            a.run_search();
            a.searched_empty();
        });
    }
    {
        let a = app.clone();
        let tbody = el(doc, "s-res").query_selector("tbody").ok().flatten().unwrap_or_else(|| el(doc, "s-res"));
        on(&tbody, "click", move |e: web_sys::Event| {
            let row = e
                .target()
                .and_then(|t| t.dyn_into::<Element>().ok())
                .and_then(|t| t.closest("tr").ok().flatten())
                .and_then(|r| r.get_attribute("data-row"))
                .and_then(|r| r.parse::<usize>().ok());
            if let Some(i) = row {
                a.borrow_mut().load_result(i);
            }
        });
    }
    {
        // Mobile: tap the HUD to expand / collapse it.
        let d = doc.clone();
        on(&el(doc, "hud"), "click", move |_: web_sys::Event| {
            let _ = el(&d, "hud").class_list().toggle("open");
        });
    }
    {
        let d = doc.clone();
        on(&el(doc, "hint-x"), "click", move |_: web_sys::Event| {
            let _ = el(&d, "hint").class_list().add_1("hidden");
            if let Some(s) = storage() {
                let _ = s.set_item("bd_hint", "1");
            }
        });
    }
}

fn wire_toolbar(app: &Shared, doc: &Document) {
    for id in ["b-play", "b-tension", "b-defl", "b-force", "b-dims"] {
        let a = app.clone();
        let d = doc.clone();
        on(&el(doc, id), "click", move |_: web_sys::Event| {
            let mut guard = a.borrow_mut();
            let a = &mut *guard;
            let Some(s) = a.scene.as_mut() else { return };
            let tg = &mut s.toggles;
            let v = match id {
                "b-play" => {
                    tg.play = !tg.play;
                    el(&d, "b-play").set_text_content(Some(if tg.play { "❚❚" } else { "▶" }));
                    tg.play
                }
                "b-tension" => { tg.tension = !tg.tension; tg.tension }
                "b-defl" => { tg.defl = !tg.defl; tg.defl }
                "b-force" => { tg.force = !tg.force; tg.force }
                _ => { tg.dims = !tg.dims; tg.dims }
            };
            if id != "b-play" {
                toggle_button(&d, id, v);
            }
            s.set_design(&a.inp, &a.out, false);
            a.dirty = true;
        });
    }
    {
        let a = app.clone();
        on(&el(doc, "speed"), "change", move |e: web_sys::Event| {
            let v = e.target().and_then(|t| t.dyn_into::<HtmlSelectElement>().ok()).map_or(0.25, |s| s.value().parse().unwrap_or(0.25));
            a.borrow_mut().speed = v;
        });
    }
    {
        let a = app.clone();
        on(&el(doc, "time"), "input", move |e: web_sys::Event| {
            let v = e.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()).map_or(0.0, |s| s.value().parse::<f32>().unwrap_or(0.0));
            let mut guard = a.borrow_mut();
            let a = &mut *guard;
            if let Some(s) = a.scene.as_mut() {
                // Slider spans one output revolution.
                s.theta = v / 1000.0 * std::f32::consts::TAU * s.total_ratio();
                s.set_design(&a.inp, &a.out, false);
            }
            a.dirty = true;
        });
    }
    {
        let a = app.clone();
        on(&el(doc, "preset"), "change", move |e: web_sys::Event| {
            let v = e.target().and_then(|t| t.dyn_into::<HtmlSelectElement>().ok()).map_or(0, |s| s.value().parse().unwrap_or(0));
            if let Some(s) = a.borrow_mut().scene.as_mut() {
                s.preset(v);
            }
        });
    }
    {
        let a = app.clone();
        on(&el(doc, "b-lang"), "click", move |_: web_sys::Event| {
            let mut a = a.borrow_mut();
            a.lang = if a.lang == Lang::Ko { Lang::En } else { Lang::Ko };
            if let Some(s) = storage() {
                let _ = s.set_item("bd_lang", if a.lang == Lang::Ko { "ko" } else { "en" });
            }
            a.apply_lang();
        });
    }
    {
        let d = doc.clone();
        let a = app.clone();
        on(&el(doc, "b-panel"), "click", move |_: web_sys::Event| {
            let _ = el(&d, "panel").class_list().toggle("hidden");
            a.borrow_mut().plots();
        });
    }
}

fn pos(e: &PointerEvent) -> (f32, f32) {
    (e.offset_x() as f32, e.offset_y() as f32)
}

fn wire_canvas(app: &Shared, canvas: &HtmlCanvasElement) {
    if app.borrow().scene.is_none() {
        return;
    }
    {
        let a = app.clone();
        let cv = canvas.clone();
        on(canvas, "pointerdown", move |e: PointerEvent| {
            let _ = cv.set_pointer_capture(e.pointer_id());
            let _ = cv.focus();
            let mut guard = a.borrow_mut();
            let a = &mut *guard;
            let p = pos(&e);
            a.pointers.insert(e.pointer_id(), p);
            a.last = p;
            if a.pointers.len() == 2 {
                let v: Vec<_> = a.pointers.values().copied().collect();
                let dist = ((v[0].0 - v[1].0).powi(2) + (v[0].1 - v[1].1).powi(2)).sqrt();
                a.pinch = Some((dist, ((v[0].0 + v[1].0) / 2.0, (v[0].1 + v[1].1) / 2.0)));
                a.drag = Drag::None;
                return;
            }
            let Some(s) = a.scene.as_mut() else { return };
            s.cam.held = true;
            let target = s.pick(p.0, p.1);
            a.drag = match (e.button(), target) {
                (2, _) => Drag::Pan,
                (_, Some(Target::Centre(i))) => Drag::Centre(i),
                (_, Some(Target::Rim)) => {
                    s.toggles.defl = true;
                    let ang = s.output_angle(p.0, p.1).unwrap_or(0.0);
                    let lam = s.lambda;
                    toggle_button(&a.doc, "b-defl", true);
                    Drag::Rim { start_angle: ang, start_lambda: lam }
                }
                _ => Drag::Orbit,
            };
        });
    }
    {
        let a = app.clone();
        let cv = canvas.clone();
        on(canvas, "pointermove", move |e: PointerEvent| {
            let mut guard = a.borrow_mut();
            let a = &mut *guard;
            let p = pos(&e);
            if a.pointers.contains_key(&e.pointer_id()) {
                a.pointers.insert(e.pointer_id(), p);
            }
            let Some(s) = a.scene.as_mut() else { return };
            if let (Some((d0, m0)), 2) = (a.pinch, a.pointers.len()) {
                let v: Vec<_> = a.pointers.values().copied().collect();
                let dist = ((v[0].0 - v[1].0).powi(2) + (v[0].1 - v[1].1).powi(2)).sqrt().max(1.0);
                let mid = ((v[0].0 + v[1].0) / 2.0, (v[0].1 + v[1].1) / 2.0);
                s.cam.zoom(d0 / dist);
                let (r, u) = s.cam_axes();
                let k = s.cam.goal.dist * 0.0015;
                s.cam.pan(math::add(math::scale(r, -(mid.0 - m0.0) * k), math::scale(u, (mid.1 - m0.1) * k)));
                a.pinch = Some((dist, mid));
                return;
            }
            let (dx, dy) = (p.0 - a.last.0, p.1 - a.last.1);
            a.last = p;
            match a.drag {
                Drag::Orbit => s.cam.rotate(-dx * 0.008, dy * 0.008),
                Drag::Pan => {
                    let (r, u) = s.cam_axes();
                    let k = s.cam.goal.dist * 0.0015;
                    s.cam.pan(math::add(math::scale(r, -dx * k), math::scale(u, dy * k)));
                }
                Drag::Centre(i) => {
                    if let Some(c) = s.drag_centre(i, p.0, p.1) {
                        // Snap: nearest whole-tooth belt for the requested C, then show its real C.
                        let mut inp = a.inp.clone();
                        inp.stages[i].c = c.max(1.0) as f64;
                        let o = evaluate(&inp);
                        if let Some(so) = o.stages.get(i).filter(|so| so.c > 0.0) {
                            a.inp.stages[i].c = (so.c * 10.0).round() / 10.0;
                            set_num(&a.doc, STAGE_IDS[i][2], a.inp.stages[i].c, 1);
                            a.recompute(false);
                        }
                    }
                }
                Drag::Rim { start_angle, start_lambda } => {
                    if let Some(ang) = s.output_angle(p.0, p.1) {
                        let mut d = ang - start_angle;
                        if d > std::f32::consts::PI {
                            d -= std::f32::consts::TAU;
                        } else if d < -std::f32::consts::PI {
                            d += std::f32::consts::TAU;
                        }
                        // Turning the output clockwise (against the drive) loads it; 0.21 rad shown = design load.
                        s.lambda = (start_lambda - d / 0.21).clamp(-1.5, 2.5);
                        s.set_design(&a.inp, &a.out, false);
                        a.hud();
                        a.plots();
                    }
                }
                Drag::None => {
                    let h = s.pick(p.0, p.1);
                    if h != s.hover {
                        s.hover = h;
                        a.dirty = true;
                    }
                    let cursor = match h {
                        Some(Target::Centre(_)) => "ew-resize",
                        Some(Target::Rim) => "grab",
                        None => "default",
                    };
                    let _ = cv.style().set_property("cursor", cursor);
                }
            }
            a.dirty = true;
        });
    }
    for ev in ["pointerup", "pointercancel"] {
        let a = app.clone();
        on(canvas, ev, move |e: PointerEvent| {
            let mut a = a.borrow_mut();
            a.pointers.remove(&e.pointer_id());
            if a.pointers.len() < 2 {
                a.pinch = None;
            }
            if a.pointers.is_empty() {
                a.drag = Drag::None;
                if let Some(s) = a.scene.as_mut() {
                    s.cam.held = false;
                }
            }
        });
    }
    {
        let a = app.clone();
        on(canvas, "wheel", move |e: WheelEvent| {
            e.prevent_default();
            if let Some(s) = a.borrow_mut().scene.as_mut() {
                s.cam.zoom((e.delta_y() as f32 * 0.0015).exp());
            }
        });
    }
    {
        let a = app.clone();
        on(canvas, "dblclick", move |_: web_sys::Event| {
            if let Some(s) = a.borrow_mut().scene.as_mut() {
                s.lambda = 1.0;
                s.reset_view();
            }
        });
    }
    on(canvas, "contextmenu", |e: web_sys::Event| e.prevent_default());
}

fn start_loop(app: Shared, canvas: HtmlCanvasElement) {
    let f: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();
    *g.borrow_mut() = Some(Closure::new(move |now: f64| {
        {
            let mut guard = app.borrow_mut();
            let a = &mut *guard;
            let dt = if a.last_t > 0.0 { ((now - a.last_t) / 1000.0).clamp(0.0, 0.1) as f32 } else { 0.0 };
            a.last_t = now;
            let dragging_rim = matches!(a.drag, Drag::Rim { .. });
            if let Some(s) = a.scene.as_mut() {
                // Released load springs back to the design load.
                if !dragging_rim && (s.lambda - 1.0).abs() > 1e-3 {
                    s.lambda += (1.0 - s.lambda) * (1.0 - (-dt * 6.0).exp());
                    if (s.lambda - 1.0).abs() <= 1e-3 {
                        s.lambda = 1.0;
                    }
                    a.dirty = true;
                }
                let moving = s.step(dt, a.speed);
                if s.toggles.play {
                    let rev = s.theta / (std::f32::consts::TAU * s.total_ratio());
                    if let Some(t) = input(&a.doc, "time") {
                        t.set_value(&format!("{:.0}", rev.rem_euclid(1.0) * 1000.0));
                    }
                }
                let dpr = web_sys::window().map_or(1.0, |w| w.device_pixel_ratio()).min(2.0);
                let (cw, ch) = (canvas.client_width(), canvas.client_height());
                let (w, h) = ((cw as f64 * dpr) as u32, (ch as f64 * dpr) as u32);
                let resized = canvas.width() != w || canvas.height() != h;
                if resized {
                    canvas.set_width(w);
                    canvas.set_height(h);
                }
                s.size = (cw as f32, ch as f32);
                if moving || resized || a.dirty {
                    if a.dirty {
                        s.set_design(&a.inp, &a.out, false);
                    }
                    s.render(w as i32, h as i32);
                    let lam_changed = a.dirty;
                    a.dirty = false;
                    a.labels();
                    if lam_changed {
                        a.hud();
                    }
                }
            }
        }
        if let (Some(w), Some(cb)) = (web_sys::window(), f.borrow().as_ref()) {
            let _ = w.request_animation_frame(cb.as_ref().unchecked_ref());
        }
    }));
    if let (Some(w), Some(cb)) = (web_sys::window(), g.borrow().as_ref()) {
        let _ = w.request_animation_frame(cb.as_ref().unchecked_ref());
    };
}

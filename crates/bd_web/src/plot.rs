//! Tiny canvas-2D line plots: up to two y axes, markers and vertical guide lines.
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d as Ctx, HtmlCanvasElement};

pub struct Series<'a> {
    pub pts: Vec<(f64, f64)>,
    pub color: &'a str,
    /// Plotted against the right-hand axis.
    pub right: bool,
}

pub struct Plot<'a> {
    pub series: Vec<Series<'a>>,
    /// Marked points (x, y, right axis, colour).
    pub marks: Vec<(f64, f64, bool, &'a str)>,
    /// Dashed vertical lines (x, colour).
    pub vlines: Vec<(f64, &'a str)>,
    pub xlabel: String,
    pub dark: bool,
}

fn range(vals: impl Iterator<Item = f64>) -> (f64, f64) {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for v in vals.filter(|v| v.is_finite()) {
        lo = lo.min(v);
        hi = hi.max(v);
    }
    if !lo.is_finite() {
        return (0.0, 1.0);
    }
    lo = lo.min(0.0);
    if hi - lo < 1e-9 {
        hi = lo + 1.0;
    }
    (lo, hi + (hi - lo) * 0.08)
}

fn fmt(v: f64) -> String {
    let a = v.abs();
    if a >= 1e4 {
        format!("{:.0}k", v / 1e3)
    } else if a >= 100.0 || a == 0.0 {
        format!("{v:.0}")
    } else if a >= 10.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    }
}

pub fn draw(canvas: &HtmlCanvasElement, p: &Plot) {
    let dpr = web_sys::window().map_or(1.0, |w| w.device_pixel_ratio());
    let (cw, ch) = (canvas.client_width() as f64, canvas.client_height() as f64);
    if cw < 10.0 || ch < 10.0 {
        return;
    }
    canvas.set_width((cw * dpr) as u32);
    canvas.set_height((ch * dpr) as u32);
    let Some(ctx) = canvas.get_context("2d").ok().flatten().and_then(|c| c.dyn_into::<Ctx>().ok()) else { return };
    let _ = ctx.scale(dpr, dpr);
    let (fg, grid) = if p.dark { ("#9aa3b0", "#2c333e") } else { ("#5b6573", "#d6dbe3") };
    ctx.clear_rect(0.0, 0.0, cw, ch);
    let (l, r, t, b) = (42.0, 42.0, 10.0, 30.0);
    let (w, h) = (cw - l - r, ch - t - b);
    let xs = range(p.series.iter().flat_map(|s| s.pts.iter().map(|q| q.0)));
    let xs = (xs.0, xs.1 / 1.08);
    let yl = range(p.series.iter().filter(|s| !s.right).flat_map(|s| s.pts.iter().map(|q| q.1)));
    let yr = range(p.series.iter().filter(|s| s.right).flat_map(|s| s.pts.iter().map(|q| q.1)));
    let px = |x: f64| l + (x - xs.0) / (xs.1 - xs.0) * w;
    let py = |y: f64, right: bool| {
        let (lo, hi) = if right { yr } else { yl };
        t + h - (y - lo) / (hi - lo) * h
    };
    ctx.set_font("11px system-ui, sans-serif");
    ctx.set_line_width(1.0);
    for k in 0..=4 {
        let f = k as f64 / 4.0;
        let y = t + h - f * h;
        ctx.set_stroke_style_str(grid);
        ctx.begin_path();
        ctx.move_to(l, y);
        ctx.line_to(l + w, y);
        ctx.stroke();
        ctx.set_fill_style_str(fg);
        ctx.set_text_align("right");
        let _ = ctx.fill_text(&fmt(yl.0 + f * (yl.1 - yl.0)), l - 4.0, y + 4.0);
        if p.series.iter().any(|s| s.right) {
            ctx.set_text_align("left");
            let _ = ctx.fill_text(&fmt(yr.0 + f * (yr.1 - yr.0)), l + w + 4.0, y + 4.0);
        }
        ctx.set_text_align("center");
        let xv = xs.0 + f * (xs.1 - xs.0);
        let _ = ctx.fill_text(&fmt(xv), px(xv), t + h + 13.0);
    }
    let _ = ctx.fill_text(&p.xlabel, l + w / 2.0, ch - 3.0);
    let dash = js_sys::Array::of2(&JsValue::from(4.0), &JsValue::from(3.0));
    for (x, c) in &p.vlines {
        if *x >= xs.0 && *x <= xs.1 {
            ctx.set_stroke_style_str(c);
            let _ = ctx.set_line_dash(&dash);
            ctx.begin_path();
            ctx.move_to(px(*x), t);
            ctx.line_to(px(*x), t + h);
            ctx.stroke();
            let _ = ctx.set_line_dash(&js_sys::Array::new());
        }
    }
    ctx.set_line_width(2.0);
    for s in &p.series {
        ctx.set_stroke_style_str(s.color);
        ctx.begin_path();
        for (i, (x, y)) in s.pts.iter().filter(|q| q.1.is_finite()).enumerate() {
            if i == 0 {
                ctx.move_to(px(*x), py(*y, s.right));
            } else {
                ctx.line_to(px(*x), py(*y, s.right));
            }
        }
        ctx.stroke();
    }
    for (x, y, right, c) in &p.marks {
        if !y.is_finite() {
            continue;
        }
        ctx.set_fill_style_str(c);
        ctx.set_stroke_style_str(if p.dark { "#fff" } else { "#000" });
        ctx.set_line_width(1.5);
        ctx.begin_path();
        let _ = ctx.arc(px(*x), py(*y, *right), 4.5, 0.0, std::f64::consts::TAU);
        ctx.fill();
        ctx.stroke();
    }
}

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    AudioContext, CanvasRenderingContext2d, Document, Element, HtmlCanvasElement, HtmlElement,
    KeyboardEvent, OscillatorType, PointerEvent, TouchEvent, Window,
};

const CSS: &str = r#"
:root{--bg:#090b12;--ink:#e8ecf5;--c:#00e5ff;--m:#ff2e88;--dim:#6b7488;--w:min(92vw,calc((100dvh - 240px)/2),340px)}
*{box-sizing:border-box}
[hidden]{display:none!important}
html,body{margin:0;height:100%;background:var(--bg);color:var(--ink);font:600 15px ui-monospace,Menlo,Consolas,monospace;overflow:hidden;touch-action:none;user-select:none;-webkit-user-select:none}
#bg{position:fixed;inset:0;z-index:0}
body::after{content:"";position:fixed;inset:0;pointer-events:none;z-index:9;background:repeating-linear-gradient(0deg,rgba(0,0,0,.25) 0 1px,transparent 1px 3px);animation:fl 7s infinite}
@keyframes fl{0%,92%,100%{opacity:1}94%{opacity:.55}96%{opacity:1}98%{opacity:.8}}
main{position:relative;z-index:1;height:100%;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:10px;padding:8px}
.top{width:var(--w);display:flex;justify-content:space-between;align-items:flex-end}
h1{margin:0 0 6px;font-size:28px;letter-spacing:-1px;line-height:1;position:relative;display:inline-block}
h1::before,h1::after{content:attr(data-t);position:absolute;inset:0;mix-blend-mode:screen}
h1::before{color:var(--c);clip-path:inset(0 0 55% 0);animation:gl 3.2s steps(1) infinite}
h1::after{color:var(--m);clip-path:inset(55% 0 0 0);animation:gl 2.6s steps(1) infinite reverse}
@keyframes gl{0%,88%,100%{transform:none}90%{transform:translate(-4px,1px)}93%{transform:translate(5px,-1px)}96%{transform:translate(-2px,0)}}
dl{margin:0;display:grid;grid-template-columns:auto auto;gap:0 12px;font-size:13px}
dt{color:var(--dim)} dd{margin:0;text-align:right;color:var(--c)}
#next{width:64px;height:64px;border:1px solid rgba(0,229,255,.35);background:rgba(9,11,18,.7)}
#stage{position:relative;width:var(--w);border:1px solid rgba(0,229,255,.45);box-shadow:0 0 40px rgba(0,229,255,.14),inset 0 0 30px rgba(255,46,136,.07)}
#game{display:block;width:100%;aspect-ratio:1/2;background:rgba(9,11,18,.8)}
.glitch{animation:rgb .38s steps(6)}
@keyframes rgb{0%{filter:drop-shadow(-7px 0 var(--c)) drop-shadow(7px 0 var(--m));transform:skewX(5deg)}45%{filter:drop-shadow(5px 0 var(--c)) drop-shadow(-5px 0 var(--m));transform:skewX(-3deg)}100%{filter:none;transform:none}}
#over{position:absolute;inset:0;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:12px;background:rgba(9,11,18,.86)}
#over h2{margin:0;font-size:24px;color:var(--m);text-shadow:-2px 0 var(--c),2px 0 var(--m);animation:gl 1.2s steps(1) infinite}
#over p{margin:0;color:var(--dim)}
#controls{display:flex;gap:8px;padding-bottom:env(safe-area-inset-bottom)}
.btn{width:58px;height:58px;background:rgba(232,236,245,.06);border:0;color:var(--ink);font:inherit;font-size:24px;clip-path:polygon(9px 0,100% 0,100% calc(100% - 9px),calc(100% - 9px) 100%,0 100%,0 9px);box-shadow:inset 0 0 0 1px var(--c);touch-action:manipulation;cursor:pointer}
.btn:active{background:var(--c);color:var(--bg)}
.btn.wide{width:auto;padding:0 28px;height:46px;font-size:16px}
@media (prefers-reduced-motion:reduce){body::after,h1::before,h1::after,#over h2{animation:none}}
"#;

const BODY_HTML: &str = r#"
<canvas id="bg"></canvas>
<main>
  <div class="top">
    <div>
      <h1 data-t="TETRIS">TETRIS</h1>
      <dl>
        <dt>Счёт</dt><dd id="score">0</dd>
        <dt>Уровень</dt><dd id="level">1</dd>
        <dt>Линии</dt><dd id="lines">0</dd>
        <dt>Рекорд</dt><dd id="best">0</dd>
      </dl>
    </div>
    <canvas id="next" width="80" height="80"></canvas>
  </div>
  <div id="stage">
    <canvas id="game" width="300" height="600"></canvas>
    <div id="over" hidden>
      <h2>Стек переполнен</h2>
      <p id="final"></p>
      <button class="btn wide" data-a="reset">Ещё раз</button>
    </div>
  </div>
  <div id="controls">
    <button class="btn" data-a="left" data-r="1">←</button>
    <button class="btn" data-a="turn">↻</button>
    <button class="btn" data-a="soft" data-r="1">↓</button>
    <button class="btn" data-a="hard">⤓</button>
    <button class="btn" data-a="right" data-r="1">→</button>
  </div>
</main>
"#;

fn mount_ui(doc: &Document) -> Result<(), JsValue> {
    if let Some(head) = doc.head() {
        let style = doc.create_element("style")?;
        style.set_text_content(Some(CSS));
        head.append_child(&style)?;
    }
    if let Some(body) = doc.body() {
        body.set_inner_html(BODY_HTML);
    }
    Ok(())
}

const W: usize = 10;
const H: usize = 20;
const S: f64 = 30.0;
const COL: [&str; 7] = ["#00e5ff", "#ffd400", "#ff2e88", "#3d6bff", "#ff7a1a", "#35ff8c", "#ff3b3b"];

fn shapes(idx: usize) -> Vec<Vec<u8>> {
    let raw: &[&[&[u8]]] = &[
        &[&[1, 1, 1, 1]],
        &[&[1, 1], &[1, 1]],
        &[&[0, 1, 0], &[1, 1, 1]],
        &[&[1, 0, 0], &[1, 1, 1]],
        &[&[0, 0, 1], &[1, 1, 1]],
        &[&[0, 1, 1], &[1, 1, 0]],
        &[&[1, 1, 0], &[0, 1, 1]],
    ];
    raw[idx % 7].iter().map(|r| r.to_vec()).collect()
}

fn rand_f64() -> f64 { js_sys::Math::random() }

struct Piece { i: usize, s: Vec<Vec<u8>>, x: isize, y: isize }
struct Particle { x: f64, y: f64, vx: f64, vy: f64, l: f64, c: &'static str }

struct Game {
    win: Window, doc: Document, cv: HtmlCanvasElement, g: CanvasRenderingContext2d,
    n2: CanvasRenderingContext2d, bg: HtmlCanvasElement, b: CanvasRenderingContext2d,
    ac: Option<AudioContext>, board: [[u8; W]; H], p: Piece, nxt: usize, bag: Vec<usize>,
    score: u32, level: u32, lines: u32, over: bool, drop: f64, acc: f64,
    parts: Vec<Particle>, shake: f64, flash: Vec<usize>, flash_t: f64, best: u32,
    rain: Vec<f64>, fr: u32, last_t: f64, touch_start: Option<(f64, f64)>,
}
impl Game {
    fn new(win: Window, doc: Document) -> Result<Self, JsValue> {
        let cv: HtmlCanvasElement = doc.get_element_by_id("game").unwrap().dyn_into()?;
        let g: CanvasRenderingContext2d = cv.get_context("2d")?.unwrap().dyn_into()?;
        let nv: HtmlCanvasElement = doc.get_element_by_id("next").unwrap().dyn_into()?;
        let n2: CanvasRenderingContext2d = nv.get_context("2d")?.unwrap().dyn_into()?;
        let bg: HtmlCanvasElement = doc.get_element_by_id("bg").unwrap().dyn_into()?;
        let b: CanvasRenderingContext2d = bg.get_context("2d")?.unwrap().dyn_into()?;
        let best = win.local_storage().ok().flatten()
            .and_then(|s| s.get_item("fsb").ok().flatten())
            .and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);

        let mut game = Self {
            win, doc, cv, g, n2, bg, b, ac: None, board: [[0; W]; H],
            p: Piece { i: 0, s: shapes(0), x: 3, y: 0 }, nxt: 0, bag: Vec::new(),
            score: 0, level: 1, lines: 0, over: false, drop: 700.0, acc: 0.0,
            parts: Vec::new(), shake: 0.0, flash: Vec::new(), flash_t: 0.0, best,
            rain: Vec::new(), fr: 0, last_t: 0.0, touch_start: None,
        };
        game.resize_bg();
        game.reset();
        Ok(game)
    }

    fn el(&self, id: &str) -> HtmlElement { self.doc.get_element_by_id(id).unwrap().dyn_into().unwrap() }

    fn tone(&mut self, f: f64, d: f64, osc_type: OscillatorType, v: f32, end_f: f64) {
        if self.ac.is_none() { self.ac = AudioContext::new().ok(); }
        if let Some(ac) = &self.ac {
            let now = ac.current_time();
            if let (Ok(o), Ok(a)) = (ac.create_oscillator(), ac.create_gain()) {
                o.set_type(osc_type);
                let _ = o.frequency().set_value_at_time(f as f32, now);
                let _ = o.frequency().exponential_ramp_to_value_at_time(end_f as f32, now + d);
                let _ = a.gain().set_value_at_time(v, now);
                let _ = a.gain().exponential_ramp_to_value_at_time(0.001, now + d);
                let _ = o.connect_with_audio_node(&a);
                let _ = a.connect_with_audio_node(&ac.destination());
                let _ = o.start();
                let _ = o.stop_with_when(now + d);
            }
        }
    }

    fn vib(&self, ms: u32) { let _ = self.win.navigator().vibrate_with_duration(ms); }

    fn take(&mut self) -> usize {
        if self.bag.is_empty() {
            self.bag = (0..7).collect();
            for i in (1..=6).rev() {
                let j = (rand_f64() * ((i + 1) as f64)) as usize;
                self.bag.swap(i, j);
            }
        }
        self.bag.pop().unwrap()
    }

    fn hit(&self, dx: isize, dy: isize, s: &[Vec<u8>]) -> bool {
        for (r, row) in s.iter().enumerate() {
            for (c, &val) in row.iter().enumerate() {
                if val == 0 { continue; }
                let x = self.p.x + (c as isize) + dx;
                let y = self.p.y + (r as isize) + dy;
                if x < 0 || x >= (W as isize) || y >= (H as isize) { return true; }
                if y >= 0 && self.board[y as usize][x as usize] != 0 { return true; }
            }
        }
        false
    }

    fn spawn(&mut self) {
        let s = shapes(self.nxt);
        let x = ((W - s[0].len()) >> 1) as isize;
        self.p = Piece { i: self.nxt, s, x, y: 0 };
        self.nxt = self.take();
        self.draw_next();
        if self.hit(0, 0, &self.p.s) { self.end(); }
    }

    fn burst(&mut self, x: f64, y: f64, c: &'static str, n: usize, v: f64) {
        for _ in 0..n {
            let a = rand_f64() * std::f64::consts::TAU;
            let spd = rand_f64() * v + 1.0;
            self.parts.push(Particle { x, y, vx: a.cos() * spd, vy: a.sin() * spd - 1.0, l: 1.0, c });
        }
    }

    fn glitch(&self) {
        let cl = self.cv.class_list();
        let _ = cl.remove_1("glitch");
        let _ = self.cv.offset_width();
        let _ = cl.add_1("glitch");
    }

    fn lock(&mut self) {
        let (p_i, p_x, p_y, s_clone) = (self.p.i, self.p.x, self.p.y, self.p.s.clone());
        for (r, row) in s_clone.iter().enumerate() {
            for (c, &v) in row.iter().enumerate() {
                if v == 0 { continue; }
                let (y, x) = (p_y + r as isize, p_x + c as isize);
                if y >= 0 && y < H as isize && x >= 0 && x < W as isize {
                    self.board[y as usize][x as usize] = (p_i + 1) as u8;
                }
                self.burst(x as f64 * S + S / 2.0, y as f64 * S + S / 2.0, COL[p_i], 2, 2.5);
            }
        }
        self.tone(130.0, 0.12, OscillatorType::Sawtooth, 0.06, 50.0);

        let mut cl = Vec::new();
        for y in 0..H { if self.board[y].iter().all(|&v| v != 0) { cl.push(y); } }

        if !cl.is_empty() {
            for &y in &cl {
                for x in 0..W {
                    let col_idx = (self.board[y][x] - 1) as usize;
                    self.burst(x as f64 * S + S / 2.0, y as f64 * S + S / 2.0, COL[col_idx], 3, 6.0);
                }
            }
            let mut new_board = [[0u8; W]; H];
            let mut dst_y = H;
            for y in (0..H).rev() {
                if !cl.contains(&y) { dst_y -= 1; new_board[dst_y] = self.board[y]; }
            }
            self.board = new_board;

            let cnt = cl.len() as u32;
            self.lines += cnt;
            let pts = match cnt { 1 => 100, 2 => 300, 3 => 500, _ => 800 };
            self.score += pts * self.level;
            self.level = 1 + (self.lines / 10);
            self.drop = (700.0 - ((self.level - 1) as f64) * 55.0).max(90.0);
            self.flash = cl;
            self.flash_t = 1.0;
            self.shake = if cnt == 4 { 24.0 } else { 5.0 + cnt as f64 * 3.0 };
            self.glitch();
            self.vib(cnt * 25);
            self.tone(300.0, 0.35, OscillatorType::Square, 0.06, 1400.0);
            if cnt == 4 { self.tone(500.0, 0.4, OscillatorType::Sawtooth, 0.06, 2000.0); }
            self.hud();
        }
        self.spawn();
    }

    fn mv(&mut self, dx: isize) {
        if !self.over && !self.hit(dx, 0, &self.p.s) {
            self.p.x += dx;
            self.tone(220.0, 0.03, OscillatorType::Square, 0.03, 220.0);
        }
    }

    fn soft(&mut self) {
        if self.over { return; }
        if !self.hit(0, 1, &self.p.s) { self.p.y += 1; self.score += 1; self.hud(); }
        else { self.lock(); }
        self.acc = 0.0;
    }

    fn hard(&mut self) {
        if self.over { return; }
        let mut n = 0;
        while !self.hit(0, 1, &self.p.s) { self.p.y += 1; n += 1; }
        self.score += n * 2;
        self.shake = 7.0;
        self.tone(200.0, 0.15, OscillatorType::Sawtooth, 0.07, 40.0);
        self.vib(12);
        let last_row = self.p.s[self.p.s.len() - 1].clone();
        let (p_x, p_y, p_i, s_len) = (self.p.x, self.p.y, self.p.i, self.p.s.len());
        for (c, &v) in last_row.iter().enumerate() {
            if v != 0 { self.burst((p_x + c as isize) as f64 * S + S / 2.0, (p_y + s_len as isize) as f64 * S, COL[p_i], 5, 4.0); }
        }
        self.lock(); self.hud(); self.acc = 0.0;
    }

    fn turn(&mut self) {
        if self.over { return; }
        let (h, w) = (self.p.s.len(), self.p.s[0].len());
        let mut rot = vec![vec![0u8; h]; w];
        for r in 0..h { for c in 0..w { rot[c][h - 1 - r] = self.p.s[r][c]; } }
        for k in [0, -1, 1, -2, 2] {
            if !self.hit(k, 0, &rot) {
                self.p.s = rot; self.p.x += k;
                self.tone(440.0, 0.08, OscillatorType::Square, 0.04, 700.0);
                return;
            }
        }
    }

    fn end(&mut self) {
        self.over = true; self.shake = 26.0; self.glitch(); self.vib(200);
        self.tone(400.0, 0.9, OscillatorType::Sawtooth, 0.07, 40.0);
        if self.score > self.best {
            self.best = self.score;
            if let Ok(Some(st)) = self.win.local_storage() { let _ = st.set_item("fsb", &self.best.to_string()); }
        }
        self.el("final").set_text_content(Some(&format!("Счёт {}, рекорд {}", self.score, self.best)));
        self.el("over").set_hidden(false);
        self.hud();
    }

    fn reset(&mut self) {
        self.board = [[0; W]; H]; self.score = 0; self.level = 1; self.lines = 0;
        self.drop = 700.0; self.acc = 0.0; self.over = false; self.parts.clear(); self.flash.clear();
        self.el("over").set_hidden(true); self.nxt = self.take(); self.spawn(); self.hud();
    }

    fn hud(&self) {
        self.el("score").set_text_content(Some(&self.score.to_string()));
        self.el("level").set_text_content(Some(&self.level.to_string()));
        self.el("lines").set_text_content(Some(&self.lines.to_string()));
        self.el("best").set_text_content(Some(&self.best.max(self.score).to_string()));
    }
}
impl Game {
    fn draw_cell(ctx: &CanvasRenderingContext2d, c: &str, x: f64, y: f64, a: f64, s: f64) {
        ctx.set_global_alpha(a); ctx.set_shadow_color(c); ctx.set_shadow_blur(s / 2.5);
        ctx.set_fill_style_str(c); ctx.fill_rect(x + 1.0, y + 1.0, s - 2.0, s - 2.0);
        ctx.set_shadow_blur(0.0); ctx.set_fill_style_str("rgba(255,255,255,.4)");
        ctx.fill_rect(x + 1.0, y + 1.0, s - 2.0, s / 8.0);
        ctx.set_fill_style_str("rgba(0,0,0,.25)");
        ctx.fill_rect(x + 1.0, y + s - 1.0 - s / 8.0, s - 2.0, s / 8.0);
        ctx.set_global_alpha(1.0);
    }

    fn draw_next(&self) {
        self.n2.clear_rect(0.0, 0.0, 80.0, 80.0);
        let s = shapes(self.nxt);
        let k = 18.0;
        let ox = (80.0 - (s[0].len() as f64) * k) / 2.0;
        let oy = (80.0 - (s.len() as f64) * k) / 2.0;
        for (r, row) in s.iter().enumerate() {
            for (c, &v) in row.iter().enumerate() {
                if v != 0 { Self::draw_cell(&self.n2, COL[self.nxt], ox + c as f64 * k, oy + r as f64 * k, 1.0, k); }
            }
        }
    }

    fn render(&self) {
        let _ = self.g.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        self.g.clear_rect(0.0, 0.0, 300.0, 600.0);
        if self.shake > 0.3 { let _ = self.g.translate((rand_f64() - 0.5) * self.shake, (rand_f64() - 0.5) * self.shake); }
        self.g.set_stroke_style_str("rgba(255,255,255,.045)"); self.g.set_line_width(1.0); self.g.begin_path();
        for i in 1..W { self.g.move_to(i as f64 * S, 0.0); self.g.line_to(i as f64 * S, 600.0); }
        for i in 1..H { self.g.move_to(0.0, i as f64 * S); self.g.line_to(300.0, i as f64 * S); }
        self.g.stroke();

        for (y, row) in self.board.iter().enumerate() {
            for (x, &v) in row.iter().enumerate() {
                if v != 0 { Self::draw_cell(&self.g, COL[(v - 1) as usize], x as f64 * S, y as f64 * S, 1.0, S); }
            }
        }

        if !self.over {
            let mut gy = 0;
            while !self.hit(0, gy + 1, &self.p.s) { gy += 1; }
            self.g.set_stroke_style_str(COL[self.p.i]); self.g.set_global_alpha(0.45);
            for (r, row) in self.p.s.iter().enumerate() {
                for (c, &v) in row.iter().enumerate() {
                    if v != 0 { self.g.stroke_rect((self.p.x + c as isize) as f64 * S + 2.0, (self.p.y + r as isize + gy) as f64 * S + 2.0, S - 4.0, S - 4.0); }
                }
            }
            self.g.set_global_alpha(1.0);
            for (r, row) in self.p.s.iter().enumerate() {
                for (c, &v) in row.iter().enumerate() {
                    if v != 0 { Self::draw_cell(&self.g, COL[self.p.i], (self.p.x + c as isize) as f64 * S, (self.p.y + r as isize) as f64 * S, 1.0, S); }
                }
            }
        }

        for q in &self.parts { self.g.set_global_alpha(q.l.max(0.0)); self.g.set_fill_style_str(q.c); self.g.fill_rect(q.x, q.y, 3.0, 3.0); }
        self.g.set_global_alpha(1.0);
        if self.flash_t > 0.0 {
            self.g.set_fill_style_str(&format!("rgba(255,255,255,{})", self.flash_t * 0.85));
            for &y in &self.flash { self.g.fill_rect(0.0, y as f64 * S, 300.0, S); }
        }
    }

    fn resize_bg(&mut self) {
        let w = self.win.inner_width().unwrap().as_f64().unwrap_or(800.0) as u32;
        let h = self.win.inner_height().unwrap().as_f64().unwrap_or(600.0) as u32;
        self.bg.set_width(w); self.bg.set_height(h);
        let cols = (w as f64 / 16.0).ceil() as usize;
        self.rain = (0..cols).map(|_| rand_f64() * (h as f64) / 16.0).collect();
    }

    fn draw_rain(&mut self) {
        let (w, h) = (self.bg.width() as f64, self.bg.height() as f64);
        self.b.set_fill_style_str("rgba(9,11,18,.14)"); self.b.fill_rect(0.0, 0.0, w, h);
        self.b.set_fill_style_str("rgba(0,229,255,.32)"); self.b.set_font("14px monospace");
        const HEX: &[u8] = b"0123456789abcdef";
        for (i, d) in self.rain.iter_mut().enumerate() {
            let ch = HEX[(rand_f64() * 16.0) as usize] as char;
            let _ = self.b.fill_text(&ch.to_string(), (i * 16) as f64, *d * 16.0);
            *d = if *d * 16.0 > h && rand_f64() > 0.975 { 0.0 } else { *d + 0.7 };
        }
    }

    fn tick(&mut self, t: f64) {
        let dt = (t - self.last_t).min(100.0); self.last_t = t;
        if !self.over { self.acc += dt; if self.acc >= self.drop { self.acc = 0.0; self.soft(); } }
        for q in &mut self.parts { q.x += q.vx; q.y += q.vy; q.vy += 0.15; q.l -= dt / 700.0; }
        self.parts.retain(|q| q.l > 0.0); self.shake *= 0.88; self.flash_t -= dt / 350.0;
        self.fr = self.fr.wrapping_add(1);
        let reduced = self.win.match_media("(prefers-reduced-motion:reduce)").ok().flatten().map(|m| m.matches()).unwrap_or(false);
        if self.fr % 3 == 0 && !reduced { self.draw_rain(); }
        self.render();
    }

    fn action(&mut self, name: &str) {
        match name { "left" => self.mv(-1), "right" => self.mv(1), "soft" => self.soft(), "hard" => self.hard(), "turn" => self.turn(), "reset" => self.reset(), _ => {} }
    }
}

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    let win = web_sys::window().unwrap();
    let doc = win.document().unwrap();
    mount_ui(&doc)?;
    let game = Rc::new(RefCell::new(Game::new(win.clone(), doc.clone())?));

    {
        let g = game.clone();
        let cb = Closure::<dyn FnMut()>::new(move || g.borrow_mut().resize_bg());
        win.add_event_listener_with_callback("resize", cb.as_ref().unchecked_ref())?;
        cb.forget();
    }
    {
        let g = game.clone();
        let cb = Closure::<dyn FnMut(KeyboardEvent)>::new(move |e: KeyboardEvent| {
            let act = match e.key().as_str() { "ArrowLeft" => Some("left"), "ArrowRight" => Some("right"), "ArrowDown" => Some("soft"), "ArrowUp" => Some("turn"), " " => Some("hard"), "r" | "R" => Some("reset"), _ => None };
            if let Some(a) = act { e.prevent_default(); g.borrow_mut().action(a); }
        });
        win.add_event_listener_with_callback("keydown", cb.as_ref().unchecked_ref())?;
        cb.forget();
    }

    let nodes = doc.query_selector_all("[data-a]")?;
    for i in 0..nodes.length() {
        if let Some(node) = nodes.item(i) {
            let el: Element = node.dyn_into()?;
            let act = el.get_attribute("data-a").unwrap_or_default();
            let repeat = el.has_attribute("data-r");
            let timer_id = Rc::new(RefCell::new(None::<i32>));
            {
                let (g, act_c, tid, win_c) = (game.clone(), act.clone(), timer_id.clone(), win.clone());
                let down = Closure::<dyn FnMut(PointerEvent)>::new(move |e: PointerEvent| {
                    e.prevent_default(); g.borrow_mut().action(&act_c);
                    if repeat && tid.borrow().is_none() {
                        let (g2, act2) = (g.clone(), act_c.clone());
                        let iv = Closure::<dyn FnMut()>::new(move || g2.borrow_mut().action(&act2));
                        if let Ok(id) = win_c.set_interval_with_callback_and_timeout_and_arguments_0(iv.as_ref().unchecked_ref(), 95) { *tid.borrow_mut() = Some(id); }
                        iv.forget();
                    }
                });
                el.add_event_listener_with_callback("pointerdown", down.as_ref().unchecked_ref())?;
                down.forget();
            }
            for ev in ["pointerup", "pointerleave", "pointercancel"] {
                let (tid, win_c) = (timer_id.clone(), win.clone());
                let up = Closure::<dyn FnMut()>::new(move || { if let Some(id) = tid.borrow_mut().take() { win_c.clear_interval_with_handle(id); } });
                el.add_event_listener_with_callback(ev, up.as_ref().unchecked_ref())?;
                up.forget();
            }
        }
    }

    let cv = doc.get_element_by_id("game").unwrap();
    {
        let g = game.clone();
        let ts = Closure::<dyn FnMut(TouchEvent)>::new(move |e: TouchEvent| { e.prevent_default(); if let Some(t) = e.touches().item(0) { g.borrow_mut().touch_start = Some((t.client_x() as f64, t.client_y() as f64)); } });
        cv.add_event_listener_with_callback("touchstart", ts.as_ref().unchecked_ref())?;
        ts.forget();
    }
    {
        let g = game.clone();
        let te = Closure::<dyn FnMut(TouchEvent)>::new(move |e: TouchEvent| {
            e.prevent_default(); let mut gm = g.borrow_mut();
            if let (Some((sx, sy)), Some(t)) = (gm.touch_start.take(), e.changed_touches().item(0)) {
                let (dx, dy) = (t.client_x() as f64 - sx, t.client_y() as f64 - sy);
                if dx.abs() < 18.0 && dy.abs() < 18.0 { gm.turn(); }
                else if dx.abs() > dy.abs() { gm.mv(if dx > 0.0 { 1 } else { -1 }); }
                else if dy > 45.0 { gm.hard(); }
                else if dy > 0.0 { gm.soft(); }
            }
        });
        cv.add_event_listener_with_callback("touchend", te.as_ref().unchecked_ref())?;
        te.forget();
    }

    let f: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let (g_loop, win_loop) = (f.clone(), win.clone());
    *g_loop.borrow_mut() = Some(Closure::new(move |t: f64| {
        game.borrow_mut().tick(t);
        let _ = win_loop.request_animation_frame(f.borrow().as_ref().unwrap().as_ref().unchecked_ref());
    }));
    win.request_animation_frame(g_loop.borrow().as_ref().unwrap().as_ref().unchecked_ref())?;
    Ok(())
}

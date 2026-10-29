//! Les contrôles de [potard](https://github.com/yrbane/potard), portés en
//! egui : knob, slider à flèche, fader, switch, stepper, bouton, LED et
//! VU-mètre, avec les gestes de la bibliothèque (drag vertical, Shift =
//! précision ×10, double-clic = valeur par défaut, molette et clavier par
//! pas) et ses attributs (`min`, `max`, `default`, `step`, `label`, `unit`,
//! `sensitivity`, `curve`, `disabled`, `color`). La logique pure est dans
//! [`logic`], testée à part ; le reste ne fait que dessiner.
use nih_plug_egui::egui::{self, Color32, Pos2, Rect, Response, Sense, Stroke, Vec2};

/// Teinte d'accent par défaut de potard (`--ctl-accent`).
pub const ACCENT: Color32 = Color32::from_rgb(0x19, 0xc2, 0xff);
/// Piste par défaut (`--ctl-track`).
pub const TRACK: Color32 = Color32::from_rgb(0x3a, 0x40, 0x48);
/// Encre sur accent (texte des boutons actifs, pouce du switch allumé).
pub const ON_ACCENT: Color32 = Color32::from_rgb(0x10, 0x13, 0x18);
/// Pouce du switch éteint.
pub const THUMB_OFF: Color32 = Color32::from_rgb(0xd7, 0xdc, 0xe2);
/// Segments du VU-mètre.
pub const VU_GREEN: Color32 = Color32::from_rgb(0x33, 0xdd, 0x66);
pub const VU_YELLOW: Color32 = Color32::from_rgb(0xff, 0xcc, 0x33);
pub const VU_RED: Color32 = Color32::from_rgb(0xff, 0x44, 0x44);
pub const VU_OFF: Color32 = Color32::from_rgb(0x2a, 0x2f, 0x36);

/// Logique des contrôles continus, sans dessin ni egui.
pub mod logic {
    /// Pixels de drag pour parcourir toute la course.
    pub const DEFAULT_SENSITIVITY_PX: f32 = 200.0;
    /// Facteur de précision quand Shift est enfoncé.
    pub const SHIFT_PRECISION: f32 = 0.1;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum Curve {
        #[default]
        Lin,
        /// Taper audio : valeur = position².
        Log,
    }

    /// Position 0..1 sur la course d'une valeur, courbe appliquée.
    pub fn fraction(value: f32, min: f32, max: f32, curve: Curve) -> f32 {
        let range = max - min;
        if range == 0.0 {
            return 0.0;
        }
        let linear = ((value - min) / range).clamp(0.0, 1.0);
        match curve {
            Curve::Lin => linear,
            Curve::Log => linear.sqrt(),
        }
    }

    /// Valeur d'une position 0..1 sur la course.
    pub fn value_of(fraction: f32, min: f32, max: f32, curve: Curve) -> f32 {
        let f = fraction.clamp(0.0, 1.0);
        let linear = match curve {
            Curve::Lin => f,
            Curve::Log => f * f,
        };
        min + linear * (max - min)
    }

    /// Nouvelle position après un drag de `delta_px` (positif = vers le
    /// haut ou la droite), Shift divisant la course par 10.
    pub fn drag(start_fraction: f32, delta_px: f32, sensitivity: f32, shift: bool) -> f32 {
        let precision = if shift { SHIFT_PRECISION } else { 1.0 };
        let s = if sensitivity > 0.0 {
            sensitivity
        } else {
            DEFAULT_SENSITIVITY_PX
        };
        (start_fraction + delta_px / s * precision).clamp(0.0, 1.0)
    }

    /// Pas par défaut : 1 % de la course.
    pub fn default_step(min: f32, max: f32) -> f32 {
        (max - min).abs() / 100.0
    }

    /// Position 0..1 du zéro sur la course : départ du remplissage bipolaire.
    pub fn zero_fraction(min: f32, max: f32) -> f32 {
        let range = max - min;
        if range == 0.0 || min >= 0.0 {
            0.0
        } else if max <= 0.0 {
            1.0
        } else {
            -min / range
        }
    }

    pub fn bipolar(min: f32, max: f32) -> bool {
        min < 0.0 && max > 0.0
    }

    /// Valeur après `n` pas (positif ou négatif), bornée.
    pub fn stepped(value: f32, n: i32, step: f32, min: f32, max: f32) -> f32 {
        (value + n as f32 * step).clamp(min.min(max), max.max(min))
    }

    /// Index d'un stepper, bouclé sur `count` options.
    pub fn wrap(index: i64, count: usize) -> usize {
        if count == 0 {
            return 0;
        }
        let c = count as i64;
        (((index % c) + c) % c) as usize
    }

    /// Segments allumés d'un VU-mètre pour un niveau 0..1.
    pub fn lit_segments(level: f32, count: usize) -> usize {
        (level.clamp(0.0, 1.0) * count as f32).round() as usize
    }

    /// Zone du segment `i` (de 0) sur `count` : 0 vert, 1 jaune, 2 rouge.
    pub fn segment_zone(i: usize, count: usize) -> u8 {
        let frac = (i + 1) as f32 / count.max(1) as f32;
        if frac > 0.85 {
            2
        } else if frac > 0.65 {
            1
        } else {
            0
        }
    }
}

use logic::Curve;

/// Teintes partagées (`--ctl-accent`, `--ctl-track`) ; un contrôle peut
/// surcharger l'accent avec `color`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub accent: Color32,
    pub track: Color32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            accent: ACCENT,
            track: TRACK,
        }
    }
}

/// Un contrôle continu : la valeur et ses attributs potard.
pub struct Control<'a> {
    pub value: &'a mut f32,
    pub min: f32,
    pub max: f32,
    /// Valeur de retour au double-clic ; sans elle, le milieu de la plage.
    pub default: Option<f32>,
    /// Pas clavier / molette ; sans lui, 1 % de la course.
    pub step: Option<f32>,
    pub label: &'a str,
    pub unit: &'a str,
    pub sensitivity: f32,
    pub curve: Curve,
    pub disabled: bool,
    pub color: Option<Color32>,
    pub decimals: usize,
    /// Une ligne d'aide sous la valeur, dans l'infobulle.
    pub hint: &'a str,
}

impl<'a> Control<'a> {
    pub fn new(value: &'a mut f32, min: f32, max: f32) -> Self {
        Self {
            value,
            min,
            max,
            default: None,
            step: None,
            label: "",
            unit: "",
            sensitivity: logic::DEFAULT_SENSITIVITY_PX,
            curve: Curve::Lin,
            disabled: false,
            color: None,
            decimals: 2,
            hint: "",
        }
    }
    pub fn default_value(mut self, v: f32) -> Self {
        self.default = Some(v);
        self
    }
    pub fn step(mut self, s: f32) -> Self {
        self.step = Some(s);
        self
    }
    pub fn label(mut self, l: &'a str) -> Self {
        self.label = l;
        self
    }
    pub fn unit(mut self, u: &'a str) -> Self {
        self.unit = u;
        self
    }
    pub fn sensitivity(mut self, px: f32) -> Self {
        self.sensitivity = px;
        self
    }
    pub fn curve(mut self, c: Curve) -> Self {
        self.curve = c;
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn color(mut self, c: Color32) -> Self {
        self.color = Some(c);
        self
    }
    pub fn decimals(mut self, n: usize) -> Self {
        self.decimals = n;
        self
    }
    pub fn hint(mut self, h: &'a str) -> Self {
        self.hint = h;
        self
    }
    fn tooltip(&self) -> String {
        if self.hint.is_empty() {
            self.value_text()
        } else {
            format!("{}\n{}", self.value_text(), self.hint)
        }
    }

    fn fraction(&self) -> f32 {
        logic::fraction(*self.value, self.min, self.max, self.curve)
    }
    fn step_value(&self) -> f32 {
        self.step
            .unwrap_or_else(|| logic::default_step(self.min, self.max))
    }
    fn default_or_mid(&self) -> f32 {
        self.default.unwrap_or((self.min + self.max) / 2.0)
    }
    fn accent(&self, theme: &Theme) -> Color32 {
        self.color.unwrap_or(theme.accent)
    }
    /// Texte de la valeur, avec l'unité (« 6 dB »).
    pub fn value_text(&self) -> String {
        let v = format!("{:.*}", self.decimals, *self.value);
        if self.unit.is_empty() {
            v
        } else {
            format!("{v} {}", self.unit)
        }
    }
}

/// Ce qu'un geste a fait : `changed` pendant le geste (`input`), `released`
/// à sa fin (`change`), comme les événements de potard.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Gesture {
    pub changed: bool,
    pub released: bool,
    pub started: bool,
}

/// Interactions communes des contrôles continus (drag sur un axe, Shift,
/// double-clic, molette, clavier) sur une réponse déjà allouée.
fn interact(ui: &mut egui::Ui, response: &Response, c: &mut Control, vertical: bool) -> Gesture {
    let mut g = Gesture::default();
    if c.disabled {
        return g;
    }
    let id = response.id;
    if response.drag_started() {
        g.started = true;
        ui.data_mut(|d| d.insert_temp(id, c.fraction()));
    }
    if response.dragged() {
        let total: Vec2 = ui
            .input(|i| i.pointer.press_origin().zip(i.pointer.latest_pos()))
            .map(|(a, b)| b - a)
            .unwrap_or(Vec2::ZERO);
        let delta = if vertical { -total.y } else { total.x };
        let shift = ui.input(|i| i.modifiers.shift);
        let start = ui
            .data(|d| d.get_temp::<f32>(id))
            .unwrap_or_else(|| c.fraction());
        let f = logic::drag(start, delta, c.sensitivity, shift);
        let v = logic::value_of(f, c.min, c.max, c.curve);
        if v != *c.value {
            *c.value = v;
            g.changed = true;
        }
    }
    if response.drag_stopped() {
        g.released = true;
    }
    if response.double_clicked() {
        let d = c.default_or_mid().clamp(c.min.min(c.max), c.max.max(c.min));
        if d != *c.value {
            *c.value = d;
            g.changed = true;
        }
        g.released = true;
    }
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll.abs() > 0.5 {
            let n = if scroll > 0.0 { 1 } else { -1 };
            let v = logic::stepped(*c.value, n, c.step_value(), c.min, c.max);
            if v != *c.value {
                *c.value = v;
                g.changed = true;
                g.released = true;
            }
        }
    }
    if response.clicked() {
        response.request_focus();
    }
    if response.has_focus() {
        let (n, home, end) = ui.input(|i| {
            let up = i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::ArrowRight);
            let down = i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::ArrowLeft);
            (
                up as i32 - down as i32,
                i.key_pressed(egui::Key::Home),
                i.key_pressed(egui::Key::End),
            )
        });
        let mut v = *c.value;
        if n != 0 {
            v = logic::stepped(v, n, c.step_value(), c.min, c.max);
        }
        if home {
            v = c.min;
        }
        if end {
            v = c.max;
        }
        if v != *c.value {
            *c.value = v;
            g.changed = true;
            g.released = true;
        }
    }
    g
}

fn label_below(ui: &mut egui::Ui, rect: Rect, label: &str, ink: Color32) {
    if label.is_empty() {
        return;
    }
    ui.painter().text(
        Pos2::new(rect.center().x, rect.bottom() + 1.0),
        egui::Align2::CENTER_TOP,
        label.to_uppercase(),
        egui::FontId::proportional(9.0),
        ink,
    );
}

fn sense(c: &Control) -> Sense {
    if c.disabled {
        Sense::hover()
    } else {
        Sense::click_and_drag()
    }
}

fn fade(color: Color32, disabled: bool) -> Color32 {
    if disabled {
        color.gamma_multiply(0.4)
    } else {
        color
    }
}

/// `<pt-knob>` : anneau de valeur sur trois quarts de tour, index, bipolaire
/// depuis le centre si `min < 0 < max`. Drag vertical. `size` = diamètre ;
/// le libellé prend 12 px de plus en dessous.
pub fn knob(ui: &mut egui::Ui, theme: &Theme, mut c: Control, size: f32) -> Gesture {
    let label_h = if c.label.is_empty() { 0.0 } else { 12.0 };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(size, size + label_h), sense(&c));
    let g = interact(ui, &response, &mut c, true);
    let hovered = response.hovered() || response.dragged();
    let ink = ui.visuals().text_color();
    let painter = ui.painter();
    let knob_rect = Rect::from_min_size(rect.min, Vec2::splat(size));
    let center = knob_rect.center();
    let r = size / 2.0 - 4.0;
    let start = -3.0 * std::f32::consts::PI / 4.0 - std::f32::consts::PI / 2.0;
    let span = 3.0 * std::f32::consts::PI / 2.0;
    let track = fade(theme.track, c.disabled);
    let accent = fade(c.accent(theme), c.disabled);
    arc(
        painter,
        center,
        r,
        start,
        start + span,
        Stroke::new(3.0_f32, track),
    );
    let f = c.fraction();
    let zero = if logic::bipolar(c.min, c.max) {
        logic::zero_fraction(c.min, c.max)
    } else {
        0.0
    };
    let (a, b) = (start + span * zero, start + span * f);
    arc(
        painter,
        center,
        r,
        a.min(b),
        a.max(b),
        Stroke::new(3.0_f32, accent),
    );
    let angle = start + span * f;
    let dir = Vec2::new(angle.cos(), angle.sin());
    painter.line_segment(
        [center + dir * r * 0.35, center + dir * r * 0.85],
        Stroke::new(3.0_f32, accent),
    );
    label_below(ui, knob_rect, c.label, ink.gamma_multiply(0.7));
    if hovered && !c.disabled {
        response.clone().on_hover_text(c.tooltip());
    }
    g
}

fn arc(painter: &egui::Painter, center: Pos2, r: f32, from: f32, to: f32, stroke: Stroke) {
    if to <= from {
        return;
    }
    let n = ((to - from) * r / 3.0).ceil().max(2.0) as usize;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let t = from + (to - from) * i as f32 / n as f32;
            center + Vec2::new(t.cos(), t.sin()) * r
        })
        .collect();
    painter.add(egui::Shape::line(pts, stroke));
}

/// `<pt-slider>` horizontal : piste, portion parcourue allumée (glow,
/// bipolaire depuis zéro si `min < 0 < max`), flèche au-dessus. Drag
/// horizontal. `width` × 36 px, libellé en dessous.
pub fn slider(ui: &mut egui::Ui, theme: &Theme, mut c: Control, width: f32) -> Gesture {
    let label_h = if c.label.is_empty() { 0.0 } else { 10.0 };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 28.0 + label_h), sense(&c));
    let g = interact(ui, &response, &mut c, false);
    let ink = ui.visuals().text_color();
    let painter = ui.painter();
    let body = Rect::from_min_size(rect.min, Vec2::new(width, 28.0));
    let pad = 8.0;
    let y = body.top() + body.height() * 0.62;
    let span = width - 2.0 * pad;
    let at = |t: f32| body.left() + pad + t * span;
    let f = c.fraction();
    let z = logic::zero_fraction(c.min, c.max);
    let track = fade(theme.track, c.disabled);
    let accent = fade(c.accent(theme), c.disabled);
    painter.line_segment(
        [Pos2::new(at(0.0), y), Pos2::new(at(1.0), y)],
        Stroke::new(4.0_f32, track),
    );
    let (x0, x1) = (at(z).min(at(f)), at(z).max(at(f)));
    // glow puis trait allumé
    painter.line_segment(
        [Pos2::new(x0, y), Pos2::new(x1, y)],
        Stroke::new(8.0_f32, accent.gamma_multiply(0.25)),
    );
    painter.line_segment(
        [Pos2::new(x0, y), Pos2::new(x1, y)],
        Stroke::new(4.0_f32, accent),
    );
    let x = at(f);
    painter.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(x, y - 4.0),
            Pos2::new(x - 5.0, y - 12.0),
            Pos2::new(x + 5.0, y - 12.0),
        ],
        accent,
        Stroke::NONE,
    ));
    label_below(ui, body, c.label, ink.gamma_multiply(0.7));
    if response.hovered() && !c.disabled {
        response.clone().on_hover_text(c.tooltip());
    }
    g
}

/// `<pt-fader>` : piste verticale et poignée. Drag vertical. `height` de
/// la piste, 36 px de large, libellé en dessous.
pub fn fader(ui: &mut egui::Ui, theme: &Theme, mut c: Control, height: f32) -> Gesture {
    let label_h = if c.label.is_empty() { 0.0 } else { 12.0 };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(36.0, height + label_h), sense(&c));
    let g = interact(ui, &response, &mut c, true);
    let ink = ui.visuals().text_color();
    let painter = ui.painter();
    let body = Rect::from_min_size(rect.min, Vec2::new(36.0, height));
    let pad = 8.0;
    let x = body.center().x;
    painter.line_segment(
        [
            Pos2::new(x, body.top() + pad),
            Pos2::new(x, body.bottom() - pad),
        ],
        Stroke::new(4.0_f32, fade(theme.track, c.disabled)),
    );
    let y = body.top() + pad + (1.0 - c.fraction()) * (height - 2.0 * pad);
    painter.rect_filled(
        Rect::from_min_max(
            Pos2::new(body.left() + 4.0, y - 5.0),
            Pos2::new(body.right() - 4.0, y + 5.0),
        ),
        1.0,
        fade(c.accent(theme), c.disabled),
    );
    painter.rect_filled(
        Rect::from_min_max(
            Pos2::new(body.left() + 4.0, y - 1.0),
            Pos2::new(body.right() - 4.0, y + 1.0),
        ),
        0.0,
        ON_ACCENT,
    );
    label_below(ui, body, c.label, ink.gamma_multiply(0.7));
    if response.hovered() && !c.disabled {
        response.clone().on_hover_text(c.tooltip());
    }
    g
}

/// `<pt-switch>` : interrupteur on/off (activator), 28 × 14, libellé à
/// droite. Clic, Espace ou Entrée. Renvoie vrai si l'état a changé.
pub fn switch(
    ui: &mut egui::Ui,
    theme: &Theme,
    checked: &mut bool,
    label: &str,
    disabled: bool,
) -> Response {
    let text_w = if label.is_empty() {
        0.0
    } else {
        ui.painter()
            .layout_no_wrap(
                label.to_string(),
                egui::FontId::proportional(13.0),
                Color32::WHITE,
            )
            .size()
            .x
            + 6.0
    };
    let (rect, mut response) = ui.allocate_exact_size(
        Vec2::new(28.0 + text_w, 18.0),
        if disabled {
            Sense::hover()
        } else {
            Sense::click()
        },
    );
    let mut toggled = response.clicked();
    if response.has_focus()
        && ui.input(|i| i.key_pressed(egui::Key::Space) || i.key_pressed(egui::Key::Enter))
    {
        toggled = true;
    }
    if toggled && !disabled {
        *checked = !*checked;
        response.mark_changed();
    }
    let ink = ui.visuals().text_color();
    let painter = ui.painter();
    let track = Rect::from_min_size(
        Pos2::new(rect.left(), rect.center().y - 7.0),
        Vec2::new(28.0, 14.0),
    );
    let (bg, thumb_x, thumb) = if *checked {
        (theme.accent, track.left() + 16.0, ON_ACCENT)
    } else {
        (theme.track, track.left() + 2.0, THUMB_OFF)
    };
    painter.rect_filled(track, 7.0_f32, fade(bg, disabled));
    painter.rect_filled(
        Rect::from_min_size(Pos2::new(thumb_x, track.top() + 2.0), Vec2::splat(10.0)),
        5.0,
        fade(thumb, disabled),
    );
    if response.has_focus() {
        painter.rect_stroke(
            track.expand(2.0),
            9.0_f32,
            Stroke::new(1.0_f32, theme.accent),
            egui::StrokeKind::Outside,
        );
    }
    if !label.is_empty() {
        painter.text(
            Pos2::new(track.right() + 6.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(13.0),
            fade(ink, disabled),
        );
    }
    response
}

/// `<pt-stepper>` : ‹ valeur › avec bouclage, molette et flèches quand il
/// a le focus. Renvoie vrai si l'index a changé.
pub fn stepper(
    ui: &mut egui::Ui,
    theme: &Theme,
    index: &mut usize,
    options: &[String],
    min_width: f32,
    disabled: bool,
) -> bool {
    let count = options.len();
    let mut delta: i64 = 0;
    let response = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if arrow_button(ui, theme, "‹", disabled || count == 0).clicked() {
                delta -= 1;
            }
            let text = options.get(*index).cloned().unwrap_or_default();
            let (rect, r) = ui.allocate_exact_size(
                Vec2::new(min_width.max(8.0), 16.0),
                if disabled {
                    Sense::hover()
                } else {
                    Sense::click()
                },
            );
            if r.clicked() {
                r.request_focus();
            }
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::proportional(11.0),
                fade(ui.visuals().text_color(), disabled),
            );
            if r.has_focus() {
                ui.painter().rect_stroke(
                    rect.expand(1.0),
                    2.0_f32,
                    Stroke::new(1.0_f32, theme.accent),
                    egui::StrokeKind::Outside,
                );
                let (n, p) = ui.input(|i| {
                    (
                        i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::ArrowUp),
                        i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::ArrowDown),
                    )
                });
                delta += n as i64 - p as i64;
            }
            if r.hovered() {
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll.abs() > 0.5 {
                    delta += if scroll > 0.0 { 1 } else { -1 };
                }
            }
            if arrow_button(ui, theme, "›", disabled || count == 0).clicked() {
                delta += 1;
            }
        })
        .response;
    let _ = response;
    if delta != 0 && !disabled && count > 0 {
        *index = logic::wrap(*index as i64 + delta, count);
        true
    } else {
        false
    }
}

/// Les petits boutons ‹ › du stepper, seuls : piste au repos, accent au survol.
pub fn arrow_button(ui: &mut egui::Ui, theme: &Theme, glyph: &str, disabled: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::splat(16.0),
        if disabled {
            Sense::hover()
        } else {
            Sense::click()
        },
    );
    let hot = response.hovered() && !disabled;
    let (bg, fg) = if hot {
        (theme.accent, ON_ACCENT)
    } else {
        (
            fade(theme.track, disabled),
            fade(ui.visuals().text_color(), disabled),
        )
    };
    ui.painter().rect_filled(rect, 3.0_f32, bg);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        egui::FontId::proportional(12.0),
        fg,
    );
    response
}

/// `<pt-button>` : bouton plat façon Ableton ; `active` le remplit à
/// l'accent (verrouillé). Momentané sinon : `clicked()`.
pub fn button(
    ui: &mut egui::Ui,
    theme: &Theme,
    text: &str,
    active: bool,
    disabled: bool,
) -> Response {
    button_colored(ui, theme, text, active, disabled, theme.accent)
}

/// `<pt-button>` avec sa propre teinte d'accent (ex. rouge d'enregistrement).
pub fn button_colored(
    ui: &mut egui::Ui,
    theme: &Theme,
    text: &str,
    active: bool,
    disabled: bool,
    accent: Color32,
) -> Response {
    let font = egui::FontId::proportional(11.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font.clone(), Color32::WHITE);
    let size = Vec2::new(galley.size().x + 20.0, 22.0);
    let (rect, response) = ui.allocate_exact_size(
        size,
        if disabled {
            Sense::hover()
        } else {
            Sense::click()
        },
    );
    let hot = response.hovered() && !disabled;
    let down = response.is_pointer_button_down_on();
    let ink = ui.visuals().text_color();
    let (bg, fg, border) = if active || down {
        (fade(accent, disabled), ON_ACCENT, Color32::TRANSPARENT)
    } else {
        (
            fade(ui.visuals().widgets.inactive.weak_bg_fill, disabled),
            fade(ink, disabled),
            if disabled {
                theme.track.gamma_multiply(0.6)
            } else if hot {
                accent
            } else {
                accent.gamma_multiply(0.55)
            },
        )
    };
    let painter = ui.painter();
    painter.rect(
        rect,
        5.0,
        bg,
        Stroke::new(1.0_f32, border),
        egui::StrokeKind::Inside,
    );
    painter.text(rect.center(), egui::Align2::CENTER_CENTER, text, font, fg);
    response
}

/// `<pt-led>` : point de 8 px, éteint sur la piste, allumé à sa couleur avec
/// un halo.
pub fn led(ui: &mut egui::Ui, theme: &Theme, on: bool, color: Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
    let painter = ui.painter();
    if on {
        painter.circle_filled(rect.center(), 6.0_f32, color.gamma_multiply(0.25));
        painter.circle_filled(rect.center(), 4.0_f32, color);
    } else {
        painter.circle_filled(rect.center(), 4.0_f32, theme.track);
    }
    response
}

/// `<pt-vumeter>` : segments avec mémoire de pic, horizontal ou vertical.
pub fn vumeter(
    ui: &mut egui::Ui,
    level: f32,
    peak: f32,
    segments: usize,
    size: Vec2,
    horizontal: bool,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    let count = segments.max(1);
    let gap = 2.0;
    let length = if horizontal { size.x } else { size.y };
    let seg = (length - gap * (count as f32 - 1.0)) / count as f32;
    let lit = logic::lit_segments(level, count);
    for i in 0..count {
        let color = match logic::segment_zone(i, count) {
            2 => VU_RED,
            1 => VU_YELLOW,
            _ => VU_GREEN,
        };
        let fill = if i < lit { color } else { VU_OFF };
        let offset = i as f32 * (seg + gap);
        let r = if horizontal {
            Rect::from_min_size(
                Pos2::new(rect.left() + offset, rect.top()),
                Vec2::new(seg, size.y),
            )
        } else {
            Rect::from_min_size(
                Pos2::new(rect.left(), rect.bottom() - offset - seg),
                Vec2::new(size.x, seg),
            )
        };
        painter.rect_filled(r, 0.0_f32, fill);
    }
    let peak = peak.clamp(0.0, 1.0);
    if peak > 0.0 {
        let r = if horizontal {
            Rect::from_min_size(
                Pos2::new(
                    (rect.left() + peak * size.x - 1.0).min(rect.right() - 2.0),
                    rect.top(),
                ),
                Vec2::new(2.0, size.y),
            )
        } else {
            Rect::from_min_size(
                Pos2::new(
                    rect.left(),
                    (rect.bottom() - peak * size.y - 1.0).max(rect.top()),
                ),
                Vec2::new(size.x, 2.0),
            )
        };
        painter.rect_filled(r, 0.0_f32, Color32::WHITE);
    }
    response
}

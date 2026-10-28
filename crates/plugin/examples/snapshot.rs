//! Image de l'interface sans hôte : dessine quelques trames avec un `egui::Context`
//! nu, rasterise les triangles en logiciel et écrit un PNG (via `sips`).
//! `cargo run --release -p make-it-drum --example snapshot -- sortie.png [son.wav] [pad sélectionné]`
use std::sync::Arc;

use nih_plug::context::gui::GuiContext;
use nih_plug::context::PluginApi;
use nih_plug::params::internals::ParamPtr;
use nih_plug::prelude::ParamSetter;
use nih_plug::wrapper::state::PluginState;
use nih_plug_egui::egui;

use make_it_drum::capture::Recorder;
use make_it_drum::editor::{draw, style, EditorState};
use make_it_drum::history::History;
use make_it_drum::params::KitPlayerParams;
use make_it_drum::slot::KitSlot;
use make_it_drum::variations::Variations;
use make_it_drum_core::kit::Kit;

struct NoHost;
impl GuiContext for NoHost {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Standalone
    }
    fn request_resize(&self) -> bool {
        false
    }
    unsafe fn raw_begin_set_parameter(&self, _: ParamPtr) {}
    unsafe fn raw_set_parameter_normalized(&self, _: ParamPtr, _: f32) {}
    unsafe fn raw_end_set_parameter(&self, _: ParamPtr) {}
    fn get_state(&self) -> PluginState {
        unimplemented!("pas d'hôte")
    }
    fn set_state(&self, _: PluginState) {}
}

const PPP: f32 = 2.0;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = args
        .first()
        .cloned()
        .unwrap_or_else(|| "snapshot.png".into());
    let source = args.get(1).cloned();
    let selected: Option<usize> = args.get(2).and_then(|s| s.parse().ok());

    let params = Arc::new(KitPlayerParams::default());
    let slot = Arc::new(KitSlot::new());
    slot.set_sample_rate(44100.0);
    let history = History::new();
    let recorder = Recorder::new();
    let variations = Variations::new();
    if let Some(path) = source {
        let bytes = std::fs::read(&path).expect("lecture du son");
        let ext = std::path::Path::new(&path)
            .extension()
            .map(|e| e.to_string_lossy().into_owned());
        let sample =
            make_it_drum_core::wav::decode_audio(&bytes, ext.as_deref()).expect("décodage");
        let seeds: Vec<u32> = (0..20).map(|i| 42 + i * 7919).collect();
        let kit = Kit::generate("porte qui claque", &sample, Some(&seeds), |_, _| {}).expect("kit");
        variations.reset(make_it_drum::loader::seeds_of(&kit));
        variations.record((0..20).map(|i| Some(7 + i)).collect());
        variations.record((0..20).map(|i| Some(99 + i)).collect());
        slot.publish(Arc::new(kit));
        slot.set_status("Kit « porte qui claque » — 20 pads");
    } else {
        slot.set_status("Déposez un son pour commencer");
    }

    let (w, h) = params.editor_state.size();
    let ctx = egui::Context::default();
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    let mut state = EditorState::new();
    state.select(selected);
    let run = |_task| {};
    let input = || egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(w as f32, h as f32),
        )),
        viewports: {
            let mut v = egui::ViewportIdMap::default();
            v.insert(
                egui::ViewportId::ROOT,
                egui::ViewportInfo {
                    native_pixels_per_point: Some(PPP),
                    ..Default::default()
                },
            );
            v
        },
        ..Default::default()
    };
    let mut atlas: Option<(usize, usize, Vec<egui::Color32>)> = None;
    let mut full = None;
    for i in 0..4 {
        let out = ctx.run(input(), |ctx| {
            if i == 0 {
                style(ctx);
            }
            draw(
                ctx,
                &setter,
                &mut state,
                &params,
                &slot,
                &history,
                &recorder,
                &variations,
                &run,
            );
        });
        for (_, delta) in &out.textures_delta.set {
            if delta.pos.is_none() {
                let (sz, px): ([usize; 2], Vec<egui::Color32>) = match &delta.image {
                    egui::ImageData::Color(c) => (c.size, c.pixels.clone()),
                    egui::ImageData::Font(f) => (f.size, f.srgba_pixels(None).collect()),
                };
                atlas = Some((sz[0], sz[1], px));
            }
        }
        full = Some(out);
    }
    let full = full.unwrap();
    let (tw, th, tex) = atlas.expect("atlas de police");
    let prims = ctx.tessellate(full.shapes, PPP);

    let pw = (w as f32 * PPP) as usize;
    let ph = (h as f32 * PPP) as usize;
    let mut img = vec![[0.0f32; 4]; pw * ph];
    for prim in prims {
        let egui::epaint::Primitive::Mesh(mesh) = prim.primitive else {
            continue;
        };
        let clip = prim.clip_rect;
        for tri in mesh.indices.chunks(3) {
            let v: Vec<&egui::epaint::Vertex> =
                tri.iter().map(|&i| &mesh.vertices[i as usize]).collect();
            let (a, b, c) = (v[0], v[1], v[2]);
            let (ax, ay) = (a.pos.x * PPP, a.pos.y * PPP);
            let (bx, by) = (b.pos.x * PPP, b.pos.y * PPP);
            let (cx, cy) = (c.pos.x * PPP, c.pos.y * PPP);
            let min_x = ax.min(bx).min(cx).max(clip.min.x * PPP).floor().max(0.0) as usize;
            let max_x = (ax.max(bx).max(cx).min(clip.max.x * PPP).ceil() as usize).min(pw);
            let min_y = ay.min(by).min(cy).max(clip.min.y * PPP).floor().max(0.0) as usize;
            let max_y = (ay.max(by).max(cy).min(clip.max.y * PPP).ceil() as usize).min(ph);
            let area = (bx - ax) * (cy - ay) - (cx - ax) * (by - ay);
            if area.abs() < 1e-6 {
                continue;
            }
            for y in min_y..max_y {
                for x in min_x..max_x {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let w0 = ((bx - px) * (cy - py) - (cx - px) * (by - py)) / area;
                    let w1 = ((cx - px) * (ay - py) - (ax - px) * (cy - py)) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                        continue;
                    }
                    let u = w0 * a.uv.x + w1 * b.uv.x + w2 * c.uv.x;
                    let vv = w0 * a.uv.y + w1 * b.uv.y + w2 * c.uv.y;
                    let tx = ((u * tw as f32) as usize).min(tw - 1);
                    let ty = ((vv * th as f32) as usize).min(th - 1);
                    let t = tex[ty * tw + tx];
                    let col = |ch: fn(&egui::Color32) -> u8| {
                        (w0 * ch(&a.color) as f32
                            + w1 * ch(&b.color) as f32
                            + w2 * ch(&c.color) as f32)
                            / 255.0
                    };
                    let src = [
                        col(|c| c.r()) * t.r() as f32 / 255.0,
                        col(|c| c.g()) * t.g() as f32 / 255.0,
                        col(|c| c.b()) * t.b() as f32 / 255.0,
                        col(|c| c.a()) * t.a() as f32 / 255.0,
                    ];
                    let d = &mut img[y * pw + x];
                    for k in 0..4 {
                        d[k] = src[k] + d[k] * (1.0 - src[3]);
                    }
                }
            }
        }
    }
    let mut ppm = format!("P6\n{pw} {ph}\n255\n").into_bytes();
    for p in &img {
        for v in &p[..3] {
            ppm.push((v.clamp(0.0, 1.0) * 255.0).round() as u8);
        }
    }
    let ppm_path = format!("{out}.ppm");
    std::fs::write(&ppm_path, ppm).expect("écriture");
    let status = std::process::Command::new("sips")
        .args(["-s", "format", "png", &ppm_path, "--out", &out])
        .output()
        .expect("sips");
    if !status.status.success() {
        eprintln!("sips : {}", String::from_utf8_lossy(&status.stderr));
    }
    std::fs::remove_file(&ppm_path).ok();
    println!("{out} ({pw}×{ph})");
}

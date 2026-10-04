//! Bundled-only shaping/rasterization and bounded reusable text presentation.
//! CPU glyph cache generations replace rather than retain old resources; one GPU composite page.
use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, fontdb};
use rustcraft_content::fonts::FontResources;
use std::time::Instant;
pub const GLYPH_LIMIT: usize = 2048;
pub const CACHE_BYTES: usize = 16 * 1024 * 1024;
pub const SURFACE_LIMIT: u32 = 2048;
#[derive(Clone, Debug, PartialEq)]
pub struct TextRun {
    pub text: String,
    pub position: [f32; 2],
    pub pixels: f32,
    pub role: String,
    pub color: [u8; 3],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Caret {
    pub line: usize,
    pub index: usize,
}
#[derive(Default, Clone, Debug)]
pub struct TextMetrics {
    pub glyphs: usize,
    pub cpu_bytes: usize,
    pub surface_bytes: usize,
    pub rebuilds: u64,
    pub layouts: u64,
    pub layout_us: u128,
    pub raster_us: u128,
    pub replacements: usize,
    pub upload_us: u128,
}
pub struct TextSystem {
    fonts: FontSystem,
    resources: FontResources,
    cache: SwashCache,
    last: Vec<TextRun>,
    size: (u32, u32),
    caret: Option<Caret>,
    generation_input_bytes: usize,
    pub rgba: Vec<u8>,
    pub metrics: TextMetrics,
}
struct BundledFallback;
impl cosmic_text::Fallback for BundledFallback {
    fn common_fallback(&self) -> &[&'static str] {
        &[
            "Noto Sans",
            "Noto Sans Math",
            "Noto Sans Arabic",
            "Noto Sans Hebrew",
            "Noto Sans Devanagari",
            "Noto Emoji",
        ]
    }
    fn forbidden_fallback(&self) -> &[&'static str] {
        &[]
    }
    fn script_fallback(&self, _: unicode_script::Script, _: &str) -> &[&'static str] {
        &[]
    }
}
impl TextSystem {
    pub fn new(resources: FontResources) -> Result<Self, String> {
        let mut db = fontdb::Database::new();
        for role in [
            "rustcraft:font/ui",
            "rustcraft:font/debug",
            "rustcraft:font/mono",
        ] {
            for f in resources.resolve(role)? {
                if !db
                    .faces()
                    .any(|face| face.families.iter().any(|(name, _)| name == &f.family))
                {
                    db.load_font_source(fontdb::Source::Binary(f.bytes.clone()));
                }
            }
        }
        for role in [
            "rustcraft:font/ui",
            "rustcraft:font/debug",
            "rustcraft:font/mono",
        ] {
            for f in resources.resolve(role)? {
                if !db
                    .faces()
                    .any(|face| face.families.iter().any(|(name, _)| name == &f.family))
                {
                    return Err(format!("font family {} unavailable for {role}", f.family));
                }
            }
        }
        if db.faces().count() == 0 {
            return Err("no bundled/resource fonts".into());
        }
        let fonts =
            FontSystem::new_with_locale_and_db_and_fallback("en-US".into(), db, BundledFallback);
        Ok(Self {
            fonts,
            resources,
            cache: SwashCache::new(),
            last: vec![],
            size: (0, 0),
            caret: None,
            generation_input_bytes: 0,
            rgba: vec![],
            metrics: Default::default(),
        })
    }
    pub fn face_count(&self) -> usize {
        self.fonts.db().faces().count()
    }
    pub fn coverage(&self, c: char) -> Option<String> {
        self.resources
            .resolve("rustcraft:font/debug")
            .unwrap()
            .iter()
            .find(|f| {
                ttf_parser::Face::parse(&f.bytes, 0)
                    .ok()
                    .and_then(|face| face.glyph_index(c))
                    .is_some()
            })
            .map(|f| f.family.clone())
    }
    pub fn update(
        &mut self,
        runs: &[TextRun],
        width: u32,
        height: u32,
        caret: Option<Caret>,
    ) -> bool {
        let size = (
            width.clamp(1, SURFACE_LIMIT),
            height.clamp(1, SURFACE_LIMIT),
        );
        let mut remaining = 16384usize;
        let runs = runs
            .iter()
            .take(64)
            .filter_map(|r| {
                if remaining == 0 {
                    return None;
                }
                let mut r = r.clone();
                let end = r
                    .text
                    .char_indices()
                    .map(|(i, c)| i + c.len_utf8())
                    .take_while(|end| *end <= remaining)
                    .last()
                    .unwrap_or(0);
                r.text.truncate(end);
                remaining -= r.text.len();
                Some(r)
            })
            .collect::<Vec<_>>();
        if self.last == runs && self.size == size && self.caret == caret {
            return false;
        }
        let bytes = runs.iter().map(|r| r.text.len()).sum::<usize>();
        // Bound hidden shaper/codepoint-support caches too, including repeated unsupported glyphs.
        if self.generation_input_bytes + bytes > 65536 {
            self.fonts = FontSystem::new_with_locale_and_db_and_fallback(
                "en-US".into(),
                self.fonts.db().clone(),
                BundledFallback,
            );
            self.cache = SwashCache::new();
            self.metrics.rebuilds += 1;
            self.generation_input_bytes = 0;
        }
        self.generation_input_bytes += bytes;
        self.size = size;
        self.last = runs.clone();
        self.caret = caret;
        self.rgba = vec![0; (size.0 * size.1 * 4) as usize];
        self.metrics.replacements = 0;
        self.metrics.layout_us = 0;
        self.metrics.raster_us = 0;
        for (number, run) in runs.iter().enumerate() {
            let started = Instant::now();
            let pixels = run.pixels.clamp(6., 96.).round();
            let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(pixels, pixels * 1.25));
            buffer.set_size(
                Some((size.0 as f32 - run.position[0]).max(1.)),
                Some(size.1 as f32),
            );
            let family = &self
                .resources
                .resolve(&run.role)
                .unwrap_or_else(|_| self.resources.resolve("rustcraft:font/ui").unwrap())[0]
                .family;
            buffer.set_text(
                &run.text,
                &Attrs::new().family(Family::Name(family)),
                Shaping::Advanced,
                None,
            );
            buffer.shape_until_scroll(&mut self.fonts, false);
            self.metrics.layout_us += started.elapsed().as_micros();
            self.metrics.layouts += 1;
            let started = Instant::now();
            for line in buffer.layout_runs() {
                for g in line.glyphs {
                    self.metrics.replacements += usize::from(g.glyph_id == 0);
                    if self.cache.image_cache.len() >= GLYPH_LIMIT
                        || self.cache_bytes() >= CACHE_BYTES
                    {
                        self.cache = SwashCache::new();
                        self.metrics.rebuilds += 1;
                    }
                    let glyph = g.physical((run.position[0].round(), run.position[1].round()), 1.);
                    self.cache.with_pixels(
                        &mut self.fonts,
                        glyph.cache_key,
                        Color::rgb(run.color[0], run.color[1], run.color[2]),
                        |x, y, color| {
                            let x = x + glyph.x;
                            let y = y + glyph.y + line.line_y as i32;
                            if x >= 0 && y >= 0 && x < size.0 as i32 && y < size.1 as i32 {
                                let index = ((y as u32 * size.0 + x as u32) * 4) as usize;
                                let a = color.a() as u32;
                                let old = self.rgba[index + 3] as u32;
                                let out = a + old * (255 - a) / 255;
                                {
                                    for (channel, value) in
                                        [color.r(), color.g(), color.b()].into_iter().enumerate()
                                    {
                                        self.rgba[index + channel] = ((value as u32 * a
                                            + self.rgba[index + channel] as u32 * old * (255 - a)
                                                / 255)
                                            .checked_div(out)
                                            .unwrap_or(0))
                                            as u8;
                                    }
                                    self.rgba[index + 3] = out as u8;
                                }
                            }
                        },
                    );
                }
                if number == 0
                    && let Some(c) = caret
                    && line.line_i == c.line
                {
                    let candidate_x = line
                        .glyphs
                        .iter()
                        .find_map(|g| {
                            if c.index == g.start {
                                Some(if g.level.is_ltr() { g.x } else { g.x + g.w })
                            } else if c.index == g.end {
                                Some(if g.level.is_ltr() { g.x + g.w } else { g.x })
                            } else {
                                None
                            }
                        })
                        .or_else(|| (line.glyphs.is_empty() && c.index == 0).then_some(0.));
                    let Some(x) = candidate_x.map(|x| x + run.position[0]) else {
                        continue;
                    };
                    for y in 0..pixels as u32 {
                        let cy = (run.position[1] + line.line_top) as u32 + y;
                        let cx = x.round().max(0.) as u32;
                        if cy < size.1 && cx < size.0 {
                            let i = ((cy * size.0 + cx) * 4) as usize;
                            self.rgba[i..i + 4].fill(255);
                        }
                    }
                }
            }
            self.metrics.raster_us += started.elapsed().as_micros();
        }
        // A single oversized glyph is bounded by 96px; clear after sampling if it crosses the byte cap.
        if self.cache_bytes() > CACHE_BYTES {
            self.cache = SwashCache::new();
            self.metrics.rebuilds += 1;
        }
        self.metrics.glyphs = self.cache.image_cache.len();
        self.metrics.cpu_bytes = self.cache_bytes();
        self.metrics.surface_bytes = self.rgba.len();
        true
    }
    fn cache_bytes(&self) -> usize {
        self.cache
            .image_cache
            .values()
            .flatten()
            .map(|i| i.data.len())
            .sum()
    }
}
pub const MULTILINGUAL: &str = "English: The quick brown fox 123\nРусский: Состояние чанка: загружен\nУкраїнська: Налаштування світу\nΕλληνικά: Δοκιμή κειμένου\nCombining: café café\nGraphemes: 🦀 👨‍👩‍👧‍👦 🇺🇦\nArabic: مرحبا بالعالم\nHebrew: שלום עולם\n日本語 中文 한국어\nDevanagari: नमस्ते दुनिया\nMissing: \u{10ffff}";
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_only_shaping_coverage_reuse_and_cache_soak() {
        let mut s = TextSystem::new(FontResources::builtin()).unwrap();
        assert_eq!(s.face_count(), 7);
        for c in "AéЖіїєґΩ∑中日한مرحباשלוםनमस्ते🦀".chars() {
            assert!(s.coverage(c).is_some(), "missing {c}");
        }
        let mut run = TextRun {
            text: MULTILINGUAL.into(),
            position: [0., 0.],
            pixels: 12.,
            role: "rustcraft:font/debug".into(),
            color: [255; 3],
        };
        let now = Instant::now();
        assert!(s.update(std::slice::from_ref(&run), 800, 480, None));
        let cold = now.elapsed();
        let now = Instant::now();
        assert!(!s.update(std::slice::from_ref(&run), 800, 480, None));
        let warm = now.elapsed();
        assert!(s.rgba.iter().any(|b| *b != 0));
        // Known bundled CJK stresses real glyph entries without a minute of unrelated
        // unsupported-script fallback work in every workspace edit loop.
        for start in (0x4e00..0x6e00)
            .step_by(256)
            .chain((0x4e00..0x6e00).step_by(256))
        {
            run.text = (start..start + 256).filter_map(char::from_u32).collect();
            s.update(std::slice::from_ref(&run), 800, 480, None);
            assert!(s.metrics.glyphs <= GLYPH_LIMIT);
            assert!(s.metrics.cpu_bytes <= CACHE_BYTES);
        }
        // Low glyph variety must also retire hidden shaper/support-cache generations.
        for i in 0..6 {
            run.text = format!("{} {i}", "x".repeat(13000));
            s.update(std::slice::from_ref(&run), 800, 480, None);
        }
        assert!(s.metrics.rebuilds > 0);
        println!(
            "TEXT_ACCEPTANCE cold={cold:?} unchanged={warm:?} {:?}",
            s.metrics
        );
        assert!(s.update(&[run], 1200, 720, None));
        assert_eq!(s.rgba.len(), 1200 * 720 * 4);
    }
}

#[cfg(test)]
mod resource_tests {
    use super::*;
    #[test]
    fn semantic_font_replacement_changes_rendered_role_without_renderer_changes() {
        let mut r = FontResources::builtin();
        let run = TextRun {
            text: "Привет café".into(),
            position: [0., 0.],
            pixels: 12.,
            role: "rustcraft:font/ui".into(),
            color: [255; 3],
        };
        let mut a = TextSystem::new(r.clone()).unwrap();
        a.update(std::slice::from_ref(&run), 200, 32, None);
        let mut alternate = r.resolve("rustcraft:font/ui").unwrap().to_vec();
        alternate.swap(0, 1);
        alternate[0].owner = "sandbox_test:fonts".into();
        r.register("rustcraft:font/ui", alternate).unwrap();
        let mut b = TextSystem::new(r).unwrap();
        b.update(&[run], 200, 32, None);
        assert_ne!(a.rgba, b.rgba);
    }
    #[test]
    fn bundled_cmap_inventory() {
        for f in FontResources::builtin()
            .resolve("rustcraft:font/debug")
            .unwrap()
        {
            let face = ttf_parser::Face::parse(&f.bytes, 0).unwrap();
            let mut covered = std::collections::BTreeSet::new();
            for table in face.tables().cmap.unwrap().subtables {
                if table.is_unicode() {
                    table.codepoints(|n| {
                        covered.insert(n);
                    });
                }
            }
            println!(
                "FONT_COVERAGE {} bytes={} glyphs={} cmap={} license=OFL1.1",
                f.family,
                f.bytes.len(),
                face.number_of_glyphs(),
                covered.len()
            );
        }
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    #[test]
    fn representative_text_service_measurements() {
        let mut s = TextSystem::new(FontResources::builtin()).unwrap();
        for (label, text) in [
            ("no_text", ""),
            (
                "static_page",
                "Overview\nWorld columns: 81\nMesh pending: 12",
            ),
            ("console_idle", "COMMAND > /help"),
            ("console_typing", "COMMAND > /help config"),
            ("first_unicode", MULTILINGUAL),
            (
                "warm_unicode_changed",
                "Русский: загружен Українська Ελληνικά مرحبا नमस्ते 中文",
            ),
        ] {
            let run = TextRun {
                text: text.into(),
                position: [0., 0.],
                pixels: 12.,
                role: "rustcraft:font/debug".into(),
                color: [255; 3],
            };
            let start = Instant::now();
            s.update(std::slice::from_ref(&run), 800, 480, None);
            let changed = start.elapsed();
            let metrics = s.metrics.clone();
            let start = Instant::now();
            assert!(!s.update(&[run], 800, 480, None));
            println!(
                "TEXT_COST {label} changed={changed:?} unchanged={:?} layout_us={} raster_us={} glyphs={} cpu_bytes={}",
                start.elapsed(),
                metrics.layout_us,
                metrics.raster_us,
                metrics.glyphs,
                metrics.cpu_bytes
            );
        }
    }
}

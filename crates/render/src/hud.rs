//! Generic Unicode text runs and read-only presentation geometry.
use crate::{AtlasRegion, Camera, PageVertices, Vertex, page_vertices_mut};
use rustcraft_engine_core::BlockPos;

const SELECTION_EXPANSION: f32 = 0.002;

/// Emits world-space line-list edges. The camera projection clips near-plane intersections, while
/// the dedicated render pipeline tests fragments against the already-populated scene depth.
fn selection_segments(position: BlockPos) -> Vec<([f32; 3], [f32; 3])> {
    let lo = [
        position.x as f32 - SELECTION_EXPANSION,
        position.y as f32 - SELECTION_EXPANSION,
        position.z as f32 - SELECTION_EXPANSION,
    ];
    let hi = [
        position.x as f32 + 1.0 + SELECTION_EXPANSION,
        position.y as f32 + 1.0 + SELECTION_EXPANSION,
        position.z as f32 + 1.0 + SELECTION_EXPANSION,
    ];
    let point = |index: usize| {
        std::array::from_fn(|axis| {
            if index & (1 << axis) == 0 {
                lo[axis]
            } else {
                hi[axis]
            }
        })
    };
    let mut result = Vec::with_capacity(12);
    for corner in 0..8 {
        for bit in [1, 2, 4] {
            if corner & bit == 0 {
                result.push((point(corner), point(corner | bit)));
            }
        }
    }
    result
}
#[derive(Clone)]
pub struct Slot {
    pub model: Option<crate::inspection::BlockModel>,
    pub top: AtlasRegion,
    pub side: AtlasRegion,
    pub bottom: AtlasRegion,
    pub tint: [f32; 3],
    pub count: u16,
    pub block_3d: bool,
}
pub struct HudSnapshot<'a> {
    pub slots: [Option<Slot>; 9],
    pub selected: usize,
    pub target: Option<BlockPos>,
    pub target_geometry: Option<(
        &'a [[[f32; 3]; 2]],
        rustcraft_engine_core::orientation::ModelRotation,
    )>,
    pub text: &'a str,
    pub caret: Option<crate::text::Caret>,
    pub items: &'a [[f32; 3]],
    pub mining_progress: Option<f32>,
    pub inventory_open: bool,
    pub inventory_slots: [Option<Slot>; 36],
    pub crafting_slots: [Option<Slot>; 4],
    pub crafting_output: Option<Slot>,
    pub cursor_slot: Option<Slot>,
    pub cursor_position: [f32; 2],
}
impl Default for HudSnapshot<'_> {
    fn default() -> Self {
        Self {
            slots: [const { None }; 9],
            selected: 0,
            target: None,
            target_geometry: None,
            text: "",
            caret: None,
            items: &[],
            mining_progress: None,
            inventory_open: false,
            inventory_slots: [const { None }; 36],
            crafting_slots: [const { None }; 4],
            crafting_output: None,
            cursor_slot: None,
            cursor_position: [0.; 2],
        }
    }
}
#[must_use]
pub fn inventory_slot_position(index: usize) -> Option<(f32, f32)> {
    if index >= 36 {
        return None;
    }
    if index < 9 {
        Some((8. + index as f32 * 18., 142.))
    } else {
        let i = index - 9;
        Some((8. + (i % 9) as f32 * 18., 84. + (i / 9) as f32 * 18.))
    }
}
#[must_use]
pub fn destroy_stage(progress: f32) -> u8 {
    ((progress.clamp(0., 1.) * 10.).floor() as u8).min(9)
}
#[must_use]
pub fn beta_gui_scale(width: u32, height: u32) -> f32 {
    let mut factor = 1u32;
    while width / (factor + 1) >= 320 && height / (factor + 1) >= 240 {
        factor += 1;
    }
    factor as f32
}
#[must_use]
pub fn beta_gui_project_point(v: [f32; 3]) -> [f32; 3] {
    let q = beta_gui_direction(v.map(|v| v - 0.5));
    [8. + 10. * q[0], 8. + 10. * q[1], 7. + 10. * q[2]]
}
fn beta_gui_direction(q: [f32; 3]) -> [f32; 3] {
    // OpenGL postmultiplication: S(1,1,-1) Rx(210) Ry(45) Ry(-90).
    // See exact-fidelity note M3.8. Screen Y already points down here.
    let (sy, cy) = (-45f32).to_radians().sin_cos();
    let (sx, cx) = 210f32.to_radians().sin_cos();
    let q = [q[0] * cy + q[2] * sy, q[1], -q[0] * sy + q[2] * cy];
    [q[0], q[1] * cx - q[2] * sx, -(q[1] * sx + q[2] * cx)]
}
pub fn gui_face_light(normal: [f32; 3]) -> f32 {
    let n = beta_gui_direction(normal);
    let (s, c) = 120f32.to_radians().sin_cos();
    let mut light = 0.4f32;
    for p in [[0.2f32, 1., -0.7], [-0.2, 1., 0.7]] {
        let len = p.iter().map(|v| v * v).sum::<f32>().sqrt();
        let l = [
            p[0] / len,
            (p[1] * c - p[2] * s) / len,
            (p[1] * s + p[2] * c) / len,
        ];
        light += 0.6 * n.iter().zip(l).map(|(a, b)| a * b).sum::<f32>().max(0.);
    }
    light.min(1.)
}
/// Shared by hotbar, inventory, cursor and the offscreen inspector. Native 16px icon.
pub fn gui_block_vertices(
    out: &mut Vec<Vertex>,
    slot: Slot,
    origin: [f32; 2],
    scale: f32,
    viewport: [f32; 2],
) {
    let model = slot.model.unwrap_or(crate::inspection::BlockModel {
        triangles: None,
        state: rustcraft_engine_core::BlockState::new(rustcraft_engine_core::BlockId(0)),
        textures: [
            slot.side,
            slot.side,
            slot.side,
            slot.side,
            slot.top,
            slot.bottom,
        ],
        tints: [slot.tint; 6],
        rotation: rustcraft_engine_core::orientation::ModelRotation::IDENTITY,
    });
    model.gui_vertices(out, origin, scale, viewport);
}

/// Page-preserving form used by production inventory, hotbar and cursor rendering.
pub fn gui_block_page_vertices(
    out: &mut Vec<PageVertices>,
    slot: Slot,
    origin: [f32; 2],
    scale: f32,
    viewport: [f32; 2],
) {
    let model = slot.model.unwrap_or(crate::inspection::BlockModel {
        triangles: None,
        state: rustcraft_engine_core::BlockState::new(rustcraft_engine_core::BlockId(0)),
        textures: [
            slot.side,
            slot.side,
            slot.side,
            slot.side,
            slot.top,
            slot.bottom,
        ],
        tints: [slot.tint; 6],
        rotation: rustcraft_engine_core::orientation::ModelRotation::IDENTITY,
    });
    model.gui_page_vertices(out, origin, scale, viewport);
}

#[derive(Default)]
pub struct HudGeometry {
    pub vertices: Vec<Vertex>,
    pub item_vertices: Vec<Vertex>,
    pub item_pages: Vec<PageVertices>,
    pub gui_vertices: Vec<Vertex>,
    pub hotbar_vertices: Vec<Vertex>,
    pub player_vertices: Vec<Vertex>,
    pub debug_vertices: Vec<Vertex>,
    pub selection_vertices: Vec<Vertex>,
    pub debug_changed: bool,
    pub text_runs: Vec<crate::text::TextRun>,
    pub debug_runs: Vec<crate::text::TextRun>,
    debug_scale: f32,
    cached_text: String,
    cached_size: (u32, u32),
    width: f32,
    height: f32,
}
impl HudGeometry {
    pub fn set_text_scale(&mut self, scale: f32) {
        if self.debug_scale != scale {
            self.debug_scale = scale;
            self.cached_size = (0, 0);
        }
    }

    pub fn build_gui_background(&mut self, width: u32, height: u32, open: bool, selected: usize) {
        self.gui_vertices.clear();
        self.hotbar_vertices.clear();
        let scale = beta_gui_scale(width, height);
        if !open {
            // Center the resolved hotbar at the bottom of the scaled viewport.
            let sw = width as f32 / scale;
            let sh = height as f32 / scale;
            let x = sw / 2. - 91.;
            let y = sh - 22.;
            Self::gui_rect(
                x * scale,
                y * scale,
                182. * scale,
                22. * scale,
                width,
                height,
                &mut self.hotbar_vertices,
            );
            // Selector overlaps the slot origin by one logical pixel.
            let sx = (x - 1. + selected.min(8) as f32 * 20.) * scale;
            Self::gui_rect(
                sx,
                (y - 1.) * scale,
                24. * scale,
                22. * scale,
                width,
                height,
                &mut self.hotbar_vertices,
            );
            return;
        }
        let w = 176. * scale;
        let h = 166. * scale;
        let x = (width as f32 - w) / 2.;
        let y = (height as f32 - h) / 2.;
        for (px, py, u, v) in [
            (x, y, 0., 0.),
            (x + w, y, 1., 0.),
            (x + w, y + h, 1., 1.),
            (x, y + h, 0., 1.),
        ] {
            self.gui_vertices.push(Vertex {
                position: [
                    px / width as f32 * 2. - 1.,
                    1. - py / height as f32 * 2.,
                    0.,
                ],
                uv: [u, v],
                shade: 1.,
                color: [1.; 3],
            });
        }
        let first = self.gui_vertices.clone();
        self.gui_vertices
            .extend([first[0], first[1], first[2], first[0], first[2], first[3]]);
        self.gui_vertices.drain(0..4);
    }
    #[allow(clippy::too_many_arguments)]
    fn gui_rect(x: f32, y: f32, w: f32, h: f32, width: u32, height: u32, out: &mut Vec<Vertex>) {
        let uv = [[0., 0.], [1., 0.], [1., 1.], [0., 1.]];
        for i in [0, 1, 2, 0, 2, 3] {
            out.push(Vertex {
                position: [
                    (x + [0., w, w, 0.][i]) / width as f32 * 2. - 1.,
                    1. - (y + [0., 0., h, h][i]) / height as f32 * 2.,
                    0.,
                ],
                uv: uv[i],
                shade: 1.,
                color: [1.; 3],
            });
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn skin_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let uv = [[0., 0.], [1., 0.], [1., 1.], [0., 1.]];
        for i in [0, 1, 2, 0, 2, 3] {
            self.player_vertices.push(Vertex {
                position: [
                    (x + [0., w, w, 0.][i]) / self.width * 2. - 1.,
                    1. - (y + [0., 0., h, h][i]) / self.height * 2.,
                    0.,
                ],
                uv: uv[i],
                shade: 1.,
                color: [1.; 3],
            });
        }
    }
    fn rect(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 3],
        region: Option<AtlasRegion>,
    ) {
        for i in [0, 1, 2, 0, 2, 3] {
            let uv = [[0., 0.], [1., 0.], [1., 1.], [0., 1.]][i];
            self.vertices.push(Vertex {
                position: [
                    (x + uv[0] * w) / self.width * 2. - 1.,
                    1. - (y + uv[1] * h) / self.height * 2.,
                    0.,
                ],
                uv: region.map_or(uv, |value| crate::region_uv(value, uv)),
                shade: if region.is_some() { 1. } else { -1. },
                color,
            });
        }
    }
    fn slot_item(&mut self, slot: Slot, x: f32, y: f32, size: f32, scale: f32) {
        let count = slot.count;
        if slot.block_3d {
            gui_block_page_vertices(
                &mut self.item_pages,
                slot,
                [x, y],
                scale,
                [self.width, self.height],
            );
        } else {
            self.item_rect(
                x + 2. * scale,
                y + 2. * scale,
                size - 4. * scale,
                size - 4. * scale,
                slot.tint,
                slot.side,
            );
        }
        if count > 1 {
            self.text(
                &count.to_string(),
                x + size - 9. * scale,
                y + size - 8. * scale,
                scale,
            );
        }
    }
    fn item_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 3], region: AtlasRegion) {
        let out = page_vertices_mut(&mut self.item_pages, region.texture);
        for i in [0, 1, 2, 0, 2, 3] {
            let uv = [[0., 0.], [1., 0.], [1., 1.], [0., 1.]][i];
            out.push(Vertex {
                position: [
                    (x + uv[0] * w) / self.width * 2. - 1.,
                    1. - (y + uv[1] * h) / self.height * 2.,
                    0.,
                ],
                uv: crate::region_uv(region, uv),
                shade: 1.,
                color,
            });
        }
    }
    pub fn build(&mut self, s: &HudSnapshot, width: u32, height: u32, camera: Camera) {
        self.player_vertices.clear();
        self.text_runs.clear();
        self.vertices.clear();
        self.selection_vertices.clear();
        self.item_vertices.clear();
        self.item_pages.clear();
        self.width = width as f32;
        self.height = height as f32;
        let scale = beta_gui_scale(width, height);
        let left = self.width / 2. - 91. * scale;
        for i in 0..9 {
            if s.inventory_open {
                break;
            }
            let x = left + i as f32 * 20. * scale;
            if let Some(slot) = s.slots[i].clone() {
                // GuiIngame item origin: center-90 + slot*20 + 2, height-19.
                self.slot_item(
                    slot,
                    x + 2. * scale,
                    self.height - 19. * scale,
                    16. * scale,
                    scale,
                );
            }
        }
        if s.inventory_open {
            let panel_w = 176. * scale;
            let panel_h = 166. * scale;
            let panel_x = (self.width - panel_w) / 2.;
            let panel_y = (self.height - panel_h) / 2.;
            let slot = 18. * scale;
            let craft_x = panel_x + 88. * scale;
            let craft_y = panel_y + 26. * scale;
            // Resolved preview regions: head, torso, arms and legs.
            self.skin_rect(
                panel_x + 48. * scale,
                panel_y + 18. * scale,
                16. * scale,
                16. * scale,
            );
            self.skin_rect(
                panel_x + 46. * scale,
                panel_y + 34. * scale,
                20. * scale,
                26. * scale,
            );
            self.skin_rect(
                panel_x + 42. * scale,
                panel_y + 35. * scale,
                4. * scale,
                23. * scale,
            );
            self.skin_rect(
                panel_x + 66. * scale,
                panel_y + 35. * scale,
                4. * scale,
                23. * scale,
            );
            self.skin_rect(
                panel_x + 47. * scale,
                panel_y + 60. * scale,
                8. * scale,
                20. * scale,
            );
            self.skin_rect(
                panel_x + 57. * scale,
                panel_y + 60. * scale,
                8. * scale,
                20. * scale,
            );
            for (index, item) in s.inventory_slots.iter().cloned().enumerate() {
                if let (Some(item), Some((x, y))) = (item, inventory_slot_position(index)) {
                    self.slot_item(item, panel_x + x * scale, panel_y + y * scale, slot, scale);
                }
            }
            for i in 0..4 {
                let x = craft_x + (i % 2) as f32 * 18. * scale;
                let y = craft_y + (i / 2) as f32 * 18. * scale;
                if let Some(item) = s.crafting_slots[i].clone() {
                    self.slot_item(item, x, y, slot, scale);
                }
            }
            self.rect(
                panel_x + 144. * scale,
                panel_y + 36. * scale,
                18. * scale,
                18. * scale,
                [0.08; 3],
                None,
            );
            if let Some(item) = s.crafting_output.clone() {
                self.slot_item(
                    item,
                    panel_x + 144. * scale,
                    panel_y + 36. * scale,
                    slot,
                    scale,
                );
            }
            if let Some(item) = s.cursor_slot.clone() {
                let cursor_starts = self
                    .item_pages
                    .iter()
                    .map(|page| (page.texture, page.vertices.len()))
                    .collect::<Vec<_>>();
                self.slot_item(
                    item,
                    s.cursor_position[0] - 8. * scale,
                    s.cursor_position[1] - 8. * scale,
                    18. * scale,
                    scale,
                );
                // Cursor has its own depth band above ordinary slots.
                for page in &mut self.item_pages {
                    let start = cursor_starts
                        .iter()
                        .find_map(|(texture, len)| (*texture == page.texture).then_some(*len))
                        .unwrap_or(0);
                    for vertex in &mut page.vertices[start..] {
                        vertex.position[2] -= 0.25;
                    }
                }
            }
        }
        self.item_vertices.extend(
            self.item_pages
                .iter()
                .flat_map(|page| page.vertices.iter().copied()),
        );
        if !s.inventory_open {
            let cx = self.width / 2.;
            let cy = self.height / 2.;
            self.rect(
                cx - 5. * scale,
                cy - scale / 2.,
                10. * scale,
                scale,
                [1.; 3],
                None,
            );
            self.rect(
                cx - scale / 2.,
                cy - 5. * scale,
                scale,
                10. * scale,
                [1.; 3],
                None,
            );
        }
        if !s.inventory_open
            && let Some(p) = s.target
        {
            let segments = if let Some((edges, rotation)) = s.target_geometry {
                edges
                    .iter()
                    .map(|edge| {
                        let edge =
                            edge.map(|v| rotation.point(v).map(|v| v + (v - 0.5).signum() * 0.002));
                        let edge =
                            edge.map(|v| [v[0] + p.x as f32, v[1] + p.y as f32, v[2] + p.z as f32]);
                        (edge[0], edge[1])
                    })
                    .collect()
            } else {
                selection_segments(p)
            };
            for (a, b) in segments {
                for position in [a, b] {
                    self.selection_vertices.push(Vertex {
                        position,
                        uv: [0.0; 2],
                        shade: 1.0,
                        color: [0.0; 3],
                    });
                }
            }
        }
        for item in s.items {
            let matrix = camera.view_projection();
            let v = [item[0], item[1], item[2], 1.];
            let r: [f32; 4] =
                std::array::from_fn(|row| (0..4).map(|col| matrix[col][row] * v[col]).sum());
            if r[3] > 0.05 {
                let x = (r[0] / r[3] + 1.) * self.width / 2.;
                let y = (1. - r[1] / r[3]) * self.height / 2.;
                self.rect(
                    x - 4. * scale,
                    y - 4. * scale,
                    8. * scale,
                    8. * scale,
                    [1., 0.8, 0.2],
                    None,
                );
            }
        }
        self.debug_changed = self.cached_text != s.text || self.cached_size != (width, height);
        if !self.debug_changed {
            return;
        }
        self.cached_text.clear();
        self.cached_text.push_str(s.text);
        self.cached_size = (width, height);
        std::mem::swap(&mut self.vertices, &mut self.debug_vertices);
        self.vertices.clear();
        self.debug_runs.clear();
        if !s.text.is_empty() {
            self.rect(
                3.,
                3.,
                self.width - 6.,
                (s.text.lines().count() as f32 * 15. * self.debug_scale.max(1.)).min(self.height),
                [0.035; 3],
                None,
            );
            self.debug_runs.push(crate::text::TextRun {
                text: s.text.into(),
                position: [5., 4.],
                pixels: 12.,
                role: "rustcraft:font/debug".into(),
                color: [255; 3],
            });
        }
        std::mem::swap(&mut self.vertices, &mut self.debug_vertices);
    }
    fn text(&mut self, text: &str, x: f32, y: f32, scale: f32) {
        self.text_runs.push(crate::text::TextRun {
            text: text.into(),
            position: [x, y],
            pixels: 12. * scale,
            role: "rustcraft:font/ui".into(),
            color: [255; 3],
        });
    }
}
/// Specialist offscreen fixtures use the SAME Unicode shaper and rasterizer, converted to
/// opaque pixel geometry for their existing capture path. Normal HUD uses the reusable texture.
pub fn diagnostic_label(text: &str, origin: [f32; 2], viewport: [f32; 2]) -> Vec<Vertex> {
    let mut system =
        crate::text::TextSystem::new(rustcraft_content::fonts::FontResources::builtin()).unwrap();
    system.update(
        &[crate::text::TextRun {
            text: text.into(),
            position: origin,
            pixels: 12.,
            role: "rustcraft:font/debug".into(),
            color: [255; 3],
        }],
        viewport[0] as u32,
        viewport[1] as u32,
        None,
    );
    let mut h = HudGeometry {
        width: viewport[0],
        height: viewport[1],
        ..Default::default()
    };
    for (i, p) in system.rgba.as_chunks::<4>().0.iter().enumerate() {
        if p[3] >= 128 {
            h.rect(
                (i % (viewport[0] as usize)) as f32,
                (i / (viewport[0] as usize)) as f32,
                1.,
                1.,
                [1.; 3],
                None,
            );
        }
    }
    for tri in h.vertices.as_chunks_mut::<3>().0 {
        tri.swap(1, 2);
    }
    h.vertices
}
#[cfg(test)]
mod tests {
    use super::*;
    use rustcraft_engine_core::Vec3;

    fn test_camera(position: Vec3) -> Camera {
        Camera {
            position,
            yaw: 0.0,
            pitch: 0.0,
            aspect: 16.0 / 9.0,
            fov_y: 70.0_f32.to_radians(),
            near: 0.05,
            far: 100.0,
        }
    }

    #[test]
    fn semantic_presentation_uses_full_regions_and_preserves_preview() {
        let mut geometry = HudGeometry::default();
        geometry.build_gui_background(800, 600, false, 3);
        assert_eq!(geometry.hotbar_vertices.len(), 12);
        let original_positions = geometry
            .hotbar_vertices
            .iter()
            .map(|v| v.position)
            .collect::<Vec<_>>();
        let regions = [
            AtlasRegion::from_pixels(crate::TextureHandle(3), [1024, 512], [17, 31, 182, 22])
                .unwrap(),
            AtlasRegion::from_pixels(crate::TextureHandle(7), [128, 256], [43, 69, 24, 22])
                .unwrap(),
        ];
        let draws = crate::resolve_presentation_quads(&mut geometry.hotbar_vertices, &regions);
        assert_eq!(
            draws,
            vec![
                (crate::TextureHandle(3), 0..6),
                (crate::TextureHandle(7), 6..12)
            ]
        );
        assert_eq!(
            geometry
                .hotbar_vertices
                .iter()
                .map(|v| v.position)
                .collect::<Vec<_>>(),
            original_positions
        );
        for (index, quad) in geometry
            .hotbar_vertices
            .as_chunks::<6>()
            .0
            .iter()
            .enumerate()
        {
            assert_eq!(quad[0].uv, regions[index].uv_min);
            assert_eq!(quad[2].uv, regions[index].uv_max);
        }
        let snapshot = HudSnapshot {
            inventory_open: true,
            ..Default::default()
        };
        geometry.build(&snapshot, 800, 600, test_camera(Vec3::ZERO));
        assert_eq!(geometry.player_vertices.len(), 36);
        geometry.build_gui_background(800, 600, true, 0);
        assert_eq!(
            geometry.player_vertices.len(),
            36,
            "background rebuild must preserve preview"
        );
        assert_eq!(geometry.gui_vertices[0].uv, [0., 0.]);
        assert_eq!(geometry.gui_vertices[2].uv, [1., 1.]);
        geometry.build(&snapshot, 800, 600, test_camera(Vec3::ZERO));
        assert_eq!(
            geometry.player_vertices.len(),
            36,
            "preview must not accumulate"
        );
        geometry.build(&HudSnapshot::default(), 800, 600, test_camera(Vec3::ZERO));
        assert!(geometry.player_vertices.is_empty());
    }

    #[test]
    fn selection_geometry_is_world_space_expanded_and_camera_independent() {
        let position = BlockPos { x: -2, y: 4, z: 9 };
        let segments = selection_segments(position);
        assert_eq!(segments.len(), 12);
        assert_eq!(
            segments[0],
            ([-2.002, 3.998, 8.998], [-0.998, 3.998, 8.998])
        );
        assert!(
            segments
                .iter()
                .flat_map(|(a, b)| a.iter().chain(b))
                .all(|v| v.is_finite())
        );
        assert_eq!(SELECTION_EXPANSION, 0.002);
        // The same topology is used at all camera poses; only the GPU projection/depth test varies.
        let front = test_camera(Vec3::new(0.5, 0.5, -2.0));
        let inside = test_camera(Vec3::new(0.5, 0.5, 0.5));
        assert_ne!(front.view_projection(), inside.view_projection());
        assert_eq!(selection_segments(position), segments);
    }

    #[test]
    fn beta_inventory_layout_keeps_hotbar_on_bottom_row() {
        assert_eq!(inventory_slot_position(0), Some((8., 142.)));
        assert_eq!(inventory_slot_position(8), Some((152., 142.)));
        assert_eq!(inventory_slot_position(9), Some((8., 84.)));
        assert_eq!(inventory_slot_position(35), Some((152., 120.)));
    }
    #[test]
    fn destroy_stage_mapping_has_ten_stages() {
        assert_eq!(destroy_stage(0.), 0);
        assert_eq!(destroy_stage(0.099), 0);
        assert_eq!(destroy_stage(0.1), 1);
        assert_eq!(destroy_stage(1.), 9);
    }
    #[test]
    fn beta_gui_scale_matches_scaled_resolution_constraints() {
        assert_eq!(beta_gui_scale(1280, 720), 3.0);
        assert_eq!(beta_gui_scale(800, 600), 2.0);
        assert_eq!(beta_gui_scale(319, 239), 1.0);
    }
    #[test]
    fn effective_gui_matrix_has_source_derived_bounds_depth_and_visible_faces() {
        assert_eq!(beta_gui_project_point([0.5; 3]), [8., 8., 7.]);
        let points = crate::geometry::FACES
            .into_iter()
            .flat_map(|f| f.positions)
            .map(beta_gui_project_point)
            .collect::<Vec<_>>();
        for (axis, expected) in [(0, (0.928932, 15.071068)), (1, (0.134339, 15.865661))] {
            let min = points.iter().map(|v| v[axis]).fold(f32::INFINITY, f32::min);
            let max = points
                .iter()
                .map(|v| v[axis])
                .fold(f32::NEG_INFINITY, f32::max);
            assert!((min - expected.0).abs() < 0.00002);
            assert!((max - expected.1).abs() < 0.00002);
        }
        let mut visible = Vec::new();
        for f in crate::geometry::FACES {
            let p = f.positions.map(beta_gui_project_point);
            // Pixel Y down => outward CCW screen faces have a negative determinant.
            let area = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1])
                - (p[1][1] - p[0][1]) * (p[2][0] - p[0][0]);
            assert!(area.abs() > 40.);
            if area < 0. {
                visible.push(f.direction);
            }
            assert!((0.4..=1.).contains(&gui_face_light(f.normal)));
        }
        assert_eq!(
            visible,
            [crate::Face::North, crate::Face::East, crate::Face::Top]
        );
    }
    #[test]
    fn inventory_hotbar_cursor_share_geometry_independent_of_world_camera() {
        let t = crate::AtlasRegion::grid_cell(crate::TextureHandle(0), 16, 16, 3, 2).unwrap();
        let slot = Slot {
            model: None,
            top: t,
            side: t,
            bottom: t,
            tint: [1.; 3],
            count: 23,
            block_3d: true,
        };
        let mut s = HudSnapshot::default();
        s.slots[0] = Some(slot.clone());
        let mut h = HudGeometry::default();
        let mut camera = crate::diagnostic::Stage::Triangle.camera(1.);
        h.build(&s, 800, 600, camera);
        let hotbar = h.item_vertices.clone();
        assert_eq!(hotbar.len(), 36);
        camera.yaw = 2.;
        camera.pitch = 0.7;
        camera.position = crate::Vec3::new(100., 40., -50.);
        h.build(&s, 800, 600, camera);
        for (a, b) in hotbar.iter().zip(&h.item_vertices) {
            assert_eq!(a.position, b.position);
        }
        s.inventory_open = true;
        s.inventory_slots[0] = Some(slot.clone());
        h.build(&s, 800, 600, camera);
        let inventory = h.item_vertices.clone();
        s.inventory_slots[0] = None;
        s.cursor_slot = Some(slot);
        s.cursor_position = [400., 300.];
        h.build(&s, 800, 600, camera);
        for other in [&inventory, &h.item_vertices] {
            assert_eq!(other.len(), 36);
            for i in 0..36 {
                assert_eq!(hotbar[i].uv, other[i].uv);
                assert_eq!(hotbar[i].shade, other[i].shade);
                for axis in 0..3 {
                    assert!(
                        ((hotbar[i].position[axis] - hotbar[0].position[axis])
                            - (other[i].position[axis] - other[0].position[axis]))
                            .abs()
                            < 1e-5
                    );
                }
            }
        }
    }

    #[test]
    fn gui_block_keeps_top_side_and_bottom_atlas_pages() {
        let regions = [
            crate::AtlasRegion::full(crate::TextureHandle(1)),
            crate::AtlasRegion::full(crate::TextureHandle(1)),
            crate::AtlasRegion::full(crate::TextureHandle(1)),
            crate::AtlasRegion::full(crate::TextureHandle(1)),
            crate::AtlasRegion::full(crate::TextureHandle(0)),
            crate::AtlasRegion::full(crate::TextureHandle(2)),
        ];
        let model = crate::inspection::BlockModel {
            triangles: None,
            state: rustcraft_engine_core::BlockState::new(rustcraft_engine_core::BlockId(9)),
            textures: regions,
            tints: [[1.; 3]; 6],
            rotation: rustcraft_engine_core::orientation::ModelRotation::IDENTITY,
        };
        let slot = Slot {
            model: Some(model.clone()),
            top: regions[crate::Face::Top as usize],
            side: regions[crate::Face::North as usize],
            bottom: regions[crate::Face::Bottom as usize],
            tint: [1.; 3],
            count: 1,
            block_3d: true,
        };
        let mut snapshot = HudSnapshot::default();
        snapshot.slots[0] = Some(slot);
        let mut geometry = HudGeometry::default();
        geometry.build(
            &snapshot,
            800,
            600,
            crate::diagnostic::Stage::Triangle.camera(800. / 600.),
        );
        assert_eq!(
            geometry
                .item_pages
                .iter()
                .map(|page| page.texture)
                .collect::<Vec<_>>(),
            [
                crate::TextureHandle(0),
                crate::TextureHandle(1),
                crate::TextureHandle(2)
            ]
        );
        assert_eq!(geometry.item_vertices.len(), 36);
        assert_eq!(
            geometry
                .item_pages
                .iter()
                .map(|page| page.vertices.len())
                .sum::<usize>(),
            36
        );
    }
    #[test]
    fn beta_gui_cube_projection_is_finite_and_non_degenerate() {
        let points = [
            [0., 0., 0.],
            [1., 0., 0.],
            [1., 1., 0.],
            [0., 1., 0.],
            [0., 0., 1.],
            [1., 0., 1.],
            [1., 1., 1.],
            [0., 1., 1.],
        ]
        .map(beta_gui_project_point);
        let min_x = points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
        let max_x = points
            .iter()
            .map(|p| p[0])
            .fold(f32::NEG_INFINITY, f32::max);
        let min_y = points.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        let max_y = points
            .iter()
            .map(|p| p[1])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(points.iter().flatten().all(|v| v.is_finite()));
        assert!(max_x - min_x > 0.5);
        assert!(max_y - min_y > 0.5);
    }
    #[test]
    fn debug_text_is_cached_invalidated_on_resize_and_cleared_when_hidden() {
        let mut h = HudGeometry::default();
        let mut s = HudSnapshot {
            caret: None,
            slots: [const { None }; 9],
            selected: 0,
            target: None,
            target_geometry: None,
            text: "FPS 60",
            items: &[],
            mining_progress: None,
            inventory_open: false,
            inventory_slots: [const { None }; 36],
            crafting_slots: [const { None }; 4],
            crafting_output: None,
            cursor_slot: None,
            cursor_position: [0.; 2],
        };
        let camera = crate::diagnostic::Stage::Triangle.camera(1.);
        h.build(&s, 800, 600, camera);
        assert!(h.debug_changed && !h.debug_vertices.is_empty());
        let vertices = h.debug_vertices.len();
        h.build(&s, 800, 600, camera);
        assert!(!h.debug_changed);
        assert_eq!(h.debug_vertices.len(), vertices);
        s.selected = 8;
        h.build(&s, 800, 600, camera);
        assert!(!h.debug_changed);
        h.build(&s, 1280, 720, camera);
        assert!(h.debug_changed);
        h.set_text_scale(1.5);
        h.build(&s, 1280, 720, camera);
        assert!(h.debug_changed);
        h.build(&s, 1280, 720, camera);
        assert!(!h.debug_changed);
        s.text = "";
        h.build(&s, 1280, 720, camera);
        assert!(h.debug_vertices.is_empty());
        assert!(!h.vertices.is_empty()); // crosshair/hotbar survive hiding F3
    }
}

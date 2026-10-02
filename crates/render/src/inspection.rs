//! Content-inspector boundary: resolve a state once, inspect world/GUI/item representations.
//! No runtime, window, first-party IDs, or local asset paths are required.
use crate::{
    AtlasRegion, BlockState, BlockTextureResolver, Face, PageVertices, TextureHandle, Vertex,
    geometry, hud, page_vertices_mut,
};
use rustcraft_engine_core::orientation::ModelRotation;
#[derive(Debug, Clone, Copy)]
pub struct BlockModel {
    pub state: BlockState,
    pub textures: [AtlasRegion; 6],
    pub tints: [[f32; 3]; 6],
    pub rotation: ModelRotation,
}
impl BlockModel {
    pub fn resolve(state: BlockState, resolver: &impl BlockTextureResolver) -> Option<Self> {
        let mut textures = [AtlasRegion::full(TextureHandle(0)); 6];
        for f in geometry::FACES {
            textures[f.direction as usize] = resolver.state_texture(state, f.direction)?;
        }
        Some(Self {
            state,
            textures,
            tints: geometry::FACES.map(|f| resolver.tint(state.block, f.direction)),
            rotation: resolver.model_rotation(state),
        })
    }
    /// Canonical positions and UVs, with a model/state transform applied only to positions.
    pub fn world_vertices(&self, out: &mut Vec<Vertex>) {
        for f in geometry::FACES {
            for i in f.indices {
                out.push(Vertex {
                    position: self.rotation.point(f.positions[i]),
                    uv: crate::region_uv(self.textures[f.direction as usize], f.uv_corners[i]),
                    shade: 1.,
                    color: self.tints[f.direction as usize],
                });
            }
        }
    }
    pub fn world_page_vertices(&self, pages: &mut Vec<PageVertices>) {
        for f in geometry::FACES {
            let region = self.textures[f.direction as usize];
            let out = page_vertices_mut(pages, region.texture);
            for i in f.indices {
                out.push(Vertex {
                    position: self.rotation.point(f.positions[i]),
                    uv: crate::region_uv(region, f.uv_corners[i]),
                    shade: 1.,
                    color: self.tints[f.direction as usize],
                });
            }
        }
    }
    pub fn gui_vertices(
        &self,
        out: &mut Vec<Vertex>,
        origin: [f32; 2],
        scale: f32,
        viewport: [f32; 2],
    ) {
        let texture = self.textures[0].texture;
        assert!(
            self.textures.iter().all(|region| region.texture == texture),
            "single-page GUI diagnostic helper received a multi-page model"
        );
        for f in geometry::FACES {
            let normal = self.rotation.transform(f.normal);
            for i in f.indices {
                let p = hud::beta_gui_project_point(self.rotation.point(f.positions[i]));
                out.push(Vertex {
                    position: [
                        (origin[0] + p[0] * scale) / viewport[0] * 2. - 1.,
                        1. - (origin[1] + p[1] * scale) / viewport[1] * 2.,
                        0.5 - p[2] * 0.01,
                    ],
                    uv: crate::region_uv(self.textures[f.direction as usize], f.uv_corners[i]),
                    shade: hud::gui_face_light(normal),
                    color: self.tints[f.direction as usize],
                });
            }
        }
    }
    /// GUI geometry grouped by the physical atlas page sampled by each face.
    pub fn gui_page_vertices(
        &self,
        pages: &mut Vec<PageVertices>,
        origin: [f32; 2],
        scale: f32,
        viewport: [f32; 2],
    ) {
        for f in geometry::FACES {
            let normal = self.rotation.transform(f.normal);
            let region = self.textures[f.direction as usize];
            let out = page_vertices_mut(pages, region.texture);
            for i in f.indices {
                let p = hud::beta_gui_project_point(self.rotation.point(f.positions[i]));
                out.push(Vertex {
                    position: [
                        (origin[0] + p[0] * scale) / viewport[0] * 2. - 1.,
                        1. - (origin[1] + p[1] * scale) / viewport[1] * 2.,
                        0.5 - p[2] * 0.01,
                    ],
                    uv: crate::region_uv(region, f.uv_corners[i]),
                    shade: hud::gui_face_light(normal),
                    color: self.tints[f.direction as usize],
                });
            }
        }
    }
    pub fn texture(&self, face: Face) -> AtlasRegion {
        self.textures[face as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustcraft_engine_core::orientation::{Facing, ModelRotation};
    use rustcraft_engine_core::{BlockId, BlockPos, World};
    struct Materials;
    impl BlockTextureResolver for Materials {
        fn texture(&self, _: BlockId, face: Face) -> Option<AtlasRegion> {
            AtlasRegion::grid_cell(TextureHandle(0), 16, 16, face as u32, 0)
        }
        fn opaque(&self, b: BlockId) -> bool {
            b.0 != 0
        }
        fn model_rotation(&self, s: BlockState) -> ModelRotation {
            ModelRotation::facing(s.facing())
        }
    }
    #[test]
    fn state_survives_extraction_rotates_geometry_and_keeps_uv_attached() {
        let mut world = World::new(BlockId(0));
        let pos = BlockPos { x: 0, y: 0, z: 0 };
        let state = BlockState::new(BlockId(1)).with_facing(Facing::East);
        world.set_state(pos, state);
        let extracted = crate::RenderWorld::from_world(&world);
        let chunk = extracted.chunks().next().unwrap();
        assert_eq!(chunk.state((0, 0, 0)), state);
        let mesh = crate::build_chunk_mesh(&extracted, chunk, &Materials);
        assert_eq!(mesh.vertices.len(), 24);
        let model = BlockModel::resolve(state, &Materials).unwrap();
        let mut inspected = Vec::new();
        model.world_vertices(&mut inspected);
        for (face, f) in geometry::FACES.iter().enumerate() {
            for (j, i) in f.indices.into_iter().enumerate() {
                let v = inspected[face * 6 + j];
                let w = mesh.vertices[face * 4 + i];
                assert_eq!(v.position, w.position);
                assert_eq!(v.uv, w.uv);
                assert_eq!(v.position, model.rotation.point(f.positions[i]));
                assert_eq!(
                    v.uv,
                    crate::region_uv(model.textures[face], f.uv_corners[i])
                );
            }
        }
    }
}

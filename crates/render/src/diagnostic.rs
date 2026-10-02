//! Isolated development scenes. No runtime, extraction, dirty queues or gameplay resolver.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Triangle,
    CubeNoCull,
    Cube,
    Checker,
    Atlas,
    Platform,
    Chunk,
    SectionNegative,
    SectionZero,
    SectionPositive,
    Normal,
    NormalLit,
}

impl Stage {
    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "triangle" => Ok(Self::Triangle),
            "cube-no-cull" => Ok(Self::CubeNoCull),
            "cube" => Ok(Self::Cube),
            "checker" => Ok(Self::Checker),
            "atlas" => Ok(Self::Atlas),
            "platform" => Ok(Self::Platform),
            "chunk" => Ok(Self::Chunk),
            "section-negative" => Ok(Self::SectionNegative),
            "section-zero" => Ok(Self::SectionZero),
            "section-positive" => Ok(Self::SectionPositive),
            "normal" => Ok(Self::Normal),
            "normal-lit" => Ok(Self::NormalLit),
            _ => Err(format!(
                "unknown diagnostic stage {name}; see docs/RENDER_DIAGNOSTICS.md"
            )),
        }
    }

    pub fn uses_atlas(self) -> bool {
        !matches!(
            self,
            Self::Triangle | Self::CubeNoCull | Self::Cube | Self::Checker
        )
    }

    pub fn normal_world(self) -> bool {
        matches!(self, Self::Normal | Self::NormalLit)
    }

    pub fn textured(self) -> bool {
        !matches!(self, Self::Triangle | Self::CubeNoCull | Self::Cube)
    }

    pub fn camera(self, aspect: f32) -> Camera {
        let position = match self {
            Self::Chunk => Vec3::new(1.5, 3.6, 7.0),
            // Fixed eye across all section tests, so origin mistakes cannot be
            // hidden by moving the camera along with the section under test.
            Self::SectionNegative | Self::SectionZero | Self::SectionPositive => {
                Vec3::new(1.5, 10.0, 48.0)
            }
            _ => Vec3::new(0.0, 1.6, 4.0),
        };
        Camera {
            position,
            yaw: 0.0,
            pitch: 0.0,
            aspect,
            fov_y: 70.0_f32.to_radians(),
            near: 0.05,
            far: 256.0,
        }
    }

    pub fn matrix(self, aspect: f32) -> [[f32; 4]; 4] {
        let camera = self.camera(aspect);
        let target = match self {
            Self::Chunk => Vec3::new(1.5, 1.5, 1.5),
            Self::SectionNegative | Self::SectionZero | Self::SectionPositive => {
                Vec3::new(1.5, 1.5, 1.5)
            }
            _ => Vec3::new(0.0, 0.5, 0.0),
        };
        // Deliberately bypass gameplay yaw/pitch entirely.
        transpose(matrix_mul(
            projection(camera),
            look_at(camera.position, target),
        ))
    }

    pub fn mesh(self) -> CpuMesh {
        assert!(
            !self.normal_world(),
            "normal stage uses client world extraction"
        );
        if self == Self::Triangle {
            return CpuMesh {
                vertices: [[-0.5, 0.0, 0.0], [0.5, 0.0, 0.0], [0.0, 1.0, 0.0]]
                    .map(|position| Vertex {
                        position,
                        uv: [0.0; 2],
                        shade: 1.0,
                        color: [1.0, 0.1, 0.1],
                    })
                    .to_vec(),
                indices: vec![0, 1, 2],
            };
        }
        if matches!(
            self,
            Self::Chunk | Self::SectionNegative | Self::SectionZero | Self::SectionPositive
        ) {
            return section_mesh(match self {
                Self::SectionNegative => -1,
                Self::SectionPositive => 1,
                _ => 0,
            });
        }
        let mut mesh = CpuMesh::default();
        if self == Self::Platform {
            for z in -1..=1 {
                for x in -1..=1 {
                    cube(&mut mesh, [x as f32, 0.0, z as f32], self);
                }
            }
        } else {
            cube(&mut mesh, [-0.5, 0.0, -0.5], self);
        }
        mesh
    }
}

// Explicit, independent geometry oracle; does not call append_face/the mesher.
// Face order: +Z red, -Z cyan, +X green, -X magenta, +Y yellow, -Y blue.
const CUBE: [[[f32; 3]; 4]; 6] = [
    [[0., 1., 1.], [0., 0., 1.], [1., 0., 1.], [1., 1., 1.]],
    [[1., 1., 0.], [1., 0., 0.], [0., 0., 0.], [0., 1., 0.]],
    [[1., 1., 1.], [1., 0., 1.], [1., 0., 0.], [1., 1., 0.]],
    [[0., 1., 0.], [0., 0., 0.], [0., 0., 1.], [0., 1., 1.]],
    [[0., 1., 0.], [0., 1., 1.], [1., 1., 1.], [1., 1., 0.]],
    [[0., 0., 1.], [0., 0., 0.], [1., 0., 0.], [1., 0., 1.]],
];
const COLORS: [[f32; 3]; 6] = [
    [1., 0., 0.],
    [0., 1., 1.],
    [0., 1., 0.],
    [1., 0., 1.],
    [1., 1., 0.],
    [0., 0., 1.],
];
// Top-left, bottom-left, bottom-right, top-right for outward CCW corners.
const UV: [[f32; 2]; 4] = [[0., 0.], [0., 1.], [1., 1.], [1., 0.]];

fn cube(mesh: &mut CpuMesh, origin: [f32; 3], stage: Stage) {
    for (face, corners) in CUBE.iter().enumerate() {
        let base = mesh.vertices.len() as u32;
        for (corner, uv) in corners.iter().zip(UV) {
            mesh.vertices.push(Vertex {
                position: std::array::from_fn(|axis| corner[axis] + origin[axis]),
                uv: if stage.uses_atlas() {
                    region_uv(
                        AtlasRegion::grid_cell(TextureHandle(0), 16, 16, 1, 0).unwrap(),
                        uv,
                    )
                } else {
                    uv
                },
                shade: 1.0,
                color: if stage.textured() {
                    [1.0; 3]
                } else {
                    COLORS[face]
                },
            });
        }
        mesh.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

fn section_mesh(section_y: i32) -> CpuMesh {
    struct Stone;
    impl BlockTextureResolver for Stone {
        fn opaque(&self, block: BlockId) -> bool {
            block == BlockId(1)
        }
        fn texture(&self, _: BlockId, _: Face) -> Option<AtlasRegion> {
            AtlasRegion::grid_cell(TextureHandle(0), 16, 16, 1, 0)
        }
    }
    let position = ChunkPos { x: 0, z: 0 };
    let mut source = World::new(BlockId(0));
    for z in 0..3 {
        for x in 0..3 {
            source.set(
                BlockPos {
                    x,
                    y: section_y * 16 + 1,
                    z,
                },
                BlockId(1),
            );
        }
    }
    let world = RenderWorld::from_world(&source);
    let chunk = world
        .chunks()
        .find(|chunk| chunk.position == position && chunk.section_y == section_y)
        .expect("diagnostic section");
    eprintln!(
        "diagnostic section=(0,{section_y},0) world_origin={:?}",
        section_origin(position, section_y)
    );
    let mut mesh = build_chunk_mesh(&world, chunk, &Stone);
    for v in &mut mesh.vertices {
        v.shade = 1.0;
    }
    mesh
}

pub(super) fn checker(device: &wgpu::Device, queue: &wgpu::Queue) -> Texture {
    // Project-owned 2x2 orientation checker: red/green top, blue/white bottom.
    upload_texture(
        device,
        queue,
        2,
        2,
        &[
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ],
    )
}

pub(super) fn copy_surface(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    texture: &wgpu::Texture,
    config: &wgpu::SurfaceConfiguration,
) -> wgpu::Buffer {
    assert!(
        config.usage.contains(wgpu::TextureUsages::COPY_SRC),
        "surface does not support diagnostic capture"
    );
    assert!(
        matches!(
            config.format,
            wgpu::TextureFormat::Bgra8UnormSrgb
                | wgpu::TextureFormat::Rgba8UnormSrgb
                | wgpu::TextureFormat::Bgra8Unorm
                | wgpu::TextureFormat::Rgba8Unorm
        ),
        "unsupported capture format"
    );
    let stride = (config.width * 4).div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("diagnostic-readback"),
        size: u64::from(stride) * u64::from(config.height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(config.height),
            },
        },
        wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        },
    );
    buffer
}

pub(super) fn save_capture(
    device: &wgpu::Device,
    buffer: wgpu::Buffer,
    config: &wgpu::SurfaceConfiguration,
    path: &Path,
) {
    let (tx, rx) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
    device
        .poll(wgpu::PollType::Wait)
        .expect("wait for diagnostic capture");
    rx.recv()
        .expect("capture callback")
        .expect("map capture buffer");
    let data = buffer.slice(..).get_mapped_range();
    let stride = (config.width * 4).div_ceil(256) * 256;
    let mut rgba = Vec::with_capacity((config.width * config.height * 4) as usize);
    for row in data.chunks(stride as usize) {
        rgba.extend_from_slice(&row[..config.width as usize * 4]);
    }
    if matches!(
        config.format,
        wgpu::TextureFormat::Bgra8UnormSrgb | wgpu::TextureFormat::Bgra8Unorm
    ) {
        for pixel in rgba.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
    }
    // Never overwrite earlier visual evidence.
    let file =
        std::fs::File::create_new(path).expect("create diagnostic PNG (path must not exist)");
    let mut png = png::Encoder::new(file, config.width, config.height);
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header()
        .expect("PNG header")
        .write_image_data(&rgba)
        .expect("PNG capture");
    eprintln!("captured surface: {}", path.display());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_cube_has_isolated_outward_quads_with_continuous_uvs() {
        let mesh = Stage::Checker.mesh();
        assert_eq!(mesh.vertices.len(), 24);
        assert_eq!(mesh.indices.len(), 36);
        let normals = [
            [0., 0., 1.],
            [0., 0., -1.],
            [1., 0., 0.],
            [-1., 0., 0.],
            [0., 1., 0.],
            [0., -1., 0.],
        ];
        for (face, normal) in normals.into_iter().enumerate() {
            let base = (face * 4) as u32;
            assert_eq!(
                &mesh.indices[face * 6..face * 6 + 6],
                &[base, base + 1, base + 2, base, base + 2, base + 3]
            );
            for (vertex, uv) in mesh.vertices[face * 4..face * 4 + 4].iter().zip(UV) {
                assert_eq!(vertex.uv, uv);
                assert_eq!(vertex.shade, 1.0);
            }
            for tri in mesh.indices[face * 6..face * 6 + 6].as_chunks::<3>().0 {
                let p: [[f32; 3]; 3] =
                    std::array::from_fn(|i| mesh.vertices[tri[i] as usize].position);
                let ab = Vec3::new(p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]);
                let ac = Vec3::new(p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]);
                assert_eq!(cross(ab, ac), Vec3::new(normal[0], normal[1], normal[2]));
                let uv: [[f32; 2]; 3] = std::array::from_fn(|i| mesh.vertices[tri[i] as usize].uv);
                let signed_area = (uv[1][0] - uv[0][0]) * (uv[2][1] - uv[0][1])
                    - (uv[1][1] - uv[0][1]) * (uv[2][0] - uv[0][0]);
                assert_eq!(signed_area, -1.0); // Both triangles have identical UV orientation.
            }
        }
    }

    #[test]
    fn platform_indices_never_cross_a_face_or_block() {
        let mesh = Stage::Platform.mesh();
        assert_eq!(mesh.vertices.len(), 9 * 24);
        for (face, indices) in mesh.indices.as_chunks::<6>().0.iter().enumerate() {
            assert!(indices.iter().all(|&i| i as usize / 4 == face));
        }
        for v in mesh.vertices {
            assert!((-1.0..=2.0).contains(&v.position[0]));
            assert!((-1.0..=2.0).contains(&v.position[2]));
            assert!((0.0..=1.0).contains(&v.position[1]));
        }
    }
}

//! Small project-owned geometry catalog; no Minecraft or private renderer policy.
use super::*;
use rustcraft_engine_core::{orientation::Facing, shape::*};
use rustcraft_game_api::{ModelKey, ShapeKey, geometry::*, state_schema::*};
fn box_part(min: [f32; 3], max: [f32; 3]) -> Primitive {
    Primitive::Box(LocalBox { min, max })
}
fn model(name: &str) -> ModelKey {
    ModelKey::parse(format!("sandbox_test:model/{name}")).unwrap()
}
fn shape(name: &str) -> ShapeKey {
    ShapeKey::parse(format!("sandbox_test:shape/{name}")).unwrap()
}
fn roles(name: &str) -> GeometryRoles {
    GeometryRoles {
        model: model(name),
        collision: shape(name),
        selection: shape(name),
        transform: ModelRotation::IDENTITY,
        light: LightCoverage::Partial,
    }
}
fn field(name: &str, domain: StateDomain, default: StateValue) -> StateField {
    StateField {
        key: id(&format!("sandbox_test:state/{name}")),
        domain,
        default,
    }
}
fn assignment(name: &str, value: StateValue) -> StateAssignment {
    StateAssignment {
        key: id(&format!("sandbox_test:state/{name}")),
        value,
    }
}
fn register_geometry(
    registry: &mut GameRegistry,
    name: &str,
    render: RenderGeometry,
    collision: Shape,
    selection: Option<Shape>,
) -> Result<(), RegistrationError> {
    registry.register_model(ModelDefinition {
        key: model(name),
        geometry: render,
    })?;
    registry.register_shape(ShapeDefinition {
        key: shape(name),
        shape: collision,
    })?;
    if let Some(selection) = selection {
        registry.register_shape(ShapeDefinition {
            key: shape(&format!("{name}_selection")),
            shape: selection,
        })?;
    }
    Ok(())
}
fn primitive_geometry(
    registry: &mut GameRegistry,
    name: &str,
    parts: Vec<Primitive>,
) -> Result<(), RegistrationError> {
    register_geometry(
        registry,
        name,
        RenderGeometry::Primitives(parts.clone()),
        Shape::Compound(parts),
        None,
    )
}
fn register_voxel(
    registry: &mut GameRegistry,
    name: &str,
    fields: Vec<StateField>,
    binding: GeometryBinding,
) -> Result<(), RegistrationError> {
    registry.register_block(VoxelDefinition {
        geometry: Some(binding),
        common: rustcraft_game_api::ContentDefinition::new(id(&format!(
            "sandbox_test:block/{name}"
        ))),
        state_schema: Some(StateSchema {
            fields,
            ..Default::default()
        }),
        collision: CollisionDescriptor::FullCube,
        targetable: true,
        material: MaterialClass::Opaque,
        textures: FaceResources::All(texture_key("sandbox_test:textures/block/foundation")),
        light: LightDescriptor::default(),
        base_rotation: ModelRotation::IDENTITY,
        orientation: OrientationProperty::None,
        face_tints: [[u16::MAX; 3]; 6],
    })
}
pub fn register(registry: &mut GameRegistry) -> Result<(), RegistrationError> {
    register_geometry(
        registry,
        "unit",
        RenderGeometry::FullCube,
        Shape::FullCube,
        None,
    )?;
    primitive_geometry(registry, "lower", vec![box_part([0.; 3], [1., 0.5, 1.])])?;
    primitive_geometry(registry, "upper", vec![box_part([0., 0.5, 0.], [1.; 3])])?;
    register_voxel(
        registry,
        "plate",
        vec![field(
            "half",
            StateDomain::Half,
            StateValue::Half(Half::Lower),
        )],
        GeometryBinding {
            default: roles("lower"),
            rules: vec![GeometryRule {
                when: vec![assignment("half", StateValue::Half(Half::Upper))],
                roles: roles("upper"),
            }],
        },
    )?;
    for (name, parts) in [
        (
            "straight",
            vec![
                box_part([0.; 3], [1., 0.5, 1.]),
                box_part([0., 0.5, 0.5], [1.; 3]),
            ],
        ),
        (
            "inner",
            vec![
                box_part([0.; 3], [1., 0.5, 1.]),
                box_part([0., 0.5, 0.5], [1.; 3]),
                box_part([0.5, 0.5, 0.], [1., 1., 0.5]),
            ],
        ),
        (
            "outer",
            vec![
                box_part([0.; 3], [1., 0.5, 1.]),
                box_part([0.5, 0.5, 0.5], [1.; 3]),
            ],
        ),
    ] {
        primitive_geometry(registry, name, parts)?;
    }
    let names = ["straight", "inner", "outer"];
    register_voxel(
        registry,
        "assembly",
        vec![
            field(
                "facing",
                StateDomain::Facing,
                StateValue::Facing(Facing::North),
            ),
            field(
                "shape",
                StateDomain::Shape(
                    names
                        .map(|n| id(&format!("sandbox_test:form/{n}")))
                        .to_vec(),
                ),
                StateValue::Shape(id("sandbox_test:form/straight")),
            ),
        ],
        GeometryBinding {
            default: roles("straight"),
            rules: ["inner", "outer"]
                .into_iter()
                .map(|name| GeometryRule {
                    when: vec![assignment(
                        "shape",
                        StateValue::Shape(id(&format!("sandbox_test:form/{name}"))),
                    )],
                    roles: roles(name),
                })
                .collect(),
        },
    )?;
    primitive_geometry(registry, "incline", vec![Primitive::Wedge(LocalBox::UNIT)])?;
    register_voxel(
        registry,
        "incline",
        vec![field(
            "facing",
            StateDomain::Facing,
            StateValue::Facing(Facing::North),
        )],
        GeometryBinding {
            default: roles("incline"),
            rules: vec![],
        },
    )?;
    for mask in 0..4 {
        let mut parts = vec![box_part([0.375, 0., 0.375], [0.625, 1., 0.625])];
        if mask & 1 != 0 {
            parts.push(box_part([0., 0.375, 0.375], [0.375, 0.625, 0.625]));
        }
        if mask & 2 != 0 {
            parts.push(box_part([0.625, 0.375, 0.375], [1., 0.625, 0.625]));
        }
        primitive_geometry(registry, &format!("frame_{mask}"), parts)?;
    }
    register_voxel(
        registry,
        "frame",
        vec![field(
            "connections",
            StateDomain::Connections { mask: 3 },
            StateValue::Connections(0),
        )],
        GeometryBinding {
            default: roles("frame_0"),
            rules: (1..4)
                .map(|mask| GeometryRule {
                    when: vec![assignment("connections", StateValue::Connections(mask))],
                    roles: roles(&format!("frame_{mask}")),
                })
                .collect(),
        },
    )?;
    let v = [
        [0.15, 0., 0.15],
        [0.85, 0., 0.15],
        [0.5, 0., 0.85],
        [0.5, 1., 0.5],
    ];
    let center = [0.5, 0.25, 0.4125];
    let mesh: Vec<_> = [[0, 1, 2], [0, 3, 1], [1, 3, 2], [2, 3, 0]]
        .into_iter()
        .map(|ix| {
            let mut t = Triangle {
                positions: ix.map(|i| v[i]),
                uv: [[0., 1.], [0.5, 0.], [1., 1.]],
                face: 4,
            };
            if dot(t.normal(), sub(t.positions[0], center)) < 0. {
                t.positions.swap(1, 2);
                t.uv.swap(1, 2);
            }
            t
        })
        .collect();
    register_geometry(
        registry,
        "mesh",
        RenderGeometry::StaticMesh(BoundedStaticMesh::from_triangles(&mesh).unwrap()),
        Shape::Compound(vec![box_part([0.45, 0., 0.35], [0.55, 0.4, 0.5])]),
        Some(Shape::Surface(mesh)),
    )?;
    let mut mesh_roles = roles("mesh");
    mesh_roles.selection = shape("mesh_selection");
    register_voxel(
        registry,
        "mesh",
        vec![],
        GeometryBinding {
            default: mesh_roles,
            rules: vec![],
        },
    )?;
    Ok(())
}
pub fn reactor_binding() -> GeometryBinding {
    let mut default = roles("unit");
    default.light = LightCoverage::Full;
    GeometryBinding {
        default,
        rules: vec![GeometryRule {
            when: vec![StateAssignment {
                key: id("voxel_std:state/powered"),
                value: StateValue::Powered(true),
            }],
            roles: roles("frame_3"),
        }],
    }
}
pub fn add_scene(world: &mut World, profile: &CompiledGameProfile) {
    for (x, name) in [
        (0, "plate"),
        (2, "assembly"),
        (4, "incline"),
        (6, "frame"),
        (8, "mesh"),
    ] {
        let block = profile
            .block_id(&block_key(&format!("sandbox_test:block/{name}")))
            .unwrap();
        world.set(BlockPos { x, y: 1, z: 3 }, block);
    }
}

pub fn render_proof_scene(
    profile: &CompiledGameProfile,
    materials: &CompiledVoxelRenderRegistry,
    resources: &rustcraft_content::resources::CompiledResources,
) {
    let mut world = World::new(profile.default_state().block);
    let specs = [
        ("foundation", vec![]),
        ("plate", vec![]),
        (
            "plate",
            vec![assignment("half", StateValue::Half(Half::Upper))],
        ),
        ("mesh", vec![]),
        (
            "incline",
            vec![assignment("facing", StateValue::Facing(Facing::North))],
        ),
        (
            "incline",
            vec![assignment("facing", StateValue::Facing(Facing::East))],
        ),
        (
            "incline",
            vec![assignment("facing", StateValue::Facing(Facing::South))],
        ),
        (
            "incline",
            vec![assignment("facing", StateValue::Facing(Facing::West))],
        ),
        ("assembly", vec![]),
        (
            "assembly",
            vec![assignment(
                "shape",
                StateValue::Shape(id("sandbox_test:form/inner")),
            )],
        ),
        (
            "assembly",
            vec![assignment(
                "shape",
                StateValue::Shape(id("sandbox_test:form/outer")),
            )],
        ),
        (
            "frame",
            vec![assignment("connections", StateValue::Connections(3))],
        ),
    ];
    for (index, (name, overrides)) in specs.into_iter().enumerate() {
        let block = profile
            .block_id(&block_key(&format!("sandbox_test:block/{name}")))
            .unwrap();
        let schema = &profile.block(block).unwrap().state_schema;
        let variant = if schema.is_legacy() {
            0
        } else {
            let mut values = schema.decode(0).unwrap();
            for a in overrides {
                let slot = values.iter().position(|v| v.key == a.key).unwrap();
                values[slot] = a;
            }
            schema.encode(&values).unwrap()
        };
        world.set_state(
            BlockPos {
                x: (index % 4) as i32 * 4,
                y: 0,
                z: (index / 4) as i32 * 4,
            },
            BlockState { block, variant },
        );
    }
    let presentation = RenderWorld::from_world(&world);
    let mut pages = Vec::new();
    for chunk in presentation.chunks() {
        for page in rustcraft_render::build_chunk_mesh_pages(&presentation, chunk, materials) {
            let vertices = page
                .mesh
                .indices
                .into_iter()
                .map(|i| {
                    let mut v = page.mesh.vertices[i as usize];
                    let [x, y, z] = v.position;
                    v.position = [
                        (x - z - 2.) * 0.08,
                        y * 0.20 - (x + z - 10.) * 0.045,
                        0.5 - (x + y + z) * 0.008,
                    ];
                    v.shade = (v.shade * 20.).min(1.);
                    v
                })
                .collect();
            pages.push(rustcraft_render::PageVertices {
                texture: page.texture,
                vertices,
            });
        }
    }
    let atlas: Vec<_> = resources
        .pages
        .iter()
        .map(|p| rustcraft_render::RgbaTexture {
            width: p.width,
            height: p.height,
            rgba: p.rgba.clone(),
            sampler: rustcraft_render::TextureSampling::Nearest,
        })
        .collect();
    let scene = Scene {
        vertices: Vec::new(),
        width: 960,
        height: 640,
        textured: true,
        cull: true,
    };
    pollster::block_on(offscreen::render_pages_rgba(
        &scene,
        &pages,
        &atlas,
        Path::new("target/sample-game/bg1-geometry.png"),
    ))
    .expect("render project-owned BG1 geometry proof");
    println!(
        "BG1_GEOMETRY 12 state-selected objects; outward culling; atlas pages; output=target/sample-game/bg1-geometry.png"
    );
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    #[test]
    fn local_geometry_handles_change_without_semantic_state_or_physics_change() {
        let mut registry = GameRegistry::default();
        SandboxPackage.register(&mut registry).unwrap();
        SandboxModPackage.register(&mut registry).unwrap();
        let authored = authored_profile(vec![]);
        let a = registry.compile(&authored).unwrap();
        registry
            .register_model(ModelDefinition {
                key: ModelKey::parse("aaa:model/extra").unwrap(),
                geometry: RenderGeometry::FullCube,
            })
            .unwrap();
        registry
            .register_shape(ShapeDefinition {
                key: ShapeKey::parse("aaa:shape/extra").unwrap(),
                shape: Shape::Empty,
            })
            .unwrap();
        let mut reordered = authored;
        reordered.packages.reverse();
        let b = registry.compile(&reordered).unwrap();
        let block = a
            .block_id(&block_key("sandbox_test:block/incline"))
            .unwrap();
        let state = BlockState::new(block);
        let (key, record) = a.export_state(state).unwrap();
        let restored = b.import_state(&key, &record).unwrap();
        let ga = a.geometry_for_state(state).unwrap().unwrap();
        let gb = b.geometry_for_state(restored).unwrap().unwrap();
        assert_ne!(ga.model, gb.model);
        assert_ne!(ga.collision, gb.collision);
        assert_eq!(
            a.geometry().models[ga.model.0 as usize].key,
            b.geometry().models[gb.model.0 as usize].key
        );
        assert_eq!(
            a.export_state(state).unwrap(),
            b.export_state(restored).unwrap()
        );
        assert_eq!(a.semantic_fingerprint(), b.semantic_fingerprint());
        assert_eq!(
            a.selection_hit(state, [0.5, 2., 0.5], [0., -1., 0.], 3.)
                .unwrap(),
            b.selection_hit(restored, [0.5, 2., 0.5], [0., -1., 0.], 3.)
                .unwrap()
        );
        assert_eq!(std::mem::size_of::<BlockState>(), 8);
        assert_eq!(std::mem::size_of::<ModelHandle>(), 4);
        assert_eq!(std::mem::size_of::<ShapeHandle>(), 4);
        println!(
            "BG1 resolved_entry_bytes={} state_bytes=8 handles=4",
            std::mem::size_of::<Option<ResolvedGeometry>>()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustcraft_render::BlockTextureResolver;
    fn state(profile: &CompiledGameProfile, name: &str, values: &[StateAssignment]) -> BlockState {
        let block = profile
            .block_id(&block_key(&format!("sandbox_test:block/{name}")))
            .unwrap();
        let schema = &profile.block(block).unwrap().state_schema;
        let mut full = schema.decode(0).unwrap();
        for value in values {
            *full.iter_mut().find(|a| a.key == value.key).unwrap() = value.clone();
        }
        BlockState {
            block,
            variant: schema.encode(&full).unwrap(),
        }
    }
    #[test]
    fn actual_model_collision_selection_consume_half_shape_facing_connections() {
        let profile = compile_profile(vec![]);
        let (_, render) = compile_render_resources(&profile);
        let low = state(&profile, "plate", &[]);
        let high = state(
            &profile,
            "plate",
            &[assignment("half", StateValue::Half(Half::Upper))],
        );
        let query = LocalBox {
            min: [0.1, 0.1, 0.1],
            max: [0.2, 0.2, 0.2],
        };
        assert!(profile.overlaps_state(low, query).unwrap());
        assert!(!profile.overlaps_state(high, query).unwrap());
        let selection = |s| {
            profile
                .selection_hit(s, [0.5, 2., 0.5], [0., -1., 0.], 3.)
                .unwrap()
                .unwrap()
                .distance
        };
        assert_eq!(selection(low), 1.5);
        assert_eq!(selection(high), 1.);
        assert_ne!(
            render.block_model(low).unwrap().triangles,
            render.block_model(high).unwrap().triangles
        );
        for shape_name in ["straight", "inner", "outer"] {
            for facing in [Facing::North, Facing::East, Facing::Up] {
                let s = state(
                    &profile,
                    "assembly",
                    &[
                        assignment(
                            "shape",
                            StateValue::Shape(id(&format!("sandbox_test:form/{shape_name}"))),
                        ),
                        assignment("facing", StateValue::Facing(facing)),
                    ],
                );
                let g = profile.geometry_for_state(s).unwrap().unwrap();
                assert_eq!(render.model_rotation(s), g.transform);
                let origin = g.transform.point([0.75, 2., 0.75]);
                let direction = g.transform.transform([0., -1., 0.]);
                assert_eq!(
                    profile
                        .selection_hit(s, origin, direction, 3.)
                        .unwrap()
                        .unwrap()
                        .distance,
                    1.
                );
                assert!(
                    profile
                        .overlaps_state(
                            s,
                            LocalBox {
                                min: [0.45; 3],
                                max: [0.55; 3]
                            }
                        )
                        .unwrap()
                );
            }
        }
        for mask in 0..4 {
            let s = state(
                &profile,
                "frame",
                &[assignment("connections", StateValue::Connections(mask))],
            );
            let query = LocalBox {
                min: [0.1, 0.45, 0.45],
                max: [0.2, 0.55, 0.55],
            };
            assert_eq!(profile.overlaps_state(s, query).unwrap(), mask & 1 != 0);
            let hit = profile
                .selection_hit(s, [-1., 0.5, 0.5], [1., 0., 0.], 3.)
                .unwrap()
                .unwrap();
            assert_eq!(hit.distance, if mask & 1 != 0 { 1. } else { 1.375 });
        }
    }
    #[test]
    fn dda_wedge_gap_and_boundary_geometry() {
        let profile = compile_profile(vec![]);
        let (_, render) = compile_render_resources(&profile);
        for x in [-17, -16, -1, 0, 15, 16] {
            for facing in [
                Facing::North,
                Facing::East,
                Facing::South,
                Facing::West,
                Facing::Up,
                Facing::Down,
            ] {
                let mut world = World::new(profile.default_state().block);
                let state = state(
                    &profile,
                    "incline",
                    &[assignment("facing", StateValue::Facing(facing))],
                );
                let g = profile.geometry_for_state(state).unwrap().unwrap();
                let pos = BlockPos { x, y: 16, z: 0 };
                world.set_state(pos, state);
                let o = g.transform.point([0.5, 2., 0.5]);
                let d = g.transform.transform([0., -1., 0.]);
                let hit = rustcraft_engine_core::raycast::cast_shapes(
                    &world,
                    Vec3::new(o[0] + x as f32, o[1] + 16., o[2]),
                    Vec3::new(d[0], d[1], d[2]),
                    3.,
                    |s, o, d, r| profile.selection_hit(s, o, d, r).unwrap(),
                )
                .unwrap();
                assert_eq!(hit.block, pos);
                assert_eq!(hit.distance, 1.5);
                assert_eq!(hit.local_point, [0.5; 3]);
                assert_eq!(
                    hit.adjacent,
                    BlockPos {
                        x: pos.x + hit.normal[0],
                        y: pos.y + hit.normal[1],
                        z: pos.z + hit.normal[2]
                    }
                );
                for face in rustcraft_render::geometry::FACES {
                    let local = g.transform.inverse().transform(face.normal);
                    assert_eq!(
                        render.covers_face(state, face.direction),
                        local == [0., 0., 1.] || local == [0., -1., 0.]
                    );
                }
                let presentation = RenderWorld::from_world(&world);
                assert_eq!(
                    presentation
                        .chunks()
                        .map(|c| build_chunk_mesh(&presentation, c, &render).indices.len())
                        .sum::<usize>(),
                    24
                );
                let a = g.transform.point([0.1, 0.7, 0.1]);
                let b = g.transform.point([0.2, 0.9, 0.2]);
                let min: [f32; 3] = std::array::from_fn(|i| a[i].min(b[i]));
                let max: [f32; 3] = std::array::from_fn(|i| a[i].max(b[i]));
                assert!(!world.collides_shapes(
                    rustcraft_engine_core::Aabb::new(
                        Vec3::new(min[0] + x as f32, min[1] + 16., min[2]),
                        Vec3::new(max[0] + x as f32, max[1] + 16., max[2])
                    ),
                    |s, b| profile.overlaps_state(s, b).unwrap()
                ));
            }
        }
    }
    #[test]
    fn partial_neighbor_keeps_exposed_cube_face_and_static_mesh_is_independent() {
        let profile = compile_profile(vec![]);
        let (_, render) = compile_render_resources(&profile);
        let mut world = World::new(profile.default_state().block);
        let cube = profile
            .block_id(&block_key("sandbox_test:block/foundation"))
            .unwrap();
        world.set(BlockPos { x: 15, y: 0, z: 0 }, cube);
        world.set_state(
            BlockPos { x: 16, y: 0, z: 0 },
            state(&profile, "plate", &[]),
        );
        let presentation = RenderWorld::from_world(&world);
        let left = presentation.chunks().find(|c| c.position.x == 0).unwrap();
        assert_eq!(
            build_chunk_mesh(&presentation, left, &render).indices.len(),
            36
        );
        world.set_state(
            BlockPos { x: 15, y: 0, z: 0 },
            state(&profile, "plate", &[]),
        );
        let paired = RenderWorld::from_world(&world);
        let indices: usize = paired
            .chunks()
            .map(|c| build_chunk_mesh(&paired, c, &render).indices.len())
            .sum();
        assert_eq!(
            indices, 60,
            "opposing covered partial faces must be culled across the chunk boundary"
        );
        world.set_state(
            BlockPos { x: 16, y: 0, z: 0 },
            state(
                &profile,
                "plate",
                &[assignment("half", StateValue::Half(Half::Upper))],
            ),
        );
        let opposite = RenderWorld::from_world(&world);
        assert_eq!(
            opposite
                .chunks()
                .map(|c| build_chunk_mesh(&opposite, c, &render).indices.len())
                .sum::<usize>(),
            72,
            "nonoverlapping partial masks must preserve both surfaces"
        );
        let mesh = state(&profile, "mesh", &[]);
        let model = render.block_model(mesh).unwrap();
        assert_eq!(model.triangles.as_ref().unwrap().len(), 4);
        assert_eq!(
            profile
                .selection_hit(mesh, [0.5, 2., 0.5], [0., -1., 0.], 3.)
                .unwrap()
                .unwrap()
                .distance,
            1.
        );
        let mut vertices = Vec::new();
        model.world_vertices(&mut vertices);
        assert_eq!(vertices.len(), 12);
        let mut pages = Vec::new();
        model.gui_page_vertices(&mut pages, [20., 20.], 10., [100., 100.]);
        assert_eq!(pages.iter().map(|p| p.vertices.len()).sum::<usize>(), 12);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    #[test]
    fn powered_selects_real_geometry_and_actor_motion_lands_on_slope() {
        let profile = compile_profile(vec![]);
        let (_, render) = compile_render_resources(&profile);
        let reactor = profile
            .block_id(&block_key("sandbox_mod:block/reactor"))
            .unwrap();
        let schema = &profile.block(reactor).unwrap().state_schema;
        let off = BlockState::new(reactor);
        let mut assignments = schema.decode(0).unwrap();
        assignments
            .iter_mut()
            .find(|a| a.key == id("voxel_std:state/powered"))
            .unwrap()
            .value = StateValue::Powered(true);
        let on = BlockState {
            block: reactor,
            variant: schema.encode(&assignments).unwrap(),
        };
        assert_eq!(profile.light_for_state(off).unwrap().sky_opacity, 15);
        assert_eq!(profile.light_for_state(on).unwrap().sky_opacity, 8);
        assert!(render.block_model(off).unwrap().triangles.is_none());
        assert!(render.block_model(on).unwrap().triangles.is_some());
        let query = LocalBox {
            min: [0.1, 0.1, 0.1],
            max: [0.2, 0.2, 0.2],
        };
        assert!(profile.overlaps_state(off, query).unwrap());
        assert!(!profile.overlaps_state(on, query).unwrap());
        assert!(
            profile
                .selection_hit(off, [0.1, 0.1, -1.], [0., 0., 1.], 3.)
                .unwrap()
                .is_some()
        );
        assert!(
            profile
                .selection_hit(on, [0.1, 0.1, -1.], [0., 0., 1.], 3.)
                .unwrap()
                .is_none()
        );
        let incline = profile
            .block_id(&block_key("sandbox_test:block/incline"))
            .unwrap();
        let mut world = World::new(profile.default_state().block);
        world.set(BlockPos { x: 0, y: 0, z: 0 }, incline);
        let actor =
            rustcraft_engine_core::Aabb::new(Vec3::new(0.4, 2., 0.4), Vec3::new(0.6, 3., 0.6));
        let (landed, _) = world.move_and_collide_shapes(actor, Vec3::new(0., -2., 0.), |s, b| {
            profile.overlaps_state(s, b).unwrap()
        });
        assert!((landed.min.y - 0.6).abs() < 0.001);
        assert!(!world.collides_shapes(landed, |s, b| profile.overlaps_state(s, b).unwrap()));
    }
}

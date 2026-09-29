//! Semantic content adapter for the reusable, surface-independent render inspector.
use super::{FirstPartyTextures, LocalResourceResolver, ResourceId};
use rustcraft_engine_core::orientation::{Axis, Facing, HorizontalRotation, ModelRotation};
use rustcraft_engine_core::{BlockState, Vec3};
use rustcraft_render::{
    BlockTextureResolver, Face, ItemSprite, Vertex, geometry,
    hud::{self, Slot},
    offscreen::{self, Scene},
};
use std::path::Path;
const FACES: [(&str, Face); 6] = [
    ("pos-x", Face::East),
    ("neg-x", Face::West),
    ("pos-y", Face::Top),
    ("neg-y", Face::Bottom),
    ("pos-z", Face::North),
    ("neg-z", Face::South),
];
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}
fn project(p: [f32; 3]) -> [f32; 3] {
    // Fixed orthographic inspection camera, looking at origin from +X,+Y,+Z.
    let q = p;
    let x = (q[0] - q[2]) * std::f32::consts::FRAC_1_SQRT_2;
    let y = (-q[0] + 2. * q[1] - q[2]) / 6f32.sqrt();
    [x * 1.1, y * 1.1, 0.5 - (q[0] + q[1] + q[2]) * 0.1]
}
fn line(out: &mut Vec<Vertex>, a: [f32; 3], b: [f32; 3], color: [f32; 3]) {
    let d = [b[0] - a[0], b[1] - a[1]];
    let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(0.001);
    let n = [-d[1] / l * 0.003, d[0] / l * 0.003];
    let p = [
        [a[0] + n[0], a[1] + n[1]],
        [b[0] + n[0], b[1] + n[1]],
        [b[0] - n[0], b[1] - n[1]],
        [a[0] - n[0], a[1] - n[1]],
    ];
    for i in [0, 2, 1, 0, 3, 2] {
        out.push(Vertex {
            position: [p[i][0], p[i][1], 0.001],
            uv: [0.; 2],
            shade: -1.,
            color,
        });
    }
}
#[derive(Clone, Copy)]
struct Options {
    rotation: ModelRotation,
    facing: Facing,
    axis: Axis,
    horizontal: HorizontalRotation,
    age: f32,
    count: u16,
    rear: bool,
    native: bool,
}
fn scene(name: &str, mode: &str, wire: bool, options: &Options) -> Result<Scene, String> {
    let colors = [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [1., 1., 0.]];
    let mut vertices = Vec::new();
    let mut guides = Vec::new();
    if name == "gui-grid" {
        for (row, label) in ["HOTBAR", "INVENTORY", "CURSOR"].into_iter().enumerate() {
            vertices.extend(hud::diagnostic_label(
                label,
                [4., 10. + row as f32 * 80.],
                [512., 256.],
            ));
            for (column, name) in [
                "bookshelf",
                "log",
                "grass",
                "stone",
                "cobblestone",
                "planks",
            ]
            .into_iter()
            .enumerate()
            {
                let block = rustcraft_minecraft_b173::blocks::BLOCKS
                    .iter()
                    .find(|b| b.name.strip_prefix("minecraft_b173:") == Some(name))
                    .unwrap();
                let model = rustcraft_render::inspection::BlockModel::resolve(
                    BlockState::new(block.id),
                    &FirstPartyTextures,
                )
                .unwrap();
                let origin = [10. + column as f32 * 84., 26. + row as f32 * 80.];
                model.gui_vertices(&mut vertices, origin, 2., [512., 256.]);
                vertices.extend(hud::diagnostic_label(
                    name,
                    [origin[0], origin[1] + 36.],
                    [512., 256.],
                ));
            }
        }
        return Ok(Scene {
            vertices,
            width: 512,
            height: 256,
            textured: true,
            cull: true,
        });
    }
    if let Some(face) = name.strip_prefix("face-") {
        let direction = FACES
            .iter()
            .find(|(n, _)| *n == face)
            .ok_or("unknown face")?
            .1;
        let f = geometry::definition(direction);
        let p = f.positions;
        guides.push((direction, [0., 0., 0.5], [0., 0., 0.2]));
        let u = sub(p[1], p[0]);
        let v = sub(p[3], p[0]);
        for j in f.indices {
            let q = sub(p[j], p[0]);
            vertices.push(Vertex {
                position: [(dot(q, u) - 0.5) * 1.5, (dot(q, v) - 0.5) * 1.5, 0.5],
                uv: if mode == "atlas" {
                    [
                        (3. + f.uv_corners[j][0]) / 16.,
                        (2. + f.uv_corners[j][1]) / 16.,
                    ]
                } else {
                    f.uv_corners[j]
                },
                shade: 1.,
                color: if mode == "corners" {
                    colors[j]
                } else {
                    [1.; 3]
                },
            });
        }
    } else {
        let (context, block) = name.split_once('-').ok_or("expected context-block")?;
        let block = rustcraft_minecraft_b173::blocks::BLOCKS
            .iter()
            .find(|b| b.name.strip_prefix("minecraft_b173:") == Some(block))
            .ok_or("unknown semantic block")?;
        let r = FirstPartyTextures;
        let state = BlockState::new(block.id)
            .with_facing(options.facing)
            .with_axis(options.axis)
            .with_rotation(options.horizontal);
        let mut model =
            rustcraft_render::inspection::BlockModel::resolve(state, &r).ok_or("block model")?;
        model.rotation = block.base_model_rotation.compose(options.rotation);
        let view = |p: [f32; 3]| {
            project(if options.rear {
                [-p[0], p[1], -p[2]]
            } else {
                p
            })
        };
        let slot = Slot {
            model: Some(model),
            top: r.texture(block.id, Face::Top).ok_or("top texture")?,
            side: r.texture(block.id, Face::North).ok_or("side texture")?,
            bottom: r.texture(block.id, Face::Bottom).ok_or("bottom texture")?,
            tint: [1.; 3],
            count: 1,
            block_3d: true,
        };
        match context {
            "gui" | "hotbar" | "inventory" | "cursor" => model.gui_vertices(
                &mut vertices,
                if options.native {
                    [120., 120.]
                } else {
                    [0., 0.]
                },
                if options.native { 1. } else { 16. },
                [256., 256.],
            ),
            "dropped" => {
                rustcraft_render::append_dropped_items(
                    &mut vertices,
                    &[ItemSprite {
                        model: Some(model),
                        position: Vec3::ZERO,
                        top: slot.top,
                        side: slot.side,
                        bottom: slot.bottom,
                        tint: [1.; 3],
                        age: options.age,
                        count: options.count,
                        hover_start: 0.,
                    }],
                );
                for v in &mut vertices {
                    v.position = view([
                        v.position[0] * 4.,
                        (v.position[1] - 0.1) * 4.,
                        v.position[2] * 4.,
                    ]);
                }
            }
            "cube" => {
                model.world_vertices(&mut vertices);
                for v in &mut vertices {
                    v.position = view(v.position.map(|v| v - 0.5));
                }
            }
            _ => return Err("unknown representation".into()),
        }
        for f in geometry::FACES {
            let project_normal = |distance: f32| {
                let p = std::array::from_fn(|i| 0.5 + f.normal[i] * distance);
                let p = model.rotation.point(p);
                if matches!(context, "gui" | "inventory" | "hotbar" | "cursor") {
                    let p = hud::beta_gui_project_point(p);
                    let (origin, scale) = if options.native {
                        (120., 1.)
                    } else {
                        (0., 16.)
                    };
                    [
                        (origin + p[0] * scale) / 128. - 1.,
                        1. - (origin + p[1] * scale) / 128.,
                        0.5 - p[2] * 0.01,
                    ]
                } else {
                    let p = p.map(|v| v - 0.5);
                    if context == "dropped" {
                        let (s, c) = rustcraft_render::beta_item_rotation_degrees(options.age, 0.)
                            .to_radians()
                            .sin_cos();
                        view([
                            p[0] * c - p[2] * s,
                            p[1] + 4. * (rustcraft_render::beta_item_bob(options.age, 0.) - 0.1),
                            p[0] * s + p[2] * c,
                        ])
                    } else {
                        view(p)
                    }
                }
            };
            guides.push((f.direction, project_normal(0.5), project_normal(0.75)));
        }
        if mode != "atlas" {
            for (i, v) in vertices.iter_mut().enumerate() {
                v.uv = geometry::FACES[(i / 6) % 6].uv_corners[[0, 1, 2, 0, 2, 3][i % 6]];
                v.color = if mode == "corners" {
                    colors[[0, 1, 2, 0, 2, 3][i % 6]]
                } else {
                    [1.; 3]
                };
                v.shade = 1.;
            }
        }
    }
    if wire {
        let triangles = vertices.clone();
        for tri in triangles.as_chunks::<3>().0 {
            for i in 0..3 {
                line(
                    &mut vertices,
                    tri[i].position,
                    tri[(i + 1) % 3].position,
                    [1., 0., 1.],
                );
                let p = tri[i].position;
                line(
                    &mut vertices,
                    [p[0] - 0.01, p[1], 0.],
                    [p[0] + 0.01, p[1], 0.],
                    [1., 1., 0.],
                );
            }
        }
        for (direction, center, tip) in guides {
            line(&mut vertices, center, tip, [0., 1., 1.]);
            let label = FACES.iter().find(|(_, f)| *f == direction).unwrap().0;
            vertices.extend(hud::diagnostic_label(
                label,
                [(tip[0] + 1.) * 128., (1. - tip[1]) * 128.],
                [256., 256.],
            ));
        }
    }
    Ok(Scene {
        vertices,
        width: 256,
        height: 256,
        textured: mode == "uv" || mode == "atlas",
        cull: true,
    })
}
pub fn run() -> bool {
    let args: Vec<_> = std::env::args().collect();
    let all = args
        .iter()
        .any(|a| a == "--render-test-all" || a == "--fidelity-m3");
    let flag = args.iter().position(|a| a == "--render-test");
    if !all && flag.is_none() {
        return false;
    }
    let mut cases = Vec::new();
    if all {
        for (name, _) in FACES {
            for mode in ["solid", "corners", "uv", "atlas"] {
                cases.push((format!("face-{name}"), mode.to_string()));
            }
        }
        for block in [
            "bookshelf",
            "log",
            "grass",
            "stone",
            "cobblestone",
            "planks",
        ] {
            for context in ["cube", "dropped", "gui", "hotbar", "inventory", "cursor"] {
                cases.push((format!("{context}-{block}"), "atlas".into()));
            }
        }
        cases.push(("gui-bookshelf".into(), "corners".into()));
        cases.push(("gui-grid".into(), "atlas".into()));
    } else {
        let name = args
            .get(flag.unwrap() + 1)
            .expect("render-test SCENE")
            .clone();
        let mode = args
            .iter()
            .position(|a| a == "--mode")
            .and_then(|i| args.get(i + 1))
            .map_or(
                if name.starts_with("face-") {
                    "corners"
                } else {
                    "atlas"
                },
                String::as_str,
            );
        assert!(
            ["corners", "solid", "uv", "atlas"].contains(&mode),
            "unknown mode"
        );
        cases.push((name, mode.into()));
    }
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .map(String::as_str)
    };
    let horizontal = |s: &str| match s {
        "0" => HorizontalRotation::R0,
        "90" => HorizontalRotation::R90,
        "180" => HorizontalRotation::R180,
        "270" => HorizontalRotation::R270,
        _ => panic!("rotation must be 0/90/180/270"),
    };
    let facing = match value("--facing").unwrap_or("north") {
        "north" => Facing::North,
        "south" => Facing::South,
        "east" => Facing::East,
        "west" => Facing::West,
        "up" => Facing::Up,
        "down" => Facing::Down,
        _ => panic!("invalid facing"),
    };
    let axis = match value("--axis").unwrap_or("y") {
        "x" => Axis::X,
        "y" => Axis::Y,
        "z" => Axis::Z,
        _ => panic!("invalid axis"),
    };
    let turn = horizontal(value("--rotation").unwrap_or("0"));
    let options = Options {
        rotation: ModelRotation::horizontal(horizontal(value("--base-rotation").unwrap_or("0")))
            .compose(ModelRotation::facing(facing))
            .compose(ModelRotation::axis(axis))
            .compose(ModelRotation::horizontal(turn)),
        facing,
        axis,
        horizontal: turn,
        age: value("--item-age")
            .unwrap_or("0")
            .parse()
            .expect("numeric age"),
        count: value("--count")
            .unwrap_or("1")
            .parse()
            .expect("stack count"),
        rear: args.iter().any(|a| a == "--rear"),
        native: args.iter().any(|a| a == "--native"),
    };
    let atlas = LocalResourceResolver::from_environment("reference/assets/terrain.png")
        .resolve(&ResourceId("terrain.png".into()));
    for (name, mode) in cases {
        let mut variants = vec![options];
        if all && (name.starts_with("cube-") || name.starts_with("dropped-")) {
            variants.push(Options {
                rear: true,
                ..options
            });
        }
        if all
            && ["gui-", "inventory-", "hotbar-", "cursor-"]
                .iter()
                .any(|p| name.starts_with(p))
            && name != "gui-grid"
        {
            variants.push(Options {
                native: true,
                ..options
            });
        }
        for options in variants {
            let wire = args.iter().any(|a| a == "--wireframe");
            let mut scene = scene(&name, &mode, wire, &options).expect("diagnostic scene");
            scene.cull = !args.iter().any(|a| a == "--no-cull");
            let state_suffix = if options.rotation != ModelRotation::IDENTITY {
                format!(
                    "-{:?}-{:?}-{:?}-base{}",
                    options.facing,
                    options.axis,
                    options.horizontal,
                    value("--base-rotation").unwrap_or("0")
                )
            } else {
                String::new()
            };
            let path = format!(
                "target/render-tests/{name}-{mode}{}{}{}{state_suffix}.png",
                if wire { "-wire" } else { "" },
                if options.rear { "-rear" } else { "" },
                if options.native { "-native" } else { "" }
            );
            pollster::block_on(offscreen::render(
                &scene,
                (mode == "atlas").then_some(atlas.as_path()),
                Path::new(&path),
            ))
            .expect("offscreen rendering");
            let bounds: [(f32, f32); 3] = std::array::from_fn(|axis| {
                scene
                    .vertices
                    .iter()
                    .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| {
                        (lo.min(v.position[axis]), hi.max(v.position[axis]))
                    })
            });
            std::fs::write(Path::new(&path).with_extension("txt"),format!("Scene: {name}; mode: {mode}; viewport: {}x{}; rear: {}; native: {}\nstate: facing={:?} axis={:?} horizontal={:?}\nage={} partial=0 count={}\nclip bounds: {bounds:?}\nGUI native source bounds: X=[0.928932,15.071068] Y=[0.134339,15.865661]\n",scene.width,scene.height,options.rear,options.native,options.facing,options.axis,options.horizontal,options.age,options.count)).expect("diagnostic manifest");
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bookshelf_log_grass_keep_semantic_face_textures_in_every_representation() {
        let resolver = FirstPartyTextures;
        for (definition, top, side, bottom) in [
            (
                rustcraft_minecraft_b173::blocks::BOOKSHELF,
                "minecraft_b173:planks",
                "minecraft_b173:bookshelf",
                "minecraft_b173:planks",
            ),
            (
                rustcraft_minecraft_b173::blocks::LOG,
                "minecraft_b173:log_top",
                "minecraft_b173:log_side",
                "minecraft_b173:log_top",
            ),
            (
                rustcraft_minecraft_b173::blocks::GRASS,
                "minecraft_b173:grass_top",
                "minecraft_b173:grass_side",
                "minecraft_b173:dirt",
            ),
        ] {
            let model = rustcraft_render::inspection::BlockModel::resolve(
                BlockState::new(definition.id),
                &resolver,
            )
            .unwrap();
            for f in geometry::FACES {
                let expected = match f.direction {
                    Face::Top => top,
                    Face::Bottom => bottom,
                    _ => side,
                };
                assert_eq!(definition.textures.face(f.direction as usize), expected);
                let tile = rustcraft_minecraft_b173::blocks::atlas_tile(expected).unwrap();
                assert_eq!(
                    (model.texture(f.direction).x, model.texture(f.direction).y),
                    tile
                );
            }
            let mut before = Vec::new();
            let mut after = Vec::new();
            let item = ItemSprite {
                model: Some(model),
                position: Vec3::new(3., 4., 5.),
                top: model.texture(Face::Top),
                side: model.texture(Face::North),
                bottom: model.texture(Face::Bottom),
                tint: [1.; 3],
                age: 0.,
                count: 1,
                hover_start: 0.,
            };
            rustcraft_render::append_dropped_items(&mut before, &[item]);
            for age in [0., 5., 10., 20., 40.] {
                after.clear();
                rustcraft_render::append_dropped_items(&mut after, &[ItemSprite { age, ..item }]);
                for (a, b) in before.iter().zip(&after) {
                    assert_eq!(a.uv, b.uv);
                    assert!(b.position.iter().all(|v| v.is_finite()));
                }
                if age > 0. {
                    assert_ne!(before[0].position, after[0].position);
                }
            }
            assert_eq!(item.position, Vec3::new(3., 4., 5.));
            for (count, copies) in [(1, 1), (2, 2), (6, 3), (21, 4)] {
                after.clear();
                rustcraft_render::append_dropped_items(&mut after, &[ItemSprite { count, ..item }]);
                assert_eq!(after.len(), copies * 36);
            }
        }
    }
}

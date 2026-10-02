use rustcraft_content::resources::{
    AtlasPolicy, ResourceLimits, ResourcePackage, SamplerPolicy, compile_resources,
    discover_package, write_atlas_debug,
};
use rustcraft_content::{PackageId, ResourceId};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn usage() -> ! {
    eprintln!(
        "usage:\n  rustcraft-resource report <package-root> <package-id>\n  rustcraft-resource inspect <package-root> <package-id> <resource-id>\n  rustcraft-resource atlas-debug <package-root> <package-id> <output-dir>\n  rustcraft-resource stress [resource-count]"
    );
    std::process::exit(2)
}

fn compile_native(
    root: &Path,
    package: PackageId,
) -> rustcraft_content::resources::CompiledResources {
    let package =
        discover_package(root, package, ResourceLimits::default()).unwrap_or_else(|error| {
            eprintln!("resource discovery failed: {error}");
            std::process::exit(1)
        });
    compile_resources(
        &[package],
        AtlasPolicy::default(),
        ResourceLimits::default(),
        Some(Path::new("target/resource-cache/inspector")),
    )
    .unwrap_or_else(|error| {
        eprintln!("resource compilation failed: {error}");
        std::process::exit(1)
    })
}

fn print_texture(texture: &rustcraft_content::resources::CompiledTexture) {
    println!(
        "id={}\nprovider={}\nsource={}\nsource_hash={}\ndimensions={}x{}\npage={}\npixel_rect={},{},{},{}\nuv={:?}..{:?}\npadding={}\nsampler={:?}",
        texture.id,
        texture.provider,
        texture.source_path.display(),
        texture.source_hash,
        texture.pixel_rect.width,
        texture.pixel_rect.height,
        texture.page,
        texture.pixel_rect.x,
        texture.pixel_rect.y,
        texture.pixel_rect.width,
        texture.pixel_rect.height,
        texture.uv_min,
        texture.uv_max,
        texture.padding,
        texture.sampler
    );
}

fn write_fixture(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let file = fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(file, 8, 8);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&[127; 8 * 8 * 4])
        .unwrap();
}

fn stress(count: usize) {
    let root = PathBuf::from("target/resource-stress");
    let texture = root.join("fixture.png");
    write_fixture(&texture);
    let mut package =
        ResourcePackage::new(PackageId::parse("stress:package/generated").unwrap(), &root);
    for index in 0..count {
        package
            .add_texture(
                ResourceId::parse(format!("stress:textures/generated/t{index:05}")).unwrap(),
                "fixture.png",
                None,
                SamplerPolicy::Nearest,
            )
            .unwrap();
    }
    let policy = AtlasPolicy {
        page_width: 256,
        page_height: 256,
        max_pages: 1024,
        ..AtlasPolicy::default()
    };
    let cold = compile_resources(
        &[package.clone()],
        policy,
        ResourceLimits {
            max_files: count.max(1),
            ..ResourceLimits::default()
        },
        Some(&root.join("cache")),
    )
    .unwrap();
    println!("cold {}", cold.report());
    let warm = compile_resources(
        &[package],
        policy,
        ResourceLimits {
            max_files: count.max(1),
            ..ResourceLimits::default()
        },
        Some(&root.join("cache")),
    )
    .unwrap();
    println!("warm {}", warm.report());
    write_atlas_debug(&warm, &root.join("atlas")).unwrap();
}

fn main() {
    let mut arguments = std::env::args().skip(1);
    match arguments.next().as_deref() {
        Some("report") => {
            let root = PathBuf::from(arguments.next().unwrap_or_else(|| usage()));
            let package = PackageId::parse(arguments.next().unwrap_or_else(|| usage()))
                .unwrap_or_else(|_| usage());
            let resources = compile_native(&root, package);
            println!("{}", resources.report());
            for replacement in resources.overrides {
                println!(
                    "override {}: {} -> {}",
                    replacement.id, replacement.original_provider, replacement.overriding_provider
                );
            }
        }
        Some("inspect") => {
            let root = PathBuf::from(arguments.next().unwrap_or_else(|| usage()));
            let package = PackageId::parse(arguments.next().unwrap_or_else(|| usage()))
                .unwrap_or_else(|_| usage());
            let id = ResourceId::parse(arguments.next().unwrap_or_else(|| usage()))
                .unwrap_or_else(|_| usage());
            let resources = compile_native(&root, package);
            match resources.texture(&id) {
                Some(texture) => print_texture(texture),
                None => {
                    eprintln!("resource {id} was not resolved");
                    std::process::exit(1)
                }
            }
        }
        Some("atlas-debug") => {
            let root = PathBuf::from(arguments.next().unwrap_or_else(|| usage()));
            let package = PackageId::parse(arguments.next().unwrap_or_else(|| usage()))
                .unwrap_or_else(|_| usage());
            let output = PathBuf::from(arguments.next().unwrap_or_else(|| usage()));
            let resources = compile_native(&root, package);
            for path in write_atlas_debug(&resources, &output).unwrap() {
                println!("{}", path.display());
            }
        }
        Some("stress") => stress(
            arguments
                .next()
                .map_or(1000, |value| value.parse().unwrap_or_else(|_| usage())),
        ),
        _ => usage(),
    }
}

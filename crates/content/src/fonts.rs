//! Semantic, load-time font resources. Ordered package layers override roles deterministically.
use std::{collections::BTreeMap, sync::Arc};
#[derive(Clone, Debug)]
pub struct FontResource {
    pub owner: String,
    pub family: String,
    pub bytes: Arc<Vec<u8>>,
    pub license: &'static str,
}
#[derive(Clone, Debug, Default)]
pub struct FontResources {
    roles: BTreeMap<String, Vec<FontResource>>,
}
impl FontResources {
    pub fn register(&mut self, role: &str, stack: Vec<FontResource>) -> Result<(), String> {
        if !role.contains(':')
            || role.len() > 128
            || stack.is_empty()
            || stack.len() > 8
            || stack.iter().any(|f| {
                f.bytes.is_empty()
                    || f.bytes.len() > 8 * 1024 * 1024
                    || f.family.len() > 128
                    || f.owner.len() > 128
            })
            || (!self.roles.contains_key(role) && self.roles.len() >= 16)
        {
            return Err("invalid bounded font resource role/stack".into());
        }
        self.roles.insert(role.into(), stack);
        Ok(())
    }
    pub fn resolve(&self, role: &str) -> Result<&[FontResource], String> {
        self.roles
            .get(role)
            .map(Vec::as_slice)
            .ok_or_else(|| format!("unavailable font role {role}"))
    }
    pub fn builtin() -> Self {
        macro_rules! font {
            ($dir:literal,$file:literal,$family:literal) => {
                FontResource {
                    owner: "rustcraft:builtin".into(),
                    family: $family.into(),
                    bytes: Arc::new(
                        include_bytes!(concat!("../fonts/", $dir, "/", $file)).to_vec(),
                    ),
                    license: include_str!(concat!("../fonts/", $dir, "/OFL.txt")),
                }
            };
        }
        let stack = vec![
            font!("fusion", "FusionPixel.otf", "Fusion Pixel 12px Mono latin"),
            font!("notosans", "Font.ttf", "Noto Sans"),
            font!("notosansmath", "Font.ttf", "Noto Sans Math"),
            font!("notosansarabic", "Font.ttf", "Noto Sans Arabic"),
            font!("notosanshebrew", "Font.ttf", "Noto Sans Hebrew"),
            font!("notosansdevanagari", "Font.ttf", "Noto Sans Devanagari"),
            font!("notoemoji", "Font.ttf", "Noto Emoji"),
        ];
        let mut resources = Self::default();
        for role in [
            "rustcraft:font/ui",
            "rustcraft:font/debug",
            "rustcraft:font/mono",
        ] {
            resources.register(role, stack.clone()).unwrap();
        }
        resources
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn package_override_is_semantic_and_ordered() {
        let mut r = FontResources::builtin();
        let mut alternate = r.resolve("rustcraft:font/ui").unwrap().to_vec();
        alternate.swap(0, 1);
        alternate[0].owner = "sandbox_test:package".into();
        r.register("rustcraft:font/ui", alternate).unwrap();
        assert_eq!(
            r.resolve("rustcraft:font/ui").unwrap()[0].family,
            "Noto Sans"
        );
        assert_eq!(
            r.resolve("rustcraft:font/debug").unwrap()[0].owner,
            "rustcraft:builtin"
        );
    }
}

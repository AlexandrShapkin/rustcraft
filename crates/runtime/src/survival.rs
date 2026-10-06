use crate::inventory::ItemStack;
use rustcraft_engine_core::{Aabb, EntityId, ItemId, Vec3, World};
use rustcraft_mod_api::BlockRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    Development,
    Survival,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItemEntity {
    pub id: EntityId,
    /// Monotonic durable-state revision used to select the newest copy during transfer recovery.
    pub persistence_revision: u64,
    pub stack: ItemStack,
    pub position: Vec3,
    pub velocity: Vec3,
    pub age: f32,
    pub pickup_delay: f32,
}
impl ItemEntity {
    pub fn tick(&mut self, world: &World, registry: &BlockRegistry, dt: f32) {
        if self.pickup_delay > 0. {
            self.pickup_delay = (self.pickup_delay - dt).max(0.);
        }
        self.age += dt;
        if self.age >= 300. {
            return;
        }
        if self.velocity.y != 0.
            || !world.collides_shapes(self.bounds(), |state, bounds| {
                registry.overlaps_state(state, bounds)
            })
        {
            self.velocity.y -= 9.81 * dt;
            let moved = Vec3::new(
                self.velocity.x * dt,
                self.velocity.y * dt,
                self.velocity.z * dt,
            );
            let (bounds, actual) =
                world.move_and_collide_shapes(self.bounds(), moved, |state, bounds| {
                    registry.overlaps_state(state, bounds)
                });
            self.position = Vec3::new(
                (bounds.min.x + bounds.max.x) / 2.,
                (bounds.min.y + bounds.max.y) / 2.,
                (bounds.min.z + bounds.max.z) / 2.,
            );
            if actual.y != moved.y {
                self.velocity.y = -self.velocity.y * 0.2;
            } else {
                self.velocity.y *= 0.98;
            }
            self.velocity.x *= 0.98;
            self.velocity.z *= 0.98;
        }
    }
    pub fn bounds(&self) -> Aabb {
        Aabb::new(
            self.position - Vec3::new(0.125, 0.125, 0.125),
            self.position + Vec3::new(0.125, 0.125, 0.125),
        )
    }

    #[must_use]
    pub fn column(&self) -> rustcraft_engine_core::ChunkPos {
        rustcraft_engine_core::ChunkPos {
            x: (self.position.x.floor() as i32).div_euclid(16),
            z: (self.position.z.floor() as i32).div_euclid(16),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecipeKind {
    Shaped,
    Shapeless,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipe {
    pub id: String,
    pub kind: RecipeKind,
    pub width: u8,
    pub inputs: Vec<Option<ItemId>>,
    pub output: ItemStack,
}
impl Recipe {
    pub fn matches(&self, grid: &[Option<ItemStack>]) -> bool {
        let items: Vec<_> = grid.iter().map(|stack| stack.map(|s| s.item)).collect();
        rustcraft_game_api::recipe_matches(self.kind == RecipeKind::Shaped, &items, &self.inputs)
    }
}

#[derive(Debug, Default, Clone)]
pub struct RecipeRegistry {
    pub recipes: Vec<Recipe>,
}
impl RecipeRegistry {
    pub fn find(&self, grid: &[Option<ItemStack>]) -> Option<Recipe> {
        self.recipes.iter().find(|r| r.matches(grid)).cloned()
    }
    pub fn from_definitions(r: &BlockRegistry) -> Self {
        let item = |key: &rustcraft_content::ResourceId| {
            r.resolve_item_key(key).expect("validated recipe item")
        };
        Self {
            recipes: r
                .recipes()
                .iter()
                .map(|recipe| Recipe {
                    id: recipe.local_alias.unwrap_or(recipe.key.as_str()).to_owned(),
                    kind: if recipe.shaped {
                        RecipeKind::Shaped
                    } else {
                        RecipeKind::Shapeless
                    },
                    width: recipe.width,
                    inputs: recipe
                        .inputs
                        .iter()
                        .map(|key| key.as_ref().map(item))
                        .collect(),
                    output: ItemStack {
                        item: item(&recipe.output),
                        count: recipe.count,
                        damage: 0,
                    },
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustcraft_engine_core::BlockId;
    use rustcraft_mod_api::{BlockDefinition, ItemDefinition};
    #[test]
    fn item_entity_falls_and_recipe_matches() {
        let mut r = BlockRegistry::default();
        let b = BlockDefinition::cube(1, "test:block", "test:block");
        r.register(b).unwrap();
        r.register_item(ItemDefinition {
            id: ItemId(1),
            name: "test:block",
            max_stack: 64,
            placeable: Some(BlockId(1)),
            capabilities: &[],
            tool: None,
        })
        .unwrap();
        let mut w = World::new(BlockId(0));
        w.set(
            rustcraft_engine_core::BlockPos { x: 0, y: 0, z: 0 },
            BlockId(1),
        );
        let mut e = ItemEntity {
            id: EntityId::from_parts(1, 1),
            persistence_revision: 1,
            stack: ItemStack {
                item: ItemId(1),
                count: 2,
                damage: 0,
            },
            position: Vec3::new(0.5, 3.0, 0.5),
            velocity: Vec3::ZERO,
            age: 0.0,
            pickup_delay: 0.0,
        };
        for _ in 0..100 {
            e.tick(&w, &r, 0.02);
        }
        assert!(e.position.y.is_finite() && e.age > 0.);
        assert!(
            (e.position.y - 1.125).abs() < 0.03,
            "item settled at {}",
            e.position.y
        );
        let mut rr = RecipeRegistry::default();
        rr.recipes.push(Recipe {
            id: "x".into(),
            kind: RecipeKind::Shapeless,
            width: 2,
            inputs: vec![Some(ItemId(1))],
            output: ItemStack {
                item: ItemId(1),
                count: 4,
                damage: 0,
            },
        });
        assert!(
            rr.find(&[
                Some(ItemStack {
                    item: ItemId(1),
                    count: 1,
                    damage: 0
                }),
                None,
                None,
                None
            ])
            .is_some()
        );
    }
}

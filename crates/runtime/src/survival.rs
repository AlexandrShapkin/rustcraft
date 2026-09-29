use crate::inventory::ItemStack;
use rustcraft_engine_core::{Aabb, BlockId, ItemId, Vec3, World};
use rustcraft_mod_api::{BlockRegistry, ToolCategory, ToolTier};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    Development,
    Survival,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItemEntity {
    pub id: u64,
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
        if self.age > 300. {
            return;
        }
        if self.velocity.y != 0. || !world.collides(self.bounds(), |b| registry.is_solid(b)) {
            self.velocity.y -= 9.81 * dt;
            let moved = Vec3::new(
                self.velocity.x * dt,
                self.velocity.y * dt,
                self.velocity.z * dt,
            );
            let (bounds, actual) =
                world.move_and_collide(self.bounds(), moved, |b| registry.is_solid(b));
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecipeKind {
    Shaped,
    Shapeless,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipe {
    pub id: &'static str,
    pub kind: RecipeKind,
    pub width: u8,
    pub inputs: Vec<Option<ItemId>>,
    pub output: ItemStack,
}
impl Recipe {
    pub fn matches(&self, grid: &[Option<ItemStack>]) -> bool {
        match self.kind {
            RecipeKind::Shapeless => {
                let mut a = grid.iter().flatten().map(|s| s.item).collect::<Vec<_>>();
                let mut b = self.inputs.iter().flatten().copied().collect::<Vec<_>>();
                a.sort();
                b.sort();
                a == b
            }
            RecipeKind::Shaped => {
                grid.len() == self.inputs.len()
                    && grid
                        .iter()
                        .zip(&self.inputs)
                        .all(|(a, b)| a.map(|s| s.item) == *b)
            }
        }
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
    pub fn add_defaults(&mut self, r: &BlockRegistry) {
        let id = |n: &str| {
            r.item(
                r.by_name(n)
                    .and_then(|b| b.item)
                    .unwrap_or(ItemId(u32::MAX)),
            )
            .map(|_| r.by_name(n).unwrap().item.unwrap())
        };
        let Some(log) = id("minecraft_b173:log") else {
            return;
        };
        let Some(planks) = id("minecraft_b173:planks") else {
            return;
        };
        let Some(stone) = id("minecraft_b173:cobblestone") else {
            return;
        };
        let stick = ItemId(100);
        let wp = ItemId(101);
        let wa = ItemId(102);
        let ws = ItemId(103);
        let sp = ItemId(104);
        let sa = ItemId(105);
        let ss = ItemId(106);
        let out = |item, count| ItemStack {
            item,
            count,
            damage: 0,
        };
        self.recipes.push(Recipe {
            id: "log_to_planks",
            kind: RecipeKind::Shapeless,
            width: 2,
            inputs: vec![Some(log)],
            output: out(planks, 4),
        });
        self.recipes.push(Recipe {
            id: "planks_to_sticks",
            kind: RecipeKind::Shaped,
            width: 2,
            inputs: vec![Some(planks), None, Some(planks), None],
            output: out(stick, 4),
        });
        for (id, cat) in [
            (wp, ToolCategory::Pickaxe),
            (wa, ToolCategory::Axe),
            (ws, ToolCategory::Shovel),
            (sp, ToolCategory::Pickaxe),
            (sa, ToolCategory::Axe),
            (ss, ToolCategory::Shovel),
        ] {
            let material = if id >= sp { stone } else { planks };
            let output = out(id, 1);
            let inputs = match cat {
                ToolCategory::Pickaxe => {
                    vec![Some(material), Some(material), Some(stick), Some(stick)]
                }
                ToolCategory::Axe => {
                    vec![Some(material), Some(material), Some(material), Some(stick)]
                }
                ToolCategory::Shovel => vec![Some(material), None, Some(stick), None],
            };
            self.recipes.push(Recipe {
                id: "tool",
                kind: RecipeKind::Shaped,
                width: 2,
                inputs,
                output,
            });
        }
    }
}

pub fn tool_speed(registry: &BlockRegistry, held: Option<ItemStack>, block: BlockId) -> f32 {
    let Some(def) = registry.get(block) else {
        return 1.;
    };
    let Some(stack) = held else { return 1. };
    let Some(tool) = registry.item(stack.item).and_then(|i| i.tool) else {
        return 1.;
    };
    if def.preferred_tool == Some(tool.category) {
        tool.speed
    } else {
        1.
    }
}
pub fn tier_ok(registry: &BlockRegistry, held: Option<ItemStack>, block: BlockId) -> bool {
    let Some(def) = registry.get(block) else {
        return false;
    };
    let Some(min) = def.preferred_tool else {
        return true;
    };
    let Some(stack) = held else { return false };
    registry
        .item(stack.item)
        .and_then(|i| i.tool)
        .is_some_and(|t| t.category == min && t.tier >= ToolTier::Wood)
}

#[cfg(test)]
mod tests {
    use super::*;
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
            id: 1,
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
            id: "x",
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

use rustcraft_engine_core::ItemId;
use rustcraft_mod_api::BlockRegistry;
pub const HOTBAR_SLOTS: usize = 9;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemStack {
    pub item: ItemId,
    pub count: u16,
    pub damage: u16,
}
#[derive(Debug, Clone)]
pub struct Inventory {
    slots: [Option<ItemStack>; 36],
    selected: usize,
}
impl Default for Inventory {
    fn default() -> Self {
        Self {
            slots: [None; 36],
            selected: 0,
        }
    }
}
impl Inventory {
    pub fn from_slots(slots: [Option<ItemStack>; 36], selected: usize) -> Option<Self> {
        (selected < HOTBAR_SLOTS).then_some(Self { slots, selected })
    }
    /// Applies the Beta 1.7.3 `Container.slotClick` left/right transaction to one slot.
    /// The returned stack is the new cursor contents; the caller owns the cursor state.
    pub fn slot_click(
        &mut self,
        slot: Option<usize>,
        button: u8,
        cursor: Option<ItemStack>,
        registry: &BlockRegistry,
    ) -> Option<ItemStack> {
        let Some(index) = slot else {
            return cursor
                .map(|mut s| {
                    if button == 0 {
                        s.count = 0;
                    } else {
                        s.count = s.count.saturating_sub(1);
                    }
                    s
                })
                .filter(|s| s.count > 0);
        };
        if index >= self.slots.len() {
            return cursor;
        }
        let mut cursor = cursor;
        let mut target = self.slots[index];
        let limit = target
            .and_then(|s| registry.item(s.item).map(|d| d.max_stack))
            .unwrap_or_else(|| {
                cursor
                    .and_then(|s| registry.item(s.item).map(|d| d.max_stack))
                    .unwrap_or(64)
            });
        match (cursor, target) {
            (None, None) => {}
            (None, Some(mut stack)) => {
                let take = if button == 1 {
                    stack.count.div_ceil(2)
                } else {
                    stack.count
                };
                stack.count -= take;
                target = (stack.count > 0).then_some(stack);
                cursor = Some(ItemStack {
                    count: take,
                    ..stack
                });
            }
            (Some(mut held), None) => {
                let take = if button == 1 {
                    1
                } else {
                    held.count.min(limit)
                };
                held.count -= take;
                target = Some(ItemStack {
                    count: take,
                    ..held
                });
                cursor = (held.count > 0).then_some(held);
            }
            (Some(mut held), Some(mut existing))
                if held.item == existing.item && held.damage == existing.damage =>
            {
                let max = registry
                    .item(existing.item)
                    .map_or(64, |d| d.max_stack)
                    .min(limit);
                let take = if button == 1 {
                    1
                } else {
                    held.count.min(max.saturating_sub(existing.count))
                };
                existing.count += take;
                held.count -= take;
                target = Some(existing);
                cursor = (held.count > 0).then_some(held);
            }
            (Some(held), Some(existing)) => {
                if held.count <= limit {
                    target = Some(held);
                    cursor = Some(existing);
                } else {
                    target = Some(existing);
                    cursor = Some(held);
                }
            }
        }
        self.slots[index] = target;
        cursor
    }
    pub fn slots(&self) -> &[Option<ItemStack>] {
        &self.slots
    }
    pub fn selected(&self) -> usize {
        self.selected
    }
    pub fn held(&self) -> Option<ItemStack> {
        self.slots[self.selected]
    }
    pub fn select(&mut self, slot: usize) -> bool {
        if slot >= HOTBAR_SLOTS {
            return false;
        }
        self.selected = slot;
        true
    }
    pub fn scroll(&mut self, delta: i32) {
        self.selected =
            (self.selected as i32 + delta.rem_euclid(HOTBAR_SLOTS as i32)).rem_euclid(9) as usize;
    }
    pub fn insert(&mut self, item: ItemId, mut count: u16, registry: &BlockRegistry) -> u16 {
        let Some(def) = registry.item(item) else {
            return count;
        };
        for slot in &mut self.slots {
            if let Some(stack) = slot.as_mut()
                && stack.item == item
            {
                let add = count.min(def.max_stack - stack.count);
                stack.count += add;
                count -= add;
            }
        }
        for slot in &mut self.slots {
            if slot.is_none() && count > 0 {
                let n = count.min(def.max_stack);
                *slot = Some(ItemStack {
                    item,
                    count: n,
                    damage: 0,
                });
                count -= n;
            }
        }
        count
    }
    pub fn remove(&mut self, slot: usize, count: u16) -> bool {
        let Some(Some(stack)) = self.slots.get_mut(slot) else {
            return false;
        };
        if count == 0 || count > stack.count {
            return false;
        }
        stack.count -= count;
        if stack.count == 0 {
            self.slots[slot] = None;
        }
        true
    }
    pub fn insert_partial(&mut self, stack: ItemStack, registry: &BlockRegistry) -> ItemStack {
        let remainder = self.insert(stack.item, stack.count, registry);
        ItemStack {
            count: remainder,
            ..stack
        }
    }
    pub fn occupied(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }
    pub fn slot(&self, index: usize) -> Option<ItemStack> {
        self.slots.get(index).copied().flatten()
    }
    pub fn swap_slots(&mut self, a: usize, b: usize) -> bool {
        if a >= self.slots.len() || b >= self.slots.len() {
            return false;
        }
        self.slots.swap(a, b);
        true
    }
    pub fn take_slot(&mut self, index: usize) -> Option<ItemStack> {
        if index >= self.slots.len() {
            None
        } else {
            self.slots[index].take()
        }
    }
    pub fn put_slot(&mut self, index: usize, stack: Option<ItemStack>) -> bool {
        if index >= self.slots.len() {
            return false;
        }
        self.slots[index] = stack;
        true
    }
    pub fn damage_selected(&mut self, amount: u16, registry: &BlockRegistry) -> bool {
        let Some(stack) = self.slots[self.selected].as_mut() else {
            return false;
        };
        let Some(def) = registry.item(stack.item).and_then(|i| i.tool) else {
            return false;
        };
        stack.damage = stack.damage.saturating_add(amount);
        if stack.damage >= def.durability {
            self.slots[self.selected] = None;
        }
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use rustcraft_mod_api::{BlockDefinition, ItemDefinition};
    #[test]
    fn limits_empty_selection_and_removal() {
        let mut r = BlockRegistry::default();
        r.register(BlockDefinition::cube(1, "test:block", "test:tex"))
            .unwrap();
        r.register_item(ItemDefinition {
            id: ItemId(7),
            name: "test:item",
            max_stack: 4,
            placeable: None,
            capabilities: &[],
            tool: None,
        })
        .unwrap();
        let mut i = Inventory::default();
        assert!(!i.select(9));
        i.scroll(-1);
        assert_eq!(i.selected(), 8);
        assert_eq!(i.insert(ItemId(7), 7, &r), 0);
        assert_eq!(i.slots()[0].unwrap().count, 4);
        assert_eq!(i.slots()[1].unwrap().count, 3);
        assert!(!i.remove(36, 1));
        assert!(!i.remove(0, 5));
        assert!(i.remove(0, 4));
        assert_eq!(i.slots()[0], None);
        assert_eq!(i.insert(ItemId(99), 5, &r), 5);
    }
    #[test]
    fn full_capacity_returns_remainder_and_zero_changes_nothing() {
        let mut r = BlockRegistry::default();
        r.register_item(ItemDefinition {
            id: ItemId(1),
            name: "test:item",
            max_stack: 4,
            placeable: None,
            capabilities: &[],
            tool: None,
        })
        .unwrap();
        let mut i = Inventory::default();
        assert_eq!(i.insert(ItemId(1), 0, &r), 0);
        assert!(i.slots().iter().all(Option::is_none));
        assert_eq!(i.insert(ItemId(1), 150, &r), 6);
        assert!(i.slots().iter().all(|s| s.unwrap().count == 4));
        assert!(!i.remove(0, 0));
        assert!(i.remove(3, 2));
        assert_eq!(i.insert(ItemId(1), 3, &r), 1);
        i.scroll(i32::MAX);
        assert!(i.selected() < HOTBAR_SLOTS);
    }

    #[test]
    fn beta_slot_click_split_merge_and_place_one() {
        let mut r = BlockRegistry::default();
        r.register_item(ItemDefinition {
            id: ItemId(1),
            name: "test:item",
            max_stack: 64,
            placeable: None,
            capabilities: &[],
            tool: None,
        })
        .unwrap();
        let mut i = Inventory::default();
        i.put_slot(
            0,
            Some(ItemStack {
                item: ItemId(1),
                count: 3,
                damage: 0,
            }),
        );
        let cursor = i.slot_click(Some(0), 1, None, &r).unwrap();
        assert_eq!(cursor.count, 2);
        assert_eq!(i.slot(0).unwrap().count, 1);
        let cursor = i.slot_click(Some(0), 1, Some(cursor), &r);
        assert_eq!(cursor.unwrap().count, 1);
        assert_eq!(i.slot(0).unwrap().count, 2);
        let cursor = i.slot_click(Some(1), 0, cursor, &r);
        assert!(cursor.is_none());
        assert_eq!(i.slot(1).unwrap().count, 1);
    }
}

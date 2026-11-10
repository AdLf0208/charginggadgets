// Pure Charging Gadgets slice-1 arithmetic (standalone crate lib, injected as
// `crate::game::charginggadgets`). No game access lives here: the station's
// fuel burn, the FE a burn tick makes, the FE one charge tick moves into an
// item, and the four numbers the menu shows.
//
// Upstream (`ChargingStationTile.ticker`, read from Charging Gadgets 1.14.1)
// burns one `counter` step per server tick while the buffer has room for a
// whole burn tick, adds 625 FE per step, and loads `floor(burnTime / 50)`
// steps from one fuel item. It then charges the item in slot 1 with
// `min(stored, 2500)` FE through `Capabilities.Energy.ITEM` and keeps what the
// item accepted. `ChargerEnergyStorage` holds 1,000,000 FE (the config's
// `chargerMaxPower`), takes inserts of any size and gives nothing out.

/// `Config.GENERAL.chargerMaxPower`'s default: the station's FE buffer.
pub const CAPACITY: i32 = 1_000_000;
/// `ChargingStationTile.burn`: the FE one burning tick adds.
pub const FE_PER_BURN_TICK: i32 = 625;
/// `ChargingStationTile.chargeItem`: the most one tick moves into the item.
pub const CHARGE_PER_TICK: i32 = 2_500;
/// `ChargingStationTile.initBurn`: smelting ticks per burn step.
pub const SMELTING_TICKS_PER_BURN_STEP: u32 = 50;
/// `ChargerEnergyStorage.maxInOut`: the most one insert moves (a drain is
/// refused, `canExtract` is false upstream).
pub const MAX_TRANSFER: i32 = 1_000_000;

/// The fuel slot, as `ChargingStationTile.initBurn` reads it: the next item's
/// `getBurnTime(RecipeType.SMELTING)` without taking it, and the shrink (or
/// crafting remainder) that takes one.
pub trait FuelSlot {
    /// The next fuel item's smelting burn time in ticks, if the slot is not
    /// empty.
    fn peek(&self) -> Option<u32>;
    /// `ItemStack.shrink(1)`, or the crafting remainder in its place.
    fn take(&mut self);
}

/// What one tick of the burn did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BurnTick {
    /// FE added to the buffer (`0` or `FE_PER_BURN_TICK`).
    pub added: i32,
    /// Fuel items the tick took out of the slot.
    pub fuel_consumed: u32,
}

/// The item in the charge slot, as the engine's item-energy helpers answer it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemBuffer {
    /// What the item holds now (FE).
    pub energy: i32,
    /// Its buffer's size (FE).
    pub capacity: i32,
    /// The most one insert moves.
    pub max_insert: i32,
}

/// The station's saved state, under upstream's keys (`energy`, `counter`,
/// `maxburn`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Station {
    /// Stored FE.
    pub energy: i32,
    /// Burning steps left; upstream's `counter` is an int and can go negative
    /// for a fuel under 50 smelting ticks (see `burn_tick`).
    pub counter: i32,
    /// Steps the last loaded fuel item gave; upstream's `maxburn`.
    pub max_burn: i32,
}

impl Station {
    /// An empty station.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The FE the buffer can still take.
    #[must_use]
    pub fn room(&self) -> i32 {
        CAPACITY.saturating_sub(self.energy).max(0)
    }

    /// `ChargingStationTile.tryBurn` + `burn` + `initBurn`, in one tick: add a
    /// burn step while the buffer has room for a whole one, spend a counter
    /// step, and load the next fuel item when the counter reaches zero (which
    /// upstream does inside the same `burn`, so two items back to back burn
    /// without a gap).
    pub fn burn_tick(&mut self, fuel: &mut impl FuelSlot) -> BurnTick {
        let mut tick = BurnTick::default();
        if self.room() < FE_PER_BURN_TICK {
            return tick;
        }
        if self.counter > 0 || self.load_fuel(fuel, &mut tick) {
            self.burn_step(fuel, &mut tick);
        }
        tick
    }

    /// `ChargingStationTile.burn`: one step of FE, one counter step, and the
    /// next item loaded when the counter runs out.
    fn burn_step(&mut self, fuel: &mut impl FuelSlot, tick: &mut BurnTick) {
        self.energy = (self.energy + FE_PER_BURN_TICK).min(CAPACITY);
        tick.added += FE_PER_BURN_TICK;
        self.counter -= 1;
        if self.counter == 0 {
            self.max_burn = 0;
            self.load_fuel(fuel, tick);
        }
    }

    /// `ChargingStationTile.initBurn`: take one fuel item whose smelting burn
    /// time is positive and turn it into `floor(burnTime / 50)` steps. An item
    /// with no burn time stays where it is, as upstream's `shrink` only runs
    /// inside the `burnTime > 0` branch.
    fn load_fuel(&mut self, fuel: &mut impl FuelSlot, tick: &mut BurnTick) -> bool {
        let Some(burn_time) = fuel.peek().filter(|burn| *burn > 0) else {
            return false;
        };
        fuel.take();
        tick.fuel_consumed += 1;
        self.counter = burn_steps(burn_time);
        self.max_burn = self.counter;
        true
    }

    /// `ChargingStationTile.chargeItem`: the FE that moves from the buffer into
    /// one item this tick, which the station pays. Nothing moves for an item
    /// without a buffer, a full item, or an empty station.
    pub fn charge_step(&mut self, item: Option<ItemBuffer>) -> i32 {
        let Some(item) = item else {
            return 0;
        };
        let room = item.capacity.saturating_sub(item.energy).max(0);
        let moved = self
            .energy
            .clamp(0, CHARGE_PER_TICK)
            .min(room)
            .min(item.max_insert)
            .max(0);
        self.energy -= moved;
        moved
    }

    /// The menu's four data slots, as upstream's `ContainerData` builds them:
    /// the buffer and its size in 32-FE units (so both fit a 16-bit network
    /// value), then the burning counter and the last fuel's step count.
    #[must_use]
    pub fn data_slots(&self) -> [i32; 4] {
        [self.energy / 32, CAPACITY / 32, self.counter, self.max_burn]
    }
}

/// `initBurn`'s `counter = floor(burnTime) / 50`.
#[must_use]
pub fn burn_steps(burn_time: u32) -> i32 {
    i32::try_from(burn_time / SMELTING_TICKS_PER_BURN_STEP).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fuel slot over a list of smelting burn times.
    struct FakeFuel {
        items: Vec<u32>,
    }

    impl FakeFuel {
        fn of(items: &[u32]) -> Self {
            Self {
                items: items.to_vec(),
            }
        }
    }

    impl FuelSlot for FakeFuel {
        fn peek(&self) -> Option<u32> {
            self.items.first().copied()
        }

        fn take(&mut self) {
            if !self.items.is_empty() {
                self.items.remove(0);
            }
        }
    }

    /// A discharged item with a 1,000,000 FE buffer and no per-call limit.
    fn buffer(energy: i32, capacity: i32, max_insert: i32) -> ItemBuffer {
        ItemBuffer {
            energy,
            capacity,
            max_insert,
        }
    }

    #[test]
    fn a_burn_tick_adds_625_fe_and_spends_one_step() {
        let mut station = Station {
            energy: 0,
            counter: 32,
            max_burn: 32,
        };
        let mut fuel = FakeFuel::of(&[1600]);
        let tick = station.burn_tick(&mut fuel);
        assert_eq!(tick.added, FE_PER_BURN_TICK);
        assert_eq!(tick.fuel_consumed, 0);
        assert_eq!(station.energy, 625);
        assert_eq!(station.counter, 31);
        assert_eq!(station.max_burn, 32);
    }

    #[test]
    fn one_coal_is_32_steps_and_20000_fe() {
        let mut station = Station::new();
        let mut fuel = FakeFuel::of(&[1600, 1600]);
        let mut total = 0;
        for _ in 0..32 {
            total += station.burn_tick(&mut fuel).added;
        }
        assert_eq!(total, 20_000);
        assert_eq!(station.energy, 20_000);
        // The 32nd step loads the next coal in the same tick (upstream's
        // `initBurn` inside `burn`), so burning continues without a gap: both
        // coals are out of the slot and the next 32 steps are already loaded.
        assert_eq!(station.counter, 32);
        assert_eq!(station.max_burn, 32);
        assert!(fuel.items.is_empty());
    }

    #[test]
    fn a_loaded_item_burns_its_steps_and_then_stops() {
        let mut station = Station::new();
        let mut fuel = FakeFuel::of(&[1600]);
        station.burn_tick(&mut fuel);
        // The first tick loads the coal (32 steps) and spends one of them.
        assert!(fuel.items.is_empty());
        assert_eq!(station.counter, 31);
        assert_eq!(station.max_burn, 32);
        for _ in 0..31 {
            station.burn_tick(&mut fuel);
        }
        // One coal is exactly 32 steps: the next tick finds no fuel.
        assert_eq!(station.energy, 20_000);
        assert_eq!(station.counter, 0);
        assert_eq!(station.max_burn, 0);
        let tick = station.burn_tick(&mut fuel);
        assert_eq!(tick, BurnTick::default());
        assert_eq!(station.energy, 20_000);
    }

    #[test]
    fn a_buffer_without_room_for_a_whole_tick_does_not_burn() {
        let mut station = Station {
            energy: CAPACITY - 624,
            counter: 32,
            max_burn: 32,
        };
        let mut fuel = FakeFuel::of(&[1600]);
        let tick = station.burn_tick(&mut fuel);
        assert_eq!(tick, BurnTick::default());
        assert_eq!(station.energy, CAPACITY - 624);
        assert_eq!(station.counter, 32);
        assert_eq!(fuel.items.len(), 1);
    }

    #[test]
    fn a_full_buffer_never_takes_a_fuel_item() {
        let mut station = Station {
            energy: CAPACITY,
            counter: 0,
            max_burn: 0,
        };
        let mut fuel = FakeFuel::of(&[1600]);
        for _ in 0..40 {
            station.burn_tick(&mut fuel);
        }
        assert_eq!(station.energy, CAPACITY);
        assert_eq!(fuel.items.len(), 1);
        assert_eq!(station.counter, 0);
    }

    #[test]
    fn a_fuel_without_burn_time_is_not_taken() {
        let mut station = Station::new();
        let mut fuel = FakeFuel::of(&[0, 1600]);
        assert_eq!(station.burn_tick(&mut fuel), BurnTick::default());
        assert_eq!(fuel.items.len(), 2);
        assert_eq!(station.counter, 0);
    }

    #[test]
    fn a_fuel_under_fifty_smelting_ticks_burns_one_item_a_step() {
        // 49 / 50 rounds down to 0 steps, and upstream still shrinks the item:
        // the counter goes to -1 and the next tick loads another. Preserved,
        // not fixed (ARCHITECTURE.md: keep upstream's quirks, record them).
        let mut station = Station::new();
        let mut fuel = FakeFuel::of(&[49, 49]);
        assert_eq!(station.burn_tick(&mut fuel).fuel_consumed, 1);
        assert_eq!(station.energy, 625);
        assert_eq!(station.counter, -1);
        assert_eq!(station.burn_tick(&mut fuel).fuel_consumed, 1);
        assert_eq!(station.energy, 1250);
        assert!(fuel.items.is_empty());
    }

    #[test]
    fn a_charge_tick_moves_at_most_2500_fe() {
        let mut station = Station {
            energy: 100_000,
            counter: 0,
            max_burn: 0,
        };
        let moved = station.charge_step(Some(buffer(0, 1_000_000, 1_000_000)));
        assert_eq!(moved, CHARGE_PER_TICK);
        assert_eq!(station.energy, 100_000 - CHARGE_PER_TICK);
    }

    #[test]
    fn a_charge_tick_stops_at_the_items_capacity() {
        let mut station = Station {
            energy: CAPACITY,
            counter: 0,
            max_burn: 0,
        };
        let moved = station.charge_step(Some(buffer(999_000, 1_000_000, 1_000_000)));
        assert_eq!(moved, 1_000);
        assert_eq!(station.energy, CAPACITY - 1_000);
        // A full item takes nothing more, and the station keeps its FE.
        assert_eq!(
            station.charge_step(Some(buffer(1_000_000, 1_000_000, 1_000_000))),
            0
        );
        assert_eq!(station.energy, CAPACITY - 1_000);
    }

    #[test]
    fn a_charge_tick_stops_at_the_stations_energy() {
        let mut station = Station {
            energy: 3_000,
            counter: 0,
            max_burn: 0,
        };
        assert_eq!(
            station.charge_step(Some(buffer(0, 1_000_000, 1_000_000))),
            2_500
        );
        assert_eq!(station.energy, 500);
        assert_eq!(
            station.charge_step(Some(buffer(0, 1_000_000, 1_000_000))),
            500
        );
        assert_eq!(station.energy, 0);
        assert_eq!(
            station.charge_step(Some(buffer(0, 1_000_000, 1_000_000))),
            0
        );
    }

    #[test]
    fn the_items_own_limit_per_insert_caps_the_move() {
        let mut station = Station {
            energy: 100_000,
            counter: 0,
            max_burn: 0,
        };
        assert_eq!(station.charge_step(Some(buffer(0, 1_000_000, 300))), 300);
        assert_eq!(station.energy, 100_000 - 300);
    }

    #[test]
    fn an_item_without_a_buffer_takes_nothing() {
        let mut station = Station {
            energy: 5_000,
            counter: 0,
            max_burn: 0,
        };
        assert_eq!(station.charge_step(None), 0);
        assert_eq!(station.energy, 5_000);
    }

    #[test]
    fn an_item_holding_more_than_its_capacity_takes_nothing() {
        let mut station = Station {
            energy: 5_000,
            counter: 0,
            max_burn: 0,
        };
        assert_eq!(station.charge_step(Some(buffer(900, 500, 500))), 0);
        assert_eq!(station.energy, 5_000);
    }

    #[test]
    fn the_menu_data_slots_are_thirty_fe_units() {
        let station = Station {
            energy: 640,
            counter: 12,
            max_burn: 32,
        };
        assert_eq!(station.data_slots(), [20, 31_250, 12, 32]);
        assert_eq!(Station::new().data_slots(), [0, 31_250, 0, 0]);
    }

    #[test]
    fn burn_steps_round_the_smelting_time_down() {
        assert_eq!(burn_steps(1600), 32);
        assert_eq!(burn_steps(200), 4);
        assert_eq!(burn_steps(50), 1);
        assert_eq!(burn_steps(49), 0);
        assert_eq!(burn_steps(0), 0);
    }

    #[test]
    fn the_room_is_never_negative() {
        assert_eq!(Station::new().room(), CAPACITY);
        assert_eq!(
            Station {
                energy: CAPACITY,
                ..Station::new()
            }
            .room(),
            0
        );
        assert_eq!(
            Station {
                energy: CAPACITY + 1,
                ..Station::new()
            }
            .room(),
            0
        );
    }
}

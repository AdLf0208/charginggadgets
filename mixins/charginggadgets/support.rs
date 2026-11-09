// Game-side glue for Charging Gadgets slice 1.
//
// Injected into `crate::game` as `charginggadgets_state`; it needs the game, so
// it is not part of the standalone library (only `runtime.rs` is).
//
// The station's block entity keeps upstream's three saved keys — `energy`
// (the API 142 buffer's amount), `counter` (burning steps left) and `maxburn`
// (the steps the last fuel item gave) — and two item slots, fuel 0 and charge 1.
// Each server tick it burns one step of 625 FE while the buffer has room for a
// whole one, loads a fuel item's `floor(burnTime / 50)` steps, then charges the
// item in slot 1 through `Capabilities.Energy.ITEM` (API 307) with at most
// 2,500 FE.
//
// Read from Charging Gadgets 1.14.1 (`ChargingStationTile.ticker/tryBurn/burn/
// initBurn/chargeItem`, `ChargerEnergyStorage`, `ChargerItemHandler`,
// `ChargingStationContainer`). See `../../FINDINGS.md`.

use std::array;

use horizon_client::containers::{BlockEntity, ModBlockEntity, ModContainer, ModEnergyBuffer};
use horizon_client::game::ItemEnergyBuffer;
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_client::text::Text;
use horizon_world::nbt::{Compound, JavaString, Tag};

use crate::game::Game;
use crate::game::charginggadgets::{CAPACITY, FuelSlot, ItemBuffer, MAX_TRANSFER, Station};
use crate::game::native_hooks::NativeScreen;
use crate::game::native_server::{NativeCommand, NativeServerPlayer};

/// The station block and its item (`BlockRegistry.CHARGING_STATION`).
pub const STATION: &str = "charginggadgets:charging_station";
/// The menu type the content mod declares (`data/charginggadgets/horizon/menu/`).
pub const MENU: &str = "charginggadgets:charging_station_menu";
/// Upstream's slot order (`ChargingStationTile.Slots`).
const FUEL_SLOT: usize = 0;
const CHARGE_SLOT: usize = 1;
const SLOT_COUNT: usize = 2;
/// Upstream's saved keys (`saveAdditional`).
const KEY_ENERGY: &str = "energy";
const KEY_COUNTER: &str = "counter";
const KEY_MAX_BURN: &str = "maxburn";

// ---------------------------------------------------------------------------
// Declaration hooks
// ---------------------------------------------------------------------------

/// The station has a block entity (`native_block_entity`, API 15).
#[allow(
    private_interfaces,
    reason = "the injected bundle is this crate's own module"
)]
pub fn on_block_entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if block == STATION {
        *entity = Some((block.to_owned(), Compound::new()));
    }
}

/// ... and ticks on the server (`native_block_entity_ticks`, API 15).
#[allow(
    private_interfaces,
    reason = "the injected bundle is this crate's own module"
)]
pub fn on_block_entity_ticks(block: &str, ticks: &mut bool) {
    if block == STATION {
        *ticks = true;
    }
}

/// Two slots and the station menu (`native_block_entity_container`, API 16/17).
/// Upstream's `ChargerItemHandler` is a plain `ItemStackHandler`, so every side
/// reaches both slots; the per-item rules live in `insertItem`, which a face
/// list cannot express (the tick refuses anything it cannot burn or charge).
#[allow(
    private_interfaces,
    reason = "the injected bundle is this crate's own module"
)]
pub fn on_container(block: &str, container: &mut Option<ModContainer>) {
    if block != STATION {
        return;
    }
    *container = Some(ModContainer {
        slots: SLOT_COUNT,
        faces: array::from_fn(|_| vec![FUEL_SLOT, CHARGE_SLOT]),
        inputs: vec![FUEL_SLOT, CHARGE_SLOT],
        outputs: vec![FUEL_SLOT, CHARGE_SLOT],
        menu: Some(MENU.to_owned()),
    });
}

/// The FE buffer (`native_block_entity_energy`, API 142): 1,000,000 FE, any
/// side may fill it, and nothing may drain it (`ChargerEnergyStorage.canExtract`
/// is false and `extractEnergy` answers 0).
#[allow(
    private_interfaces,
    reason = "the injected bundle is this crate's own module"
)]
pub fn on_block_entity_energy(block: &str, energy: &mut Option<ModEnergyBuffer>) {
    if block == STATION {
        *energy = Some(ModEnergyBuffer {
            capacity: CAPACITY,
            max_insert: MAX_TRANSFER,
            max_extract: 0,
            insert_faces: [true; 6],
            extract_faces: [false; 6],
        });
    }
}

/// The block item's FE buffer (`item_energy_buffer`, API 307): upstream keeps the
/// station's charge in the item's `charginggadgets:energy` component and fills
/// the block from it on placement; the pin's item-energy surface is that
/// component, so it can be filled but not drained (upstream drains it by
/// placing the item).
pub fn on_item_energy_buffer(item: &str, buffer: &mut Option<ItemEnergyBuffer>) {
    if item == STATION {
        *buffer = Some(ItemEnergyBuffer {
            capacity: CAPACITY,
            max_insert: MAX_TRANSFER,
            max_extract: 0,
        });
    }
}

// ---------------------------------------------------------------------------
// The station tick
// ---------------------------------------------------------------------------

/// One server tick (`native_block_entity_tick`, API 17): `tryBurn`, then
/// `chargeItem`, then the menu's data slots.
#[allow(
    private_interfaces,
    reason = "the injected bundle is this crate's own module"
)]
pub fn on_tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    if entity.id != STATION {
        return;
    }
    if entity.items.len() < SLOT_COUNT {
        entity.items.resize(SLOT_COUNT, None);
    }
    let mut station = Station::new();
    station.energy = read_int(&entity.nbt, KEY_ENERGY).clamp(0, CAPACITY);
    station.counter = read_int(&entity.nbt, KEY_COUNTER);
    station.max_burn = read_int(&entity.nbt, KEY_MAX_BURN);
    {
        let mut fuel = SlotFuel {
            game: &*game,
            entity: &mut *entity,
        };
        station.burn_tick(&mut fuel);
    }
    charge_item(game, &mut station, entity);
    write_int(&mut entity.nbt, KEY_ENERGY, station.energy);
    write_int(&mut entity.nbt, KEY_COUNTER, station.counter);
    write_int(&mut entity.nbt, KEY_MAX_BURN, station.max_burn);
    // `setChanged`: the chunk is saved with what this tick did.
    game.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
    let data = station.data_slots().to_vec();
    if entity.data != data {
        entity.data = data;
    }
}

/// The charge slot as `ChargingStationTile.chargeItem` reads it.
fn charge_item(game: &mut Game<'_>, station: &mut Station, entity: &mut ModBlockEntity) {
    let Some(stack) = entity.items[CHARGE_SLOT].clone() else {
        return;
    };
    // The engine's helpers act on one item of a stack, so a stack of more than
    // one is left alone (upstream keeps one charge slot value for the whole
    // stack; the menu's items are the single-item tools and batteries).
    if stack.count != 1 {
        return;
    }
    let Some(held) = game.native_item_energy(&stack) else {
        return;
    };
    let moved = station.charge_step(Some(ItemBuffer {
        energy: held.amount,
        capacity: held.capacity,
        max_insert: held.max_insert,
    }));
    if moved == 0 {
        return;
    }
    match game.native_item_energy_insert(&stack, moved) {
        Some((took, charged)) => {
            // The engine applies the same `min(room, limit, offer)` the pure
            // step did; if the two ever disagreed, hand the difference back so
            // no FE is lost.
            station.energy += moved - took;
            if took > 0 {
                entity.items[CHARGE_SLOT] = Some(charged);
            }
        }
        None => station.energy += moved,
    }
}

/// The fuel slot, as `ChargingStationTile.initBurn` reads it: the item's
/// smelting burn time and the shrink (or the crafting remainder) that takes it.
struct SlotFuel<'a, 'g> {
    game: &'a Game<'g>,
    entity: &'a mut ModBlockEntity,
}

impl FuelSlot for SlotFuel<'_, '_> {
    fn peek(&self) -> Option<u32> {
        let stack = self.entity.items.first()?.as_ref()?;
        (stack.count > 0).then(|| fuel_burn_time(self.game, stack))
    }

    fn take(&mut self) {
        let Some(Some(stack)) = self.entity.items.first_mut() else {
            return;
        };
        stack.count -= 1;
        if stack.count > 0 {
            return;
        }
        let remainder = self
            .game
            .res
            .items()
            .get(&stack.item)
            .and_then(|info| info.remainder.clone());
        self.entity.items[FUEL_SLOT] = remainder.map(|item| ItemStack::new(item.as_str(), 1));
    }
}

/// The engine's fuel value (`minecraft:cooking_fuel.burn_time`), 0 if none.
fn fuel_burn_time(game: &Game<'_>, stack: &ItemStack) -> u32 {
    let providers = game.res.providers();
    let items = game.res.items();
    providers.burn_time(items, stack, false).max(0) as u32
}

// ---------------------------------------------------------------------------
// Client side: the screen
// ---------------------------------------------------------------------------

/// The station's two slots (`native_screen`, API 19), at upstream's
/// `ChargingStationContainer` pixels in its 176 x 166 panel.
#[allow(
    private_interfaces,
    reason = "the injected bundle is this crate's own module"
)]
pub fn on_screen(block: &str, screen: &mut Option<NativeScreen>) {
    if block != STATION {
        return;
    }
    *screen = Some(NativeScreen {
        width: 176.0,
        height: 166.0,
        slots: vec![Some([65.0, 43.0]), Some([119.0, 43.0])],
        inventory_y: 84.0,
        inventory_x: 8.0,
        own_labels: false,
        own_background: false,
    });
}

/// The station's readouts (`native_screen_draw`, API 19): the buffer and the
/// burning steps, from the menu's data slots `[energy/32, capacity/32,
/// counter, maxburn]`.
#[allow(
    private_interfaces,
    reason = "the injected bundle is this crate's own module"
)]
pub fn on_screen_draw(
    game: &Game<'_>,
    block: &str,
    data: &[i32],
    batch: &mut HudBatch,
    origin: [f32; 2],
) {
    if block != STATION {
        return;
    }
    let energy = data.first().copied().unwrap_or(0) * 32;
    let capacity = data.get(1).copied().unwrap_or(0) * 32;
    let counter = data.get(2).copied().unwrap_or(0);
    let max_burn = data.get(3).copied().unwrap_or(0);
    let energy_text = format!("{energy}/{capacity} FE");
    let burn_text = if max_burn > 0 && counter > 0 {
        format!("{counter}/{max_burn}")
    } else {
        "-".to_owned()
    };
    game.native_text(
        batch,
        &energy_text,
        [origin[0] + 8.0, origin[1] + 8.0],
        0x404040,
        false,
    );
    game.native_text(
        batch,
        &burn_text,
        [origin[0] + 66.0, origin[1] + 8.0],
        0x404040,
        false,
    );
}

// ---------------------------------------------------------------------------
// Placement and tooltip
// ---------------------------------------------------------------------------

/// A charged station item fills the block it becomes
/// (`ChargingStationItem.updateCustomBlockEntityTag`), `native_place_block`
/// (API 14).
#[allow(
    private_interfaces,
    reason = "the injected bundle is this crate's own module"
)]
pub fn on_place_block(game: &mut Game<'_>, stack: &ItemStack, pos: [i32; 3]) {
    if &*stack.item != STATION || game.native_block(pos) != STATION {
        return;
    }
    let Some(held) = game.native_item_energy(stack) else {
        return;
    };
    let amount = held.amount.clamp(0, CAPACITY);
    if amount == 0 {
        return;
    }
    let _ = game.native_with_mod_block_entity(pos, |entity| {
        if entity.id == STATION {
            write_int(&mut entity.nbt, KEY_ENERGY, amount);
        }
    });
}

/// The item's tooltip (`native_tooltip`, API 12): upstream's
/// `screen.charginggadgets.energy` line, drawn only when it holds a charge.
#[allow(
    private_interfaces,
    reason = "the injected bundle is this crate's own module"
)]
pub fn on_tooltip(game: &Game<'_>, stack: &ItemStack, lines: &mut Vec<(Text, u32)>) {
    if &*stack.item != STATION {
        return;
    }
    let energy = game
        .native_item_energy(stack)
        .map_or(0, |held| held.amount.clamp(0, CAPACITY));
    if energy == 0 {
        return;
    }
    lines.push((
        Text::tr("screen.charginggadgets.energy", &[&energy, &CAPACITY]),
        0x55FF55,
    ));
}

// ---------------------------------------------------------------------------
// The `/charginggadgets` harness controls
// ---------------------------------------------------------------------------

/// The commands (`native_register_commands`, API 8). Upstream has none: they
/// exist so the harness can read and set a station without a GUI
/// (precedent: `/powah info|set`).
#[allow(
    private_interfaces,
    reason = "the injected bundle is this crate's own module"
)]
pub fn on_register_commands(commands: &mut Vec<NativeCommand>) {
    commands.push(NativeCommand::new("charginggadgets info <pos:block_pos>"));
    commands.push(NativeCommand::op(
        "charginggadgets energy <pos:block_pos> <amount:integer>",
        2,
    ));
    commands.push(NativeCommand::op(
        "charginggadgets item <pos:block_pos> <amount:integer>",
        2,
    ));
    commands.push(NativeCommand::op(
        "charginggadgets held <amount:integer>",
        2,
    ));
}

/// `/charginggadgets info|energy|item` (`native_perform_command`, API 8).
#[allow(
    private_interfaces,
    reason = "the injected bundle is this crate's own module"
)]
pub fn on_perform_command(
    game: &mut Game<'_>,
    _player: &NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    let words: Vec<&str> = command.split_whitespace().collect();
    if words.first() != Some(&"charginggadgets") {
        return;
    }
    *ran = true;
    match words.get(1).copied() {
        Some("info") => {
            let Some(pos) = parse_pos(&words[2..]) else {
                game.native_feedback(Text::literal("usage: /charginggadgets info <x> <y> <z>"));
                return;
            };
            game.native_feedback(Text::literal(readout(game, pos)));
        }
        Some("energy") => {
            let Some((pos, amount)) = parse_pos_amount(&words[2..]) else {
                game.native_feedback(Text::literal(
                    "usage: /charginggadgets energy <x> <y> <z> <amount>",
                ));
                return;
            };
            let amount = amount.clamp(0, CAPACITY);
            let changed = game
                .native_with_mod_block_entity(pos, |entity| {
                    if entity.id != STATION {
                        return false;
                    }
                    write_int(&mut entity.nbt, KEY_ENERGY, amount);
                    true
                })
                .unwrap_or(false);
            game.native_feedback(Text::literal(if changed {
                format!("Charging Station at {pos:?}: {amount}/{CAPACITY} FE.")
            } else {
                format!("{pos:?} is not a charging station.")
            }));
        }
        Some("item") => {
            let Some((pos, amount)) = parse_pos_amount(&words[2..]) else {
                game.native_feedback(Text::literal(
                    "usage: /charginggadgets item <x> <y> <z> <amount>",
                ));
                return;
            };
            match set_item_energy(game, pos, amount.clamp(0, CAPACITY)) {
                Some(energy) => game.native_feedback(Text::literal(format!(
                    "Charging Station at {pos:?}: the charge slot holds {energy}/{CAPACITY} FE."
                ))),
                None => game.native_feedback(Text::literal(format!(
                    "{pos:?} has no chargeable item in its charge slot."
                ))),
            }
        }
        Some("held") => {
            let Some(amount) = words.get(2).and_then(|word| word.parse::<i32>().ok()) else {
                game.native_feedback(Text::literal("usage: /charginggadgets held <amount>"));
                return;
            };
            match set_held_energy(game, amount.clamp(0, CAPACITY)) {
                Some(energy) => game.native_feedback(Text::literal(format!(
                    "Charging Station held item: {energy}/{CAPACITY} FE."
                ))),
                None => game.native_feedback(Text::literal("Hold a Charging Station item first.")),
            }
        }
        _ => game.native_feedback(Text::literal(
            "usage: /charginggadgets info|energy|item|held ...",
        )),
    }
}

/// The station's state, as `/charginggadgets info` prints it: the buffer, the
/// burning counter, both slots and the charge slot's item buffer.
fn readout(game: &Game<'_>, pos: [i32; 3]) -> String {
    let Some(BlockEntity::Mod(entity)) = game.world.block_entities.get(&(pos[0], pos[1], pos[2]))
    else {
        return format!("{pos:?} is not a mod block entity.");
    };
    if entity.id != STATION {
        return format!("{pos:?} is a {} not a charging station.", entity.id);
    }
    let energy = read_int(&entity.nbt, KEY_ENERGY).clamp(0, CAPACITY);
    let counter = read_int(&entity.nbt, KEY_COUNTER);
    let max_burn = read_int(&entity.nbt, KEY_MAX_BURN);
    let fuel = slot_text(entity.items.first().and_then(Option::as_ref));
    let charge = entity.items.get(CHARGE_SLOT).and_then(Option::as_ref);
    let charge_text = match charge {
        Some(stack) => format!(
            "{} x{} {}",
            stack.item,
            stack.count,
            match game.native_item_energy(stack) {
                Some(held) => format!("{}/{} FE", held.amount.clamp(0, CAPACITY), held.capacity),
                None => "no buffer".to_owned(),
            }
        ),
        None => "empty".to_owned(),
    };
    format!(
        "Charging Station at {pos:?}: {energy}/{CAPACITY} FE, burning {counter}/{max_burn} steps, fuel {fuel}, charge {charge_text}, t={}",
        game.world.game_time
    )
}

fn slot_text(stack: Option<&ItemStack>) -> String {
    match stack {
        Some(stack) => format!("{} x{}", stack.item, stack.count),
        None => "empty".to_owned(),
    }
}

/// Charge or discharge the item in the station's charge slot to `target` FE.
fn set_item_energy(game: &mut Game<'_>, pos: [i32; 3], target: i32) -> Option<i32> {
    let key = (pos[0], pos[1], pos[2]);
    let stack = match game.world.block_entities.get(&key) {
        Some(BlockEntity::Mod(entity)) if entity.id == STATION => {
            entity.items.get(CHARGE_SLOT).and_then(Option::clone)?
        }
        _ => return None,
    };
    let charged = charged_stack(game, &stack, target)?;
    game.native_with_mod_block_entity(pos, |entity| {
        if entity.id == STATION {
            entity.items[CHARGE_SLOT] = Some(charged);
        }
    })?;
    Some(target)
}

/// Charge or discharge the station item the acting player holds to `target` FE
/// (the harness's way to make a charged item, as `/mininggadgets energy` does).
fn set_held_energy(game: &mut Game<'_>, target: i32) -> Option<i32> {
    let slot = game.held_slot();
    let stack = game.stack_in_mut(slot).take()?;
    let charged = charged_stack(game, &stack, target);
    *game.stack_in_mut(slot) = Some(charged.clone().unwrap_or(stack));
    if charged.is_some() {
        game.hand_stack_replaced(slot);
        return Some(target);
    }
    None
}

/// One item of `stack` at `target` FE, through the engine's item-energy
/// helpers; none when the item is not a station item with a buffer.
fn charged_stack(game: &Game<'_>, stack: &ItemStack, target: i32) -> Option<ItemStack> {
    if &*stack.item != STATION || stack.count != 1 {
        return None;
    }
    let held = game.native_item_energy(stack)?;
    if target > held.amount {
        game.native_item_energy_insert(stack, target - held.amount)
            .map(|(_, charged)| charged)
    } else if target < held.amount {
        game.native_item_energy_extract(stack, held.amount - target)
            .map(|(_, drained)| drained)
    } else {
        Some(stack.clone())
    }
}

fn parse_pos(words: &[&str]) -> Option<[i32; 3]> {
    Some([
        words.first()?.parse().ok()?,
        words.get(1)?.parse().ok()?,
        words.get(2)?.parse().ok()?,
    ])
}

fn parse_pos_amount(words: &[&str]) -> Option<([i32; 3], i32)> {
    let pos = parse_pos(words)?;
    let amount = words.get(3)?.parse().ok()?;
    Some((pos, amount))
}

// ---------------------------------------------------------------------------
// Saved-compound reads
// ---------------------------------------------------------------------------

fn read_int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&JavaString::from(key)) {
        Some(Tag::Int(value)) => *value,
        _ => 0,
    }
}

fn write_int(nbt: &mut Compound, key: &str, value: i32) {
    nbt.insert(JavaString::from(key), Tag::Int(value));
}

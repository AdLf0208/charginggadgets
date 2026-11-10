// The station item's FE buffer, read at each call of the engine's item energy
// helpers (API 307).
fn observe_item_energy_buffer(
    item: &str,
    buffer: &mut Option<horizon_client::game::ItemEnergyBuffer>,
) {
    crate::game::charginggadgets_state::on_item_energy_buffer(item, buffer);
}

// The station's two slots and its menu (API 16/17).
fn observe_block_entity_container(
    block: &str,
    container: &mut Option<horizon_client::containers::ModContainer>,
) {
    crate::game::charginggadgets_state::on_container(block, container);
}

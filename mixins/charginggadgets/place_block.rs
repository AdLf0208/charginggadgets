// A placed station takes the charge its item carried (API 14).
fn observe_place_block(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    pos: [i32; 3],
) {
    crate::game::charginggadgets_state::on_place_block(game, stack, pos);
}

// The station item's charge line (API 12).
fn observe_tooltip(
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    lines: &mut Vec<(Text, u32)>,
) {
    crate::game::charginggadgets_state::on_tooltip(game, stack, lines);
}

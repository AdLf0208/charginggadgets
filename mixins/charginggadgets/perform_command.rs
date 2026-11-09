// Run `/charginggadgets ...` for the acting player (API 8).
fn observe_perform(
    game: &mut crate::game::Game<'_>,
    player: &NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::charginggadgets_state::on_perform_command(game, player, command, ran);
}

// Declare `/charginggadgets info|energy|item` (API 8).
fn observe_register(commands: &mut Vec<NativeCommand>) {
    crate::game::charginggadgets_state::on_register_commands(commands);
}

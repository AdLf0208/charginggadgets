// The station menu's two slots (API 19).
fn observe_screen(block: &str, screen: &mut Option<NativeScreen>) {
    crate::game::charginggadgets_state::on_screen(block, screen);
}

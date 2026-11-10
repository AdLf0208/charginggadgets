# Licence notice — Charging Gadgets

This port carries the original mod's assets and data, and rewrites its behaviour
in Rust (or, for a data-tier port, declares its blocks from the same data). The
original licence and copyright notices are reproduced verbatim in
[LICENSE](LICENSE).

## Where the licence was read

- The pack jar `charginggadgets-1.14.1.jar` (kept at `licence-cache/`)
  declares `license="MIT (https://github.com/Direwolf20-MC/ChargingGadgets/blob/master/LICENSE.md)"`
  in `META-INF/neoforge.mods.toml` and carries no licence file.
- The MIT text is reproduced from the licence URL the jar names. The copyright
  line uses the jar's own `authors` field (`Direwolf20, ErrorMikey`), because the
  jar ships no copyright line and no local upstream checkout was kept.

## The port

- Original: Charging Gadgets 1.14.1 by Direwolf20 and ErrorMikey — MIT.
- Licence: **MIT**
- Port to Horizon by AdLf0208: the Rust runtime, the Horizon declarations and the
  build/verification tooling in this repository.
- No Mojang file is included. The carried files are the mod's own assets and
  data; `data/minecraft/tags/…` files only add this mod's ids to vanilla tags.

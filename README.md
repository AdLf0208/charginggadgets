# Charging Gadgets — Horizon port

A port of [Charging Gadgets](https://github.com/Direwolf20-MC/ChargingGadgets)
by Direwolf20 and ErrorMikey for Horizon. This slice is the charging station: a
machine that burns fuel into a 1,000,000 FE buffer and charges the gadget in its
slot.

- Right-click the station to open its two-slot menu; fuel goes in one slot and
  the gadget to charge in the other. The station's block item keeps its charge.
- One coal burns for 32 steps of 625 FE (20,000 FE); modded fuels use their own
  burn time, and the crafting remainder is honoured.
- The station accepts FE from cables into the same buffer and never lets it be
  drained; 2,500 FE a tick goes into the item.

The rest of the mod is a planned gap list. MIT — original copyright and licence
notices are kept in the port repository; see LICENSE and LICENSE-NOTICE.md
there.

**Original:** [Charging Gadgets](https://github.com/Direwolf20-MC/ChargingGadgets) 1.14.1 by
Direwolf20, ErrorMikey, **MIT**. The original
copyright and licence notices are kept in [LICENSE](LICENSE), and
[LICENSE-NOTICE.md](LICENSE-NOTICE.md) records where each was read from.

**Horizon:** a port for [Horizon](https://github.com/horizon-rust/horizon), Minecraft:
Java Edition 26.3 rewritten in Rust. The exact Horizon and horizon-mixins
revisions this port builds against are pinned in `compatibility.json`.

## Install

1. Take `charginggadgets.hnmod` from this repository (or build it below).
2. Put it in the game folder's `horizon-native-mods` folder — the game folder is the one
   holding `options.txt`.
3. Native mods are always on once installed; the first install shows a
   trust notice, and the launcher builds the mod for your game.

To try it without installing, start the game with
`--dev-mod charginggadgets.hnmod`.

## Build

The port is a native mod (`.hnmod`) with a content half.

```sh
cargo test --locked --offline                        # the pure runtime rules
python3 tools/horizon.py stage   --horizon <horizon-checkout>
python3 tools/horizon.py check   --horizon <horizon-checkout>
python3 tools/horizon.py test    --horizon <horizon-checkout>
python3 tools/horizon.py build   --horizon <horizon-checkout>   # rebuild charginggadgets.hnmod
```

`tools/e2e.sh` is the in-game harness. It expects a campaign checkout
(`$CAMPAIGN_ROOT`), the shared pins under `ports/_shared/pins/`, and the
prepared 26.3 resources (`$HORIZON_RES`, default `.local/horizon-client/26.3`).

## Layout

`content/` is the shipped content; `mixins/charginggadgets/` holds the declarations and the runtime; `tools/` builds and verifies the port.

This is an unofficial port: Horizon is not affiliated with the original author.
The licence above is the original's and travels with every copy; see
[LICENSE](LICENSE) and [LICENSE-NOTICE.md](LICENSE-NOTICE.md).

#!/usr/bin/env python3
"""Assemble the slice-1 `content/` folder from the pack's Charging Gadgets jar.

The jar is the ATM10 file (`ports/_shared/licence-cache/charginggadgets-1.14.1.jar`,
SHA-1 `3208ea2c…b357`), kept as the licence and provenance evidence. Nothing is
invented: the blockstate, models, textures, lang file, loot table, recipe,
advancement and the pickaxe tag come out of the jar, re-spelled only where 26.3
differs (the carry pipeline's own upgrades: `{"tag": T}` → `"#T"`, a loot
`conditions` list → one `condition`). The block/menu/tab declarations and the
four `c:` tags the recipe names are written here, because the carry guesses
`minecraft:carved_pumpkin` for a `facing`-only block and writes no convention
tags without an oracle.

Rerun it to rebuild `content/` (it deletes its own generated folders first):

    python3 tools/assemble_content.py
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import shutil
import zipfile

ROOT = Path(__file__).resolve().parents[1]
JAR = ROOT.parent / "_shared/licence-cache/charginggadgets-1.14.1.jar"
CONTENT = ROOT / "content"
MOD = "charginggadgets"
# The pack's Modrinth SHA-1 for charginggadgets-1.14.1.jar.
JAR_SHA1 = "3208ea2ca10d786895d03c2cd3a124d14393b357"

# Carried byte-for-byte: the jar's own assets.
CARRIED_ASSETS = [
    "assets/charginggadgets/blockstates/charging_station.json",
    "assets/charginggadgets/models/block/charging_station.json",
    "assets/charginggadgets/models/item/charging_station.json",
    "assets/charginggadgets/textures/block/charging_station_bottom.png",
    "assets/charginggadgets/textures/block/charging_station_frontoff.png",
    "assets/charginggadgets/textures/block/charging_station_fronton.png",
    "assets/charginggadgets/textures/block/charging_station_side.png",
    "assets/charginggadgets/textures/block/charging_station_top.png",
    "assets/charginggadgets/textures/gui/charging_station.png",
    "assets/charginggadgets/lang/en_us.json",
    "data/charginggadgets/loot_table/blocks/charging_station.json",
    "data/charginggadgets/recipe/charging_station.json",
    "data/charginggadgets/advancement/recipes/redstone/charging_station.json",
    "data/minecraft/tags/block/mineable/pickaxe.json",
]

# 26.3's item model definition: the jar's own `models/item` file, named by it.
ITEM_DEFINITION = {"model": {"type": "minecraft:model", "model": f"{MOD}:item/charging_station"}}

# The station is `new ChargingStationBlock()`: `Properties.of().strength(2f)`,
# a horizontal `facing` (placed with `getHorizontalDirection().getOpposite()`)
# and the default stone sound. `minecraft:stonecutter` is vanilla's only
# template with exactly one 4-value `facing` state whose class opens a menu on
# use (`Class::Stonecutter`), which is what a mod container's menu needs; the
# template's own saw shape is overridden back to the full cube.
BLOCK = {
    "template": "minecraft:stonecutter",
    "properties": {
        "hardness": 2.0,
        "explosion_resistance": 2.0,
        "requires_tool": False,
        "sounds": "minecraft:stone",
        "occludes": True,
    },
    "shapes": {"default": [[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]]},
    "collision_shapes": {"default": [[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]]},
}

# `ChargingStationContainer`: slots 0 and 1, and `ContainerData`'s four values
# (energy/32, capacity/32, counter, maxburn).
MENU = {"slots": 2, "data_slots": 4}

# Upstream's own item group (`itemGroup.charginggadgets`).
TAB = {
    "title": "itemGroup.charginggadgets",
    "icon": f"{MOD}:charging_station",
    "items": [f"{MOD}:charging_station"],
}

# The jar's recipe names `c:` convention tags the prepared 26.3 root does not
# ship; an unknown tag expands to an empty ingredient, so they are written.
CONVENTION_TAGS = {
    "gems/lapis": ["minecraft:lapis_lazuli"],
    "gems/diamond": ["minecraft:diamond"],
    "ingots/iron": ["minecraft:iron_ingot"],
    "storage_blocks/coal": ["minecraft:coal_block"],
    "dusts/redstone": ["minecraft:redstone"],
}

MANIFEST = f"""# Port-owned content mod for the Charging Gadgets 1.14.1 slice: the charging
# station block, its menu, and the carried assets/data. The models, textures,
# blockstate, lang file, loot table and recipe are upstream's own MIT assets,
# read from the exact ATM10 jar (see ../../_shared/carry-work/charginggadgets/SOURCE.md).
id = "{MOD}"
name = "Charging Gadgets"
version = "1.14.1"
api = "0.2"
side = "both"
capabilities = ["registries"]
"""


def upgrade_recipe(recipe: dict) -> dict:
    """The carry pipeline's 1.21.1 → 26.3 ingredient upgrade."""
    for key, value in list(recipe.get("key", {}).items()):
        if isinstance(value, dict) and "tag" in value:
            recipe["key"][key] = f"#{value['tag']}"
        elif isinstance(value, dict) and "item" in value:
            recipe["key"][key] = value["item"]
    return recipe


def upgrade_loot(loot: dict) -> dict:
    """The carry pipeline's loot upgrade: one `condition` object per pool."""
    for pool in loot.get("pools", []):
        conditions = pool.pop("conditions", None)
        if conditions:
            condition = dict(conditions[0])
            condition["type"] = f"minecraft:{condition.pop('condition').split(':')[-1]}"
            pool["condition"] = condition
    return loot


def write(path: Path, data) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(data, bytes):
        path.write_bytes(data)
    else:
        path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")


def main() -> None:
    raw = JAR.read_bytes()
    digest = hashlib.sha1(raw).hexdigest()
    if digest != JAR_SHA1:
        raise SystemExit(f"jar SHA-1 {digest} is not the pack record {JAR_SHA1}")
    archive = zipfile.ZipFile(JAR)
    for folder in ("assets", "data"):
        shutil.rmtree(CONTENT / folder, ignore_errors=True)

    for name in CARRIED_ASSETS:
        data = archive.read(name)
        target = name
        if name.endswith("loot_table/blocks/charging_station.json"):
            data = (json.dumps(upgrade_loot(json.loads(data)), indent=2) + "\n").encode()
        elif name.endswith("recipe/charging_station.json"):
            data = (json.dumps(upgrade_recipe(json.loads(data)), indent=2) + "\n").encode()
        write(CONTENT / target, data)

    write(CONTENT / f"assets/{MOD}/items/charging_station.json", ITEM_DEFINITION)
    write(CONTENT / f"data/{MOD}/horizon/block/charging_station.json", BLOCK)
    write(CONTENT / f"data/{MOD}/horizon/menu/charging_station_menu.json", MENU)
    write(CONTENT / f"data/{MOD}/horizon/creative_tab/charging_gadgets.json", TAB)
    for tag, values in CONVENTION_TAGS.items():
        write(CONTENT / f"data/c/tags/item/{tag}.json", {"values": values})
    (CONTENT / "horizon-mod.toml").write_text(MANIFEST)

    files = sorted(p for p in CONTENT.rglob("*") if p.is_file() and p.name != "README.md")
    print(f"{len(files)} files under {CONTENT}")
    for path in files:
        print(" ", path.relative_to(CONTENT))


if __name__ == "__main__":
    main()

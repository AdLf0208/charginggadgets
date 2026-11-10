#!/usr/bin/env bash
# In-game check for Charging Gadgets slice 1: the charging station burns fuel
# into FE and fills the FE buffer of the item in its charge slot.
#
# Every mod command is typed with `ctl chat "/cmd"` because `ctl command` runs
# the console path and bypasses the native `perform_command` hook.
#
# The End, in the void at x=200 (away from the central island, its dragon egg
# and the ambient item entities of a generated level). Every station stands at
# x=202 on the stone floor, spaced 4 apart in z so a station never stands
# between the player and the next one's target face.
#
# Default run:
#  A  an empty station placed from its item reads 0/1,000,000 FE;
#  B  a non-fuel item in the fuel slot makes no FE and is not used up;
#  C  an item in the charge slot of an empty station takes nothing;
#  D  a full buffer fills a 999,000 FE item to 1,000,000 and is debited exactly
#     1,000 (the item's capacity is the stop, not the 2,500 a tick);
#  E  one coal is 32 burn steps: exactly 20,000 FE;
#  E2 a full item beside a burning station takes nothing;
#  F  a fatter station charges a discharged item at 2,500 FE a tick: the item's
#     charge is exactly 2,500 x the ticks between the two reads, and the station
#     loses exactly what the item gained (FE is moved, never copied);
#  G  a non-chargeable item in the charge slot takes nothing;
#  H  a station item carrying a charge fills the block it is placed as
#     (upstream's `updateCustomBlockEntityTag`).
#
# Default run (EMBEDDED=0): the game loads `content/` as a dev mod.
# EMBEDDED=1: the game is launched with **only the native bundle** (no
# `--dev-mod`) and the content comes from the bundle's embedded copy
# (`mixins/<id>/data/…`, the AE2 pattern). The run also checks the file the
# helper wrote, and CONTROL=1 (default with EMBEDDED) first launches the same
# binary with `.horizon-cache` unusable, where the ids must be unknown.
#
# SELFTEST=1 flips E's 20,000 assertion (it must FAIL), proving the scenario can
# fail.
set -Eeuo pipefail
export LC_ALL=C
[ -z "${CG_DEBUG:-}" ] || set -x

EMBEDDED=${EMBEDDED:-0}
CONTROL=${CONTROL:-$EMBEDDED}

CLONE=${CAMPAIGN_ROOT:-$(cd "$(dirname "$0")/../../.." && pwd)}
PORT=$CLONE/ports/charginggadgets
PIN_BIN=$CLONE/ports/_shared/bin/horizon-1d63e0a5
CTL=${CTL:-$PIN_BIN/horizon-ctl}
GAME=${GAME:-$PORT/target/horizon-cargo/fast/horizon-client}
RES=${RES:-${HORIZON_RES:-${HORIZON_RES:-$HOME/.local/horizon-client/26.3}}}
SOCK=${SOCK:-/tmp/opencode/charginggadgets-e2e.sock}
SELFTEST=${SELFTEST:-0}
KEEP=${KEEP:-0}
WORLD=ChargingGadgetsE2E
CONTROL_WORLD=ChargingGadgetsControl
# Where the arena stands: the End's void, far from the central island.
X0=200
SX=$((X0 + 2))
CAPACITY=1000000
CHARGE_PER_TICK=2500

[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }

SCRATCH=$(mktemp -d /tmp/opencode/charginggadgets-home.XXXXXX)
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")" "$XDG_CONFIG_HOME"
rm -f "$SOCK"

ctl() { "$CTL" --socket "$SOCK" "$@"; }
say() { ctl chat "/$1" >/dev/null; }
fail() { echo "FAIL: $*" >&2; ctl logs --lines 60 >&2 || true; exit 1; }
on_error() { echo "FAIL: line $1: '$2' exited $?" >&2; ctl logs --lines 40 >&2 || true; }
trap 'on_error "$LINENO" "$BASH_COMMAND"' ERR

cleanup() {
    local status=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    if [ "$KEEP" = 1 ]; then
        echo "kept scratch: $SCRATCH" >&2
    else
        rm -rf "$SCRATCH" 2>/dev/null || true
    fi
    # Last line of every run: `check-e2e-logs.py` reads it as the verdict.
    echo "EXIT:$status"
}
trap cleanup EXIT

launch() { # launch [WORLD]
    local world=${1:-$WORLD}
    rm -f "$SOCK"
    cd "$SCRATCH"
    local args=(--game "$GAME" --world "$world" --seed 20261105 --creative --headless
                --wait --resources "$RES")
    if [ "$EMBEDDED" = 1 ]; then
        echo "  launch: no --dev-mod; the bundle carries its own content" >&2
    else
        args+=(--dev-mod "$PORT/content")
    fi
    "$CTL" --socket "$SOCK" launch "${args[@]}" >/dev/null
    ctl wait-ready 120
    ctl wait 40
}

block_name() { ctl block "$1" "$2" "$3" | jq -r '.name // .block // empty'; }

# The End's void at X0: a stone floor and clear air above it. The player is
# teleported first, so the chunks the fills touch are loaded. The floor is laid
# before any station is placed: a player standing over the void falls.
arena() {
    ctl command "execute in minecraft:the_end run tp @s $X0.5 104 0.5" >/dev/null
    local loaded=false
    for _ in $(seq 1 120); do
        ctl wait 5 >/dev/null
        if [ "$(ctl block "$X0" 100 0 | jq -r '.loaded // false')" = true ]; then
            loaded=true
            break
        fi
    done
    [ "$loaded" = true ] || fail "the End arena chunk never loaded"
    ctl command "execute in minecraft:the_end run fill $((X0 - 2)) 100 -14 $((X0 + 4)) 100 14 minecraft:stone" >/dev/null
    ctl wait 6 >/dev/null
    ctl command "execute in minecraft:the_end run fill $((X0 - 2)) 101 -14 $((X0 + 4)) 108 14 minecraft:air" >/dev/null
    ctl wait 6 >/dev/null
    ctl command "execute in minecraft:the_end run kill @e[type=!player]" >/dev/null
    ctl command "execute in minecraft:the_end run tp @s $X0.5 101 0.5" >/dev/null
    ctl wait 20
    [ "$(block_name "$X0" 100 0)" = "minecraft:stone" ] || fail "the arena floor is not stone"
    [ "$(block_name "$X0" 101 0)" = "minecraft:air" ] || fail "the arena air above the floor is not air"
}

# `/charginggadgets info X Y Z`: the line, then the parsed fields in STATION,
# ITEM, T, COUNTER, MAXBURN, FUEL, CHARGE, BUFFER.
info() { # info X Y Z
    say "charginggadgets info $1 $2 $3"
    ctl wait 12 >/dev/null
    INFO_LINE=$(ctl messages 12 | jq -r '.chat[]' | grep -m1 'Charging Station at' || true)
    [ -n "$INFO_LINE" ] || fail "no /charginggadgets info reply"
    eval "$(python3 "$PORT/tools/parse_info.py" "$INFO_LINE")"
    [ "${ok:-0}" = 1 ] || fail "the info line did not parse: $INFO_LINE"
    echo "  $INFO_LINE" >&2
}

expect_station() { # expect_station AMOUNT
    [ "$STATION" = "$1" ] || fail "the station holds $STATION FE, expected $1"
}
expect_item() { # expect_item AMOUNT
    [ "$ITEM" = "$1" ] || fail "the charge slot's item holds $ITEM FE, expected $1"
}

# Put one station item in the player's first hotbar slot and select it: `give`
# may fill a main-inventory slot that `select ITEM` cannot reach.
hold_station() {
    ctl command "item replace entity @s hotbar.0 with charginggadgets:charging_station 1" >/dev/null
    ctl wait 4 >/dev/null
    ctl select 1 >/dev/null
    ctl wait 2 >/dev/null
}

# Place a station at X Y Z from the held item, and check the block appeared.
# `ctl place` places against a neighbour's visible face, so the player stands two
# blocks west and aims at the top face of the floor under the target.
place_station() { # place_station X Y Z
    local x=$1 y=$2 z=$3
    ctl command "execute in minecraft:the_end run tp @s $((x - 2)).5 $y $z.5" >/dev/null
    ctl wait 6 >/dev/null
    hold_station
    local aim reply
    aim=$(ctl look-at-block "$x" $((y - 1)) "$z" up 2>&1) \
        || fail "cannot aim at the floor under $x,$y,$z: $aim"
    reply=$(ctl place "$x" "$y" "$z" --item charginggadgets:charging_station 2>&1) \
        || fail "ctl place at $x,$y,$z failed: $(tr -d '\n' <<<"$reply" | cut -c1-300)"
    ctl wait 6 >/dev/null
    local name
    name=$(block_name "$x" "$y" "$z")
    [ "$name" = "charginggadgets:charging_station" ] || fail "the station did not place at $x,$y,$z (got '$name')"
}

# `ctl container X Y Z ITEM COUNT`: the container path fills the block's
# `inputs` in order (fuel slot 0 first, then the charge slot 1), so an item goes
# to the charge slot only when the fuel slot already holds something else.
give_to() { # give_to X Y Z ITEM
    ctl container "$1" "$2" "$3" "$4" 1 >/dev/null
    ctl wait 6 >/dev/null
}

# The control for EMBEDDED=1: the same binary (every hook is in it) with the
# embedded content made unavailable — `.horizon-cache` is a plain file, so the
# helper can neither compare nor write its content file and falls back to
# `Continue`. Both ids must then be unknown; if they resolve, the run proves
# nothing about the embedded file.
control_leg() {
    echo "== control: the same game with no embedded content file =="
    rm -rf "$SCRATCH/.horizon-cache"
    : > "$SCRATCH/.horizon-cache"
    launch "$CONTROL_WORLD"
    arena
    ctl command "item replace entity @s hotbar.0 with charginggadgets:charging_station 1" >/dev/null || true
    ctl wait 4 >/dev/null
    ctl inventory | jq -e '[.hotbar[], .main[] | select(.item == "charginggadgets:charging_station")] | length == 0' >/dev/null \
        || fail "the control game has the station item, so the content did not come from the embedded file"
    ctl command "execute in minecraft:the_end run setblock $SX 101 0 charginggadgets:charging_station" >/dev/null || true
    ctl wait 4 >/dev/null
    local name
    name=$(block_name "$SX" 101 0)
    [ "$name" = "minecraft:air" ] || fail "the control game holds $name, so the block id was known"
    [ -f "$SCRATCH/.horizon-cache" ] || fail "the control's .horizon-cache was replaced: the helper wrote its content"
    echo "  without the embedded file: no station item, the block stayed $name"
    ctl quit >/dev/null 2>&1 || true
    rm -f "$SCRATCH/.horizon-cache"
}

# EMBEDDED=1: the bundle must have written its content out at startup, byte for
# byte the delivered content mod.
embedded_content_check() {
    local cache=$SCRATCH/.horizon-cache/charginggadgets-embedded-content.hmod
    [ -f "$cache" ] || fail "the bundle did not write its embedded content to $cache"
    cmp -s "$cache" "$PORT/content/charginggadgets.hmod" \
        || fail "the embedded content differs from content/charginggadgets.hmod"
    echo "  embedded content: $(wc -c < "$cache" | tr -d ' ') B at $cache, identical to content/charginggadgets.hmod"
}

echo "== launch (The End arena, content + native bundle) =="
if [ "$CONTROL" = 1 ]; then control_leg; fi
launch
if [ "$EMBEDDED" = 1 ]; then embedded_content_check; fi
arena

ctl command "item replace entity @s hotbar.0 with charginggadgets:charging_station 1" >/dev/null
ctl wait 4 >/dev/null
ctl inventory | jq -e '[.hotbar[], .main[] | select(.item == "charginggadgets:charging_station")] | length >= 1' >/dev/null \
    || fail "the station item did not register: $(ctl inventory | jq -c '{hotbar,main}')"

echo "== A: a station placed from its item is empty =="
place_station "$SX" 101 0
info "$SX" 101 0
expect_station 0
echo "  block $(block_name "$SX" 101 0), $STATION/$CAPACITY FE"

echo "== B: a non-fuel item in the fuel slot makes no FE and is kept =="
give_to "$SX" 101 0 minecraft:dirt
info "$SX" 101 0
[ "$FUEL" = "minecraft:dirt x1" ] || fail "the fuel slot holds $FUEL, expected one dirt"
ctl wait 40 >/dev/null
info "$SX" 101 0
expect_station 0
[ "$FUEL" = "minecraft:dirt x1" ] || fail "the dirt was used up: $FUEL"
echo "  still $STATION FE after 40 ticks, fuel $FUEL"

echo "== C: an item in the charge slot of an empty station takes nothing =="
give_to "$SX" 101 0 charginggadgets:charging_station
info "$SX" 101 0
[ "$BUFFER" = 1 ] || fail "the station item declares no item energy buffer: $CHARGE"
[ "$ITEM_NAME" = "charginggadgets:charging_station" ] || fail "the charge slot holds $ITEM_NAME"
ctl wait 40 >/dev/null
info "$SX" 101 0
expect_station 0
expect_item 0
echo "  the empty station gave nothing: item $ITEM FE"

echo "== D: a full buffer fills a 999,000 FE item to its cap, debited 1,000 =="
say "charginggadgets item $SX 101 0 999000"
ctl wait 12 >/dev/null
info "$SX" 101 0
expect_item 999000
say "charginggadgets energy $SX 101 0 $CAPACITY"
ctl wait 12 >/dev/null
ctl wait 4 >/dev/null
info "$SX" 101 0
expect_item $CAPACITY
expect_station $((CAPACITY - 1000))
ctl wait 20 >/dev/null
info "$SX" 101 0
expect_item $CAPACITY
expect_station $((CAPACITY - 1000))
echo "  the item stopped at $ITEM/$ITEM_CAP, the station at $STATION FE"

echo "== E: one coal is 32 burn steps, exactly 20,000 FE =="
place_station "$SX" 101 4
ctl container "$SX" 101 4 minecraft:coal 1 >/dev/null
ctl wait 40 >/dev/null
info "$SX" 101 4
if [ "$SELFTEST" = 1 ]; then
    echo "(selftest: asserting the station holds 19,375)"
    expect_station 19375
else
    expect_station 20000
fi
[ "$COUNTER" = 0 ] || fail "the coal's counter is $COUNTER, expected 0 after 40 ticks"
echo "  one coal: $STATION FE, counter $COUNTER/$MAXBURN"

echo "== E2: three coals beside a discharged item conserve their FE =="
# Three coals keep the fuel slot occupied while the charge item is inserted, so
# the item lands in slot 1 (an empty fuel slot would take it: the first coal is
# consumed as soon as the counter loads). Whatever the station does not hold is
# in the item: 3 coal x 32 steps x 625 FE = 60,000 FE, moved between the two.
place_station "$SX" 101 -4
ctl container "$SX" 101 -4 minecraft:coal 3 >/dev/null
ctl wait 6 >/dev/null
give_to "$SX" 101 -4 charginggadgets:charging_station
ctl wait 160 >/dev/null
info "$SX" 101 -4
[ "$ITEM_NAME" = "charginggadgets:charging_station" ] || fail "the charge slot holds $ITEM_NAME, expected the station item"
[ "$ITEM" -gt 0 ] || fail "the item took no charge while the station burned"
[ $((STATION + ITEM)) -eq 60000 ] \
    || fail "the station ($STATION FE) and the item ($ITEM FE) hold $((STATION + ITEM)) FE together, expected 60,000"
echo "  three coals: station $STATION FE + item $ITEM FE = $((STATION + ITEM)) FE"

echo "== F: 2,500 FE a tick, and the station loses exactly what the item gains =="
place_station "$SX" 101 8
give_to "$SX" 101 8 minecraft:dirt
give_to "$SX" 101 8 charginggadgets:charging_station
info "$SX" 101 8
[ "$BUFFER" = 1 ] || fail "the charge slot's item declares no buffer: $CHARGE"
[ "$ITEM" = 0 ] || fail "the discharged item reads $ITEM FE, expected 0"
say "charginggadgets energy $SX 101 8 $CAPACITY"
ctl wait 12 >/dev/null
info "$SX" 101 8
STATION_BEFORE=$STATION
ITEM_BEFORE=$ITEM
T_START=$T
ctl wait 60 >/dev/null
info "$SX" 101 8
TICKS=$((T - T_START))
[ "$TICKS" -gt 0 ] || fail "no ticks passed between the two reads"
GAIN=$((ITEM - ITEM_BEFORE))
SPENT=$((STATION_BEFORE - STATION))
[ "$GAIN" -gt 0 ] || fail "the item took nothing in $TICKS ticks"
[ "$GAIN" -eq $((CHARGE_PER_TICK * TICKS)) ] \
    || fail "the item took $GAIN FE in $TICKS ticks, expected $((CHARGE_PER_TICK * TICKS)) (2,500 a tick)"
[ "$SPENT" -eq "$GAIN" ] || fail "the station lost $SPENT FE but the item gained $GAIN"
echo "  $TICKS ticks: item $ITEM_BEFORE -> $ITEM (+$GAIN), station $STATION_BEFORE -> $STATION (-$SPENT)"

echo "== G: a non-chargeable item in the charge slot takes nothing =="
place_station "$SX" 101 -8
give_to "$SX" 101 -8 minecraft:dirt
give_to "$SX" 101 -8 minecraft:stone
say "charginggadgets energy $SX 101 -8 10000"
ctl wait 12 >/dev/null
info "$SX" 101 -8
[ "$BUFFER" = 0 ] || fail "a stone stack declared an item energy buffer: $CHARGE"
[ "$ITEM_NAME" = "minecraft:stone" ] || fail "the charge slot holds $ITEM_NAME, expected stone"
STATION_BEFORE=$STATION
[ "$STATION_BEFORE" = 10000 ] || fail "the harness did not set the station's FE (reads $STATION_BEFORE)"
ctl wait 40 >/dev/null
info "$SX" 101 -8
expect_station "$STATION_BEFORE"
echo "  a stone stack took nothing: the station still holds $STATION FE"

echo "== H: a charged station item fills the block it is placed as =="
hold_station
say "charginggadgets held 999000"
ctl wait 12 >/dev/null
HELD=$(ctl messages 12 | jq -r '.chat[]' | grep -m1 'held item' || true)
echo "  $HELD"
case "$HELD" in
    *"999000/1000000 FE"*) ;;
    *) fail "the held station item was not charged: $HELD" ;;
esac
ctl command "execute in minecraft:the_end run tp @s $X0.5 101 12.5" >/dev/null
ctl wait 6 >/dev/null
ctl look-at-block "$SX" 100 12 up >/dev/null
ctl wait 2 >/dev/null
ctl place "$SX" 101 12 --item charginggadgets:charging_station >/dev/null
ctl wait 6 >/dev/null
[ "$(block_name "$SX" 101 12)" = "charginggadgets:charging_station" ] \
    || fail "the charged station item did not place"
info "$SX" 101 12
expect_station 999000
echo "  the placed block took the item's $STATION FE (upstream's keep-on-place)"

echo "PASS: an empty station charges nothing, a non-fuel item makes no FE, one coal is exactly 20,000 FE, a full buffer fills a 999,000 FE item to its cap for exactly 1,000 FE, the charge slot takes exactly 2,500 FE a tick with no FE created or lost, a non-chargeable item and an empty buffer do nothing, and a charged item fills the block it is placed as"

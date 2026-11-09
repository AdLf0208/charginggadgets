#!/usr/bin/env python3
"""Parse `/charginggadgets info`'s line into shell assignments.

`Charging Station at [0, 101, 0]: 20000/1000000 FE, burning 0/0 steps,
fuel minecraft:coal x3, charge charginggadgets:charging_station x1 0/1000000 FE, t=264`

Prints `STATION=… ITEM=… T=… COUNTER=… MAXBURN=… FUEL=… CHARGE=…` (or `ok=0`),
for the e2e to `eval`.
"""

from __future__ import annotations

import re
import sys

LINE = re.compile(
    r"^Charging Station at \[.*?\]: (?P<station>-?\d+)/(?P<capacity>\d+) FE, "
    r"burning (?P<counter>-?\d+)/(?P<maxburn>-?\d+) steps, "
    r"fuel (?P<fuel>.*?), charge (?P<charge>.*), t=(?P<t>\d+)$"
)
CHARGE = re.compile(r"(?P<item>\S+) x(?P<count>\d+)(?: (?P<energy>-?\d+)/(?P<ecap>\d+) FE)?")


def main() -> None:
    line = sys.argv[1] if len(sys.argv) > 1 else ""
    match = LINE.match(line)
    if not match:
        print("ok=0")
        return
    charge = CHARGE.match(match["charge"])
    print(f"ok=1")
    print(f"STATION={match['station']}")
    print(f"CAPACITY={match['capacity']}")
    print(f"COUNTER={match['counter']}")
    print(f"MAXBURN={match['maxburn']}")
    print(f"T={match['t']}")
    print(f"FUEL={match['fuel']!r}")
    print(f"CHARGE={match['charge']!r}")
    if charge:
        print(f"ITEM={charge['energy'] or 0}")
        print(f"ITEM_CAP={charge['ecap'] or 0}")
        print(f"ITEM_NAME={charge['item']!r}")
        print(f"BUFFER={1 if charge['energy'] is not None else 0}")
    else:
        print("ITEM=-1")
        print("ITEM_CAP=0")
        print("ITEM_NAME=''")
        print("BUFFER=0")


if __name__ == "__main__":
    main()

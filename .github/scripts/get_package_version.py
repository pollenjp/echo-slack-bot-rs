#!/usr/bin/env -S uv run --python 3.12 --script
# /// script
# dependencies = []
# ///

from pathlib import Path

import tomllib

cargo_toml = Path(__file__).parents[2] / "Cargo.toml"

if not cargo_toml.exists():
    raise FileNotFoundError(f"Cargo.toml not found in {cargo_toml}")

with open(cargo_toml, "rb") as f:
    data = tomllib.load(f)

print(data["package"]["version"])

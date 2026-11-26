# Create for Horizon (static Switch build)

This crate is the Rust port of Create 6.0.11 for Horizon, linked statically into the Switch build
of this fork. It is based on the `create-horizon-mod` port and is licensed MIT; see `LICENSE`.

The Create assets and data are **All Rights Reserved** and are not part of this repository or its
build artifacts. To play, use the `data/` and `assets/` from your own Create installation in the
native package beside the NRO (`horizon-native-mods/create/`).

The crate exposes the static entry the game registers before `main_with`:

- `BUILT`: the compiled mod's id, version and side, checked against the package beside the
  executable;
- `entry()`: the `ModEntry` with the port's `Hooks` implementation.

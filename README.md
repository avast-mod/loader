# AVaSt loader

Mod loader for Antivirus Survivors 2003 Professional. Forked from [GDPatch](https://github.com/GDPatch/GDPatch).

## What changed from upstream

- No Lua patching. Mods are content packs: a `mod.cfg` plus `data.pck` or a `data/` folder, either as a folder in `AVaSt/mods/` or as a `.avast` zip bundle.
- Mod info file is `mod.cfg` (same file the game-side API reads).
- Root folder next to the game is `AVaSt`, not `GDPatch`. Environment variables use the `AVAST_` prefix.
- Binary names are `avast.dll`, `libavast.so`, `libavast.dylib`.

## Compat

`mod.cfg` carries an optional `[compat]` section with `game_min` / `game_max` Steam build ids. The loader was released for game build 25578107 and skips mods outside their declared range. A new game version means a new loader constant plus a release.

## Installing (no binaries here)

Releases carry the loader library plus `install.sh` / `install.ps1`, never a game exe. The scripts detect the Steam install of Antivirus Survivors 2003 Professional (appid 3832490), drop the loader next to the game binary, and create `AVaSt/mods/`. See the website for the one-line install commands.

## Acknowledgements

Upstream GDPatch credits still apply: the Godot Engine, Godot RE Tools (gdsdecomp), and GDWeave. Thank you.

## License

MIT. The original `Copyright (c) 2026 GDPatch contributors` notice is kept in LICENSE, with the AVaSt fork line appended.

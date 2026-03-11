# Star Client

A Valorant stats overlay that shows player ranks, performance, and loadouts during agent select and in-game. Features a "star" indicator that highlights other Star Client users in your matches.

## Features

- In-game overlay with player stats (rank, RR, K/D, HS%, win rate, peak rank, skins)
- Star system: see which players in your match also use Star Client
- Party detection via shared match history
- "Already played with" encounter tracking
- Discord Rich Presence
- Auto-show during agent select, auto-hide in-game, hotkey toggle (F2)
- System tray with quit option
- Fully configurable columns and behavior

## Requirements

- Windows 10/11
- Valorant set to **Borderless Windowed** mode
- Rust toolchain (for building from source)

## Building

```
cargo build --release --package star-client
```

The binary will be at `target/release/star-client.exe`.

## Configuration

On first run, a config file is created at `%APPDATA%/star/star-client/config/config.toml`. Edit it to customize columns, hotkeys, and behavior. See `config.default.toml` for all options.

## Backend (Star System)

The star indicator system requires a backend server. To deploy:

```
fly launch
fly deploy
```

Or run locally:

```
cargo run --package star-backend
```

## Compliance

Star Client is compliant with Riot Games' third-party application policy:

- Does not read game memory
- Does not inject code into the game process
- Uses only official local APIs (lockfile, entitlements) and public PD/GLZ endpoints
- Overlay is a separate transparent window, not hooked into the game renderer
- Respects Riot incognito / hidden-identity flags

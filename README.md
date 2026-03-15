# CS2Shock

`CS2Shock` is a small desktop app that listens to Counter-Strike 2 Game State Integration events and sends PiShock actions when you die in a live match.

## Features

- Shock on death during live rounds
- Optional round-kill threshold that prevents the zap after you have enough kills that round
- Optional warning beep before a shock
- Optional final shock chance gate applied before every eligible shock
- Optional delayed shock that only happens if your team loses the round
- Optional beep on match start
- Optional beep on round start
- `Random` shock mode using configured min/max values
- `LastHitPercentage` mode that scales the shock based on the HP you had before death
- Built-in first-run onboarding with CS2 integration install help

## PiShock Integration

The app now uses PiShock V2 websocket auth with `Username + ApiKey` only.

- Broker connection: `wss://broker.pishock.com/v2?Username={username}&ApiKey={apikey}`
- The app keeps one long-running broker websocket alive and reuses it across commands
- Commands are sent as V2 JSON `PUBLISH` envelopes
- Owned device metadata is resolved through PiShock's HTTP API before sending commands
- No browser login flow or website session token is used

This matches the documented PiShock V2 websocket flow in the [PiShock WebSocket API documentation](https://docs.pishock.com/pishock/api-documentation/websocket-api-documentation.html) and uses PiShock's owned-device lookup from the [PiShock API documentation](https://docs.pishock.com/pishock/api-documentation/pishock-api-documentation.html).

## Requirements

- Counter-Strike 2
- A PiShock account
- A PiShock API key generated recently enough for PiShock V2 websocket usage
- Rust, if you want to build from source

## Setup

1. Launch `cs2shock.exe` or run the project from source.
2. Follow the in-app `Finish setup` prompt.
3. Let the app install `gamestate_integration_cs2shock.cfg` automatically, or use the in-app manual steps if you prefer to copy it into `game/csgo/cfg` yourself.
4. Enter your PiShock username and API key.
5. Choose one of your owned PiShock shockers from the picker.
6. Click `Test beep` once to confirm it's live.

The app starts a local listener on `http://127.0.0.1:3000/data`, which is what the Game State Integration config points to.

## Configuration

The app automatically saves the config to `cs2shock-config.json` in the current working directory. In normal packaged use, that is alongside the executable.

Current config fields:

- `shock_mode`: `Random` or `LastHitPercentage`
- `min_duration`: minimum shock duration in seconds, `0.1` to `5.0` in steps of `0.1`
- `max_duration`: maximum shock duration in seconds, `0.1` to `5.0` in steps of `0.1`
- `min_intensity`: minimum shock intensity, `1` to `100`
- `max_intensity`: maximum shock intensity, `1` to `100`
- `beep_on_match_start`: beep when a live match begins
- `beep_on_round_start`: beep when a live round begins
- `warning_beep_before_shock`: beep before sending a shock
- `warning_beep_duration`: warning beep duration in seconds, `1` to `15`
- `shock_chance`: final percent chance that an otherwise eligible shock is actually sent, `0` to `100`
- `shock_on_round_loss_only`: if enabled, a death stores a pending shock and only triggers it after round end if your team lost
- `prevent_shock_if_round_kills_reached`: if enabled, deaths are ignored once you have reached the configured kill threshold for the current round
- `round_kills_to_prevent_shock`: round kill threshold used by `prevent_shock_if_round_kills_reached`, `1` to `5`
- `username`: your PiShock username
- `selected_client_id`: the selected PiShock device ID
- `selected_shocker_id`: the selected PiShock shocker ID
- `selected_device_name`: the selected PiShock device name
- `selected_shocker_name`: the selected PiShock shocker name
- `apikey`: your PiShock API key
- `setup_dismissed`: whether the first-run setup modal was dismissed before setup completed

Default values on a fresh config:

- `min_intensity`: `1`
- `max_intensity`: `15`
- `min_duration`: `0.3`
- `max_duration`: `1.0`
- `beep_on_match_start`: enabled
- `beep_on_round_start`: disabled
- `warning_beep_before_shock`: enabled
- `warning_beep_duration`: `2`
- `shock_on_round_loss_only`: enabled
- `prevent_shock_if_round_kills_reached`: enabled
- `round_kills_to_prevent_shock`: `1`
- `shock_chance`: `50`


### Shock Modes

`Random`:
- Picks a random intensity between `min_intensity` and `max_intensity`
- Picks a random duration between `min_duration` and `max_duration` in `0.1` second steps

`LastHitPercentage`:
- Uses your health just before death as a percentage of the configured maximum
- Example: if you had `25` HP before dying and `max_intensity` is `80`, the app sends about `20` intensity
- Shock duration is sent to PiShock in milliseconds, so values like `0.3` seconds are preserved
- Very low non-zero values are rounded up to at least `0.1` seconds so the PiShock request remains valid

### Warning Beep

If `warning_beep_before_shock` is enabled, the app sends a beep first and waits for the configured warning beep duration before it sends the shock.

### Shock Chance

`shock_chance` is the final gate before an eligible shock is sent:

- `100` means the app always sends the shock
- `0` means the app never sends the shock
- values in between roll a percentage chance each time a shock survives the earlier rules

This chance still applies when `warning_beep_before_shock` is disabled, so some deaths can result in no warning beep and no shock at all.

### Round Loss Only

If `shock_on_round_loss_only` is enabled, the app does not shock immediately when you die. Instead, it remembers the death and waits for the round result:

- If your team loses, the warning beep and shock sequence runs after the round ends
- If your team wins, the deferred shock is cancelled
- Only one warning or shock sequence can trigger per round

### Round Kill Threshold

If `prevent_shock_if_round_kills_reached` is enabled, the app checks your current `round_kills` at the moment your death is detected:

- If `round_kills` is below the configured threshold, the normal shock flow still applies
- If `round_kills` is at or above the configured threshold, the death is treated as resolved and no immediate or deferred shock is created

## Build From Source

1. Clone the repository:

```bash
git clone https://github.com/VolcanoCookies/cs2shock.git
cd cs2shock
```

2. Build a release binary:

```bash
cargo build --release
```

The executable will be available at `target/release/cs2shock.exe`.

## Run From Source

```bash
cargo run
```

## Troubleshooting

If beeps work but shocks do not:

- Make sure you refreshed the owned shocker list and selected the correct PiShock shocker
- Make sure the device is online and not paused in PiShock
- Verify that your username and API key are correct
- Watch the `PiShock API` heartbeat label. The app will retry broker reconnects automatically after heartbeat/socket failures, but you can still re-focus and leave either auth field or click `Refresh shockers` to force a fresh broker warmup and device discovery.
- If you use `LastHitPercentage`, remember that dying at very low HP can produce a very small shock
- If `shock_chance` is below `100`, some otherwise eligible deaths are expected to end without a shock
- If you win the round, post-round deaths should not trigger a shock before the next round starts
- If `shock_on_round_loss_only` is enabled, deaths will not shock immediately and may be cancelled if your team wins the round
- If `prevent_shock_if_round_kills_reached` is enabled, make sure your threshold is not higher than the number of kills you expect to earn in a round
- Watch the application logs for the exact PiShock response message

If the app does not react to gameplay:

- Make sure `gamestate_integration_cs2shock.cfg` is in `game/csgo/cfg`
- Confirm the game is sending GSI data to `http://127.0.0.1:3000/data`
- Make sure you are in a live match, not warmup

## Notes

- The app only shocks on player death during live play
- The app will only trigger one warning/shock sequence per round
- If the round is already over and your team won, shocks stay disabled until the next round starts
- In round-loss-only mode, a stored death is resolved when the round winner is known
- The round-kill suppression rule is checked when the death event is detected
- `shock_chance` is applied after the optional warning beep and before the final shock is sent
- The project stores configuration in local `cs2shock-config.json`
- PiShock intensity and duration limits are enforced before sending requests
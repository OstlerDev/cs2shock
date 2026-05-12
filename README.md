<p align="center">
  <img src="https://github.com/ostlerdev/cs2shock/blob/main/assets/icon.png?raw=true" width="128" height="128" alt="cs2shock App Icon"/>
</p>

# CS2Shock

Bring real stakes to your CS2 matches! **CS2Shock** is a small app that connects Counter-Strike 2 to your PiShock, delivering a shock when you die in a live match. It also rewards you with a sound effect on every kill, so you can pair positive reinforcement with the punishment side of the loop.

## What You Need

- Counter-Strike 2
- A PiShock account and device

## Quick Start Guide

1. [Download and launch `cs2shock.exe`.](https://github.com/OstlerDev/cs2shock/releases/latest/)
2. Follow the **Setup Guide** in the app.
3. Adjust the settings to your liking.
4. Jump into a live CS2 match and try not to die!

<p align="center">
  <img src="https://github.com/ostlerdev/cs2shock/blob/main/assets/app-example.png?raw=true"  width="312" height="610" alt="cs2shock App Example"/>
</p>

## Customizing Your Experience

CS2Shock comes with several fun modes and modifiers to customize your punishment:

### Shock Modes
- **Random**: Every death is a surprise. The app picks a random intensity and duration between your configured minimum and maximum values.
- **Last Hit Percentage**: The harder you get hit, the harder you get shocked. This mode scales the shock intensity based on how much health you lost from the final blow before dying.

### Modifiers & Rules
- **Warning Beeps**: Enable this to get a warning beep from your collar right before the shock hits. You can configure how many seconds the warning lasts.
- **Russian Roulette (Shock Chance)**: Set a percentage chance (e.g., 50%) so you never know if a death will actually result in a shock. 
- **Shock Timing**: Choose whether to shock immediately on death, wait until the round ends, or wait until the round ends and only shock if your team loses.
- **Mercy Rule (Kill Threshold)**: Earn your immunity and save yourself! If you get enough kills in a round (configurable), you won't be shocked if you die later in that same round.

### Sound Rewards
Pair the punishment side with positive reinforcement. CS2Shock can play a sound effect on every kill (clicker-style) and a separate "good job" sound at the end of a round when you hit a kill threshold. Both rewards are independent of the PiShock and only need a working audio output.

<p align="center">
  <img src="https://github.com/ostlerdev/cs2shock/blob/main/assets/app-rewards-example.png?raw=true"  width="313" height="241" alt="cs2shock Rewards UI Example"/>
</p>

- **Instant Kill Reward**: Plays a short sound the moment your match kill counter goes up while you're in a live round. Defaults to a quiet `clicker.wav` for clicker-training pairing.
- **End-of-Round Reward**: At the end of a round, if your in-round kills met the configured threshold, plays a longer reward sound. Defaults to `goodpuppy1.wav`.
- **Trigger Mode**: The end-of-round reward can fire **always** when the threshold is met, or **only if your team wins** the round.
- **Volume**: Each reward has its own 0-200% volume slider in case the sound needs a boost over your game audio.
- **Custom Sounds**: Pick the bundled defaults from the dropdown, or choose your own `.wav`, `.mp3`, `.ogg`, or `.flac` file. A "Preview" button next to each picker lets you audition the sound before saving.

Rewards are gated to live rounds only, so warmup, freezetime, and intermission kills will not trigger them.

## Troubleshooting

**Why am I not getting shocked?**
- Make sure you clicked "Refresh shockers" and selected the correct PiShock device in the app.
- Ensure both your PiShock hub and device are online, connected to WiFi, and not paused on the PiShock website.
- Verify your username and API key are correct.
- If you use **Last Hit Percentage**, dying at very low HP (like 1 HP) might produce a shock too small to feel.
- Check your rules: If **Shock Chance** is below 100%, you might have just gotten lucky. If **Shock Timing** is set to trigger at round end, the shock may be delayed until the round result is known. If the **Mercy Rule** is on, you might have gotten enough kills to earn immunity!
- Check the logs for the app to see what might have happened!

**Why is the app not reacting to gameplay at all?**
- Make sure the CS2 integration was installed correctly (the app should say it's installed).
- Ensure you are playing a **live match**. The app ignores deaths during warmup!

**Why isn't my sound reward playing?**
- Confirm the reward checkbox is enabled and use the "Preview" button next to the sound picker to verify your audio output works.
- If you picked a custom file, make sure the file still exists at that path. Moving or renaming it after selection will silently fail playback (check the logs).
- Rewards are suppressed outside of live rounds, so kills during warmup or freezetime will not trigger them.

## Advanced & Technical Details

Are you a developer, or just curious about how CS2Shock works under the hood? Check out the [Technical Details (TECHDETAILS.md)](TECHDETAILS.md) for information on the PiShock V2 WebSocket integration, the raw configuration file format, and instructions on how to build the app from source.

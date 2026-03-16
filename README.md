# CS2Shock

Bring real stakes to your CS2 matches! **CS2Shock** is a small app that connects Counter-Strike 2 to your PiShock, delivering a shock when you die in a live match!

## What You Need

- Counter-Strike 2
- A PiShock account and device

## Quick Start Guide

1. Download and launch `cs2shock.exe`.
2. Follow the **Setup Guide** in the app.
3. Adjust the settings to your liking.
4. Jump into a live CS2 match and try not to die!

## Customizing Your Experience

CS2Shock comes with several fun modes and modifiers to customize your punishment:

### Shock Modes
- **Random**: Every death is a surprise. The app picks a random intensity and duration between your configured minimum and maximum values.
- **Last Hit Percentage**: The harder you get hit, the harder you get shocked. This mode scales the shock intensity based on how much health you lost from the final blow before dying.

### Modifiers & Rules
- **Warning Beeps**: Enable this to get a warning beep from your collar right before the shock hits. You can configure how many seconds the warning lasts.
- **Russian Roulette (Shock Chance)**: Set a percentage chance (e.g., 50%) so you never know if a death will actually result in a shock. 
- **Sore Loser Mode (Round Loss Only)**: If enabled, the app won't shock you immediately when you die. Instead, it waits for the round to end. If your team wins, you are spared. If your team loses, you get shocked!
- **Mercy Rule (Kill Threshold)**: Earn your immunity and save yourself! If you get enough kills in a round (configurable), you won't be shocked if you die later in that same round.

## Troubleshooting

**Why am I not getting shocked?**
- Make sure you clicked "Refresh shockers" and selected the correct PiShock device in the app.
- Ensure both your PiShock hub and device are online, connected to WiFi, and not paused on the PiShock website.
- Verify your username and API key are correct.
- If you use **Last Hit Percentage**, dying at very low HP (like 1 HP) might produce a shock too small to feel.
- Check your rules: If **Shock Chance** is below 100%, you might have just gotten lucky. If **Round Loss Mode** is on, you only get shocked if your team loses the round. If the **Mercy Rule** is on, you might have gotten enough kills to earn immunity!
- Check the logs for the app to see what might have happened!

**Why is the app not reacting to gameplay at all?**
- Make sure the CS2 integration was installed correctly (the app should say it's installed).
- Ensure you are playing a **live match**. The app ignores deaths during warmup!

## Advanced & Technical Details

Are you a developer, or just curious about how CS2Shock works under the hood? Check out the [Technical Details (TECHDETAILS.md)](TECHDETAILS.md) for information on the PiShock V2 WebSocket integration, the raw configuration file format, and instructions on how to build the app from source.

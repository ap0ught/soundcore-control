# AeroClip (A3388) support notes

How the soundcore AeroClip (model number **A3388**) was brought up in this fork,
what was learned about the hardware, and what remains. All observations below
were verified against real hardware: one AeroClip on firmware **01.49**
(serial `3388880E85935BB8`), paired to a CachyOS (Arch) laptop over BlueZ 5.87.

## Upstream dependency

Protocol support comes from [OpenSCQ30](https://github.com/Oppzippy/OpenSCQ30),
which gained the AeroClip in `feat(lib): add support for soundcore a3388`
(`fc761af`, PR #380, Sep 2026) as `soundcore::a3388` / `DeviceModel::SoundcoreA3388`.

`Cargo.toml` pins `openscq30-lib`/`openscq30-i18n` to our fork
(`github.com/ap0ught/OpenSCQ30`, branch `fix/case-battery-255-unavailable`),
one commit ahead of upstream master (`549e3d7`). The extra commit is a real
bug fix found during live testing here (see "Case battery 0xFF" below), with a
regression test built from a packet captured from these buds. Drop back to
upstream once the fix lands there.

## Bluetooth identity: two names

The same buds report different names depending on state:

| When | BlueZ `Name` / `Alias` |
|---|---|
| Seen in a discovery scan, never connected | `soundcore AeroClip` |
| After BlueZ resolves the remote name (sticks permanently) | `soundcore AeroClip LE` |

Matching is exact (case-insensitive), so `assets/devices.toml` lists the stable
post-connect name as `bluetooth_name` and the scan name under the optional
`alt_bluetooth_names` key (added for this; see `src/devices.rs`). The `--watch`
auto-launch path (`matches_known_profile`) honors both names too.

When connected, BlueZ also reports `Battery Percentage: 0x50 (80)` and the
Soundcore vendor RFCOMM service `0cf12d31-fac3-4553-bd80-d6832e7b3388`
(note the `3388`), which is the channel OpenSCQ30 drives.

## Capability profile (from the live state packet)

A state-update dump (`Command([1, 1])`, 67-byte body — full bytes in the
regression test `out_of_case_packet_has_no_case_battery` in our OpenSCQ30 fork)
shows firmware `01.49` on both sides and:

- **No ANC hardware.** No `AmbientSoundMode` / `ManualNoiseCanceling` /
  `ManualTransparency` settings exist, so with `supports_manual_ambient_ranges
  = false` the app hides the Ambient tab (`has_ambient_options`) and skips the
  startup level repair. The tray likewise omits its ambient-mode submenu for
  such devices (`TrayState::has_ambient_options`, see `src/tray.rs`), since
  offering the switch would write a setting the device doesn't have.
- **Equalizer:** 22 presets (Soundcore Signature, Acoustic, Bass Booster, …,
  Treble Reducer) plus an 8-band custom EQ (100 Hz–12.8 kHz, ±6 dB, 0.1 dB
  steps), dual channel.
- **Buttons:** double-press and triple-press per side only (e.g. Play/Pause,
  Previous/Next Song). No long-press or slide slots — the app only renders
  controls the device actually registers, so nothing extra appears.
- **Battery:** left/right levels read fine (`8/10` at test time, matching
  BlueZ). Case level reads `—` while the buds are out of the case (next
  section). Put the buds in the case to see it report.
- **Daily controls:** upstream exposes Touch Tone, Low Battery Prompt and Dual
  Connections toggles for this model, but they are not in this app's
  `DAILY_CONTROLS` list (`src/domain.rs`), so the Controls tab currently shows
  buttons only. Wiring those three in is the obvious next step.

## Case battery 0xFF (bug found live, fixed in our OpenSCQ30 fork)

With the buds out of the case, the case-battery byte parses as `255` (0xFF =
unavailable — the same sentinel `DualBatteryLevelSettingHandler` already
handles for missing sides). `CaseBatteryLevelSettingHandler::get_inner` added
the level offset to the raw `u8`: `255 + 1` **panics on overflow in debug
builds** (killing the device worker on connect) and wraps to a bogus `0/10`
in release builds. The fix returns no setting for `255`, so the UI shows `—`,
mirroring the dual-battery handler. Upstream `Liberty 4 Pro` and every other
model using this shared handler had the same latent crash.

## APK forensics (soundcore app v5.0.31, for the record)

Largely superseded by the OpenSCQ30 discovery above, but recorded for
provenance: the APK is Flutter-based — app logic lives in
`lib/arm64-v8a/libapp.so` (~21 MB), not `classes.dex` (~300 KB of stubs).
`A3388`/`AeroClip` each appear once in `libapp.so`'s string table. The tray
icon (`assets/icons/aeroclip.png`, 480×480 RGBA) was converted from the app's
own `flutter_assets/images/a3388_home_device_line.webp`.

## Verification

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`: clean.
- `cargo test --all-targets`: 18/18 pass, including new tests for alternate-name
  matching (`devices::tests::matches_alternate_bluetooth_name`) and tray
  ambient gating (`tray_without_ambient_options_hides_listening_mode_and_menu`).
- OpenSCQ30 fork: `cargo test -p openscq30-lib --lib devices::soundcore::a3388`
  4/4 pass, including the new out-of-case regression test.
- Live (`--tray-only`, buds connected): RFCOMM connects, full snapshot reads,
  no panic; L/R 80%, EQ presets + custom bands present, no ambient settings.
- Failure modes seen along the way: `br-connection-refused` while the phone's
  soundcore app still held the buds' control channel (fix: turn off phone
  Bluetooth), and the 0xFF panic above.

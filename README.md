<div align="center">

# Horizon NX

**Minecraft: Java Edition 26.3 on the Nintendo Switch, running natively.**<br>
No Java, no emulation, no streaming: [Horizon](https://github.com/horizon-rust/horizon), the game rewritten from scratch in Rust, as Switch homebrew.

<p>
  <img src="https://img.shields.io/badge/Minecraft-Java%2026.3-5D9C3F" alt="Minecraft Java 26.3">
  <img src="https://img.shields.io/badge/platform-Nintendo%20Switch%20(homebrew)-E60012" alt="Nintendo Switch homebrew">
  <img src="https://img.shields.io/badge/status-playable,%20experimental-F0A030" alt="Playable, experimental">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-LGPL--3.0--or--later-3B2F7E" alt="LGPL-3.0-or-later license"></a>
</p>

**[Download](https://github.com/KoHaku743/horizon-nx/releases/latest)** ·
**[Install](#install)** ·
**[Companion app](https://github.com/KoHaku743/horizon-nx-companion)** ·
**[Troubleshooting](#troubleshooting)** ·
**[Building](#building)**

</div>

> [!IMPORTANT]
> **Unofficial.** Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft.
> This repository contains no Minecraft code, assets or game files: you need your own copy of Minecraft: Java
> Edition. Running homebrew requires a Switch with custom firmware; do it at your own risk.

> [!NOTE]
> **This fork** ([kejo212/horizon-nx](https://github.com/kejo212/horizon-nx)) adds **Create 6.0.11** to
> the Switch build: the Rust port ([`crates/horizon-create`](crates/horizon-create)) is linked
> statically into `horizon.nro`, so it runs without a separate launcher or mod loader.
> Create's assets and data are **All Rights Reserved** and are **not** included here: copy your own
> `data/` and `assets/` into `switch/horizon/horizon-native-mods/create/` beside the NRO.
> Everything else is upstream Horizon NX
> ([KoHaku743/horizon-nx](https://github.com/KoHaku743/horizon-nx)), LGPL-3.0-or-later.

## Features

- **The real Java Edition.** Single player worlds, and multiplayer on Java servers, online-mode ones included
  (Hypixel tested), with **Microsoft sign-in** on the console.
- **Splitscreen.** Up to four players on one Switch: a Pro Controller or a pair of Joy-Con each, set up on the
  Switch's own controller screen.
- **Smooth in handheld.** Up to 60 fps, with the GPU at 460.8 MHz (a speed the Switch offers games) and faster
  still while splitscreen draws the world twice.
- **Switch controls.** Joy-Con and Pro Controller, the touch screen (menus, keyboard, tap to attack), gyro aiming
  and HD rumble with its own strength slider.
- **Mods and resource packs.** Horizon's mod registry in the game (Options → Mods → Get Mods), content mods switched
  on and off without a restart, and server resource packs (Hypixel SkyBlock tested).
- **Fast loading.** About 15 seconds to the title screen, the game files packed into one file on the SD card.
- **Updates in the game.** New releases show up on the title screen.

## Status

Tested on a Switch OLED with Atmosphère, November 2026. Current release: **v2.2.1**.

| | |
|---|---|
| Single player | ✅ Create and play worlds |
| Multiplayer | ✅ Online-mode servers included (sign in first) |
| Microsoft sign-in | ✅ On the title screen (approved by Mojang) |
| Splitscreen | ✅ Up to 4 players, a Pro Controller or a Joy-Con pair each |
| Controls | ✅ Joy-Con, Pro Controller, touch screen, gyro aiming (off by default), HD rumble |
| HOME button | ✅ With Nintendo's HOME menu ([custom HOME menus](#the-console-freezes-when-i-press-home) can freeze) |
| Horizon mods (`.hmod`) and resource packs | ✅ From Get Mods, or dropped in `switch/horizon/horizon-mods/`. Mods that run code (WebAssembly or native) can't run on the Switch, and many registry mods need a newer Horizon: Get Mods says which |
| Server resource packs | ✅ Under a minute the first time, about 7 s after that |
| Clickable chat | ✅ Server buttons and links: open the chat and tap |
| In-game updates | ✅ From this repository's releases |
| Frame rate | ✅ Up to 60 fps in handheld mode |
| Loading | ✅ About 15 s to the title screen (the very first start is slower, [see below](#4-start-it)) |
| Docked mode | ❔ Not tested yet |

## What you need

- A Nintendo Switch running **[Atmosphère](https://github.com/Atmosphere-NX/Atmosphere)** custom firmware.
- About **1.5 GB free** on the SD card.
- A computer (Windows, macOS or Linux) to prepare the game files, and **your own copy of Minecraft: Java Edition**.
- For online servers: the Microsoft account that owns Minecraft: Java Edition.

## Install

### The easy way: Horizon NX Companion

**[Horizon NX Companion](https://github.com/KoHaku743/horizon-nx-companion)** is an app for your computer (macOS,
Windows, Linux) that sets up the SD card for you:

- copies the prepared game files and **packs `resources.hzpak` on the computer**, so the Switch doesn't spend about
  20 minutes on it at the first start;
- installs `horizon.nro` from this repository's releases;
- installs mods and resource packs from Horizon's registry;
- copies worlds, mods and servers between the Switch and your computer, backs up worlds, reads the logs, frees up
  space and cleans macOS's `._` files.

Download it from its [releases](https://github.com/KoHaku743/horizon-nx-companion/releases/latest) and put the SD
card in the computer (or use hekate's **Tools > USB Tools > SD Card**). You still need the Horizon launcher from
[step 2](#2-prepare-the-game-files-on-your-computer) once, to prepare the files from your own Minecraft. Then go
to [step 4](#4-start-it).

The steps below do the same by hand.

### 1. Get the Switch build

Download `horizon-switch-….zip` from the [latest release](https://github.com/KoHaku743/horizon-nx/releases/latest)
and unzip it to the root of the SD card. It contains:

```
switch/horizon/horizon.nro
switch/horizon/client.txt
```

`client.txt` makes `horizon.nro` start the game; without it, it starts a dedicated server.

### 2. Prepare the game files on your computer

Horizon reads its data from your own Minecraft, converted once by Horizon's tools:

1. Install the **Horizon launcher** from [playhorizon.dev](https://playhorizon.dev), sign in with your Microsoft
   account, and press **Play** once. It downloads Minecraft 26.3 for your account and prepares it.
2. Find the prepared folder: it's the folder that contains `horizon-resources.json`.
   - macOS / Linux: `find ~ -name horizon-resources.json 2>/dev/null`
   - Windows (PowerShell): `Get-ChildItem $env:USERPROFILE -Recurse -Filter horizon-resources.json -ErrorAction SilentlyContinue`

### 3. Copy everything to the SD card

```
SD card
└── switch/
    └── horizon/
        ├── horizon.nro
        ├── client.txt
        └── resources/        ← the contents of the prepared folder from step 2
            ├── horizon-resources.json
            ├── assets/
            ├── data/
            └── …
```

Copy the *contents* of the prepared folder into `switch/horizon/resources/`, not the folder itself.

> [!NOTE]
> **Updating from 1.x?** Version 2.0 needs game files prepared by a Horizon launcher from November 2026 or later:
> prepare them again and replace `resources/` (and delete `resources.hzpak`), or a world won't open.

> [!WARNING]
> **On a Mac**, Finder adds hidden `._` files next to every file it copies to the SD card, and the game can't read
> them. Copy over FTP or MTP, or clean the card before ejecting it:
>
> ```sh
> dot_clean -m "/Volumes/YOUR SD CARD"
> ```

### 4. Start it

Horizon needs the memory of a full application, so **don't start it from the Album**. Hold **R** while starting
any game to open the homebrew menu as an application, then choose Horizon. (A title override or a forwarder works
too.)

The first start is slow: the graphics driver compiles the game's shaders and keeps them on the SD card, and the game
packs `resources/` into one file, `resources.hzpak` (about 520 MB), in the background (about 20 minutes; keep playing
or leave it at the title screen; with the companion app the pack is already there). From the next start it loads in
about 15 seconds. Keep the `resources` folder: the game still reads its largest files from it.

## Playing

### Microsoft account

On the title screen, **Sign In** opens the Microsoft Account screen:

1. Press **Sign In**. The Switch shows a web address and a code.
2. Open the address on your phone or computer, enter the code, and sign in with the Microsoft account that owns
   Minecraft: Java Edition.
3. The Switch finishes on its own and shows your Minecraft name.

The sign-in is kept in `switch/horizon/account.json`, so you stay signed in after a restart. **Anyone with your SD
card can use that file to sign in as you** until you press **Sign Out**, which deletes it. A world you played before
under the offline name starts you as a new player (as in Minecraft itself); the old character stays saved.

### Splitscreen

1. Turn it on in **Options → Horizon Settings → Splitscreen**, and open **Setup**: the Switch's controller screen
   opens, where you connect and order the controllers (Setup's **Controllers** button opens it again).
2. In a world, press a button on the second controller and **A** to join, then pick a name.
3. In Setup, the square button in the bottom row changes the layout (side by side, or top and bottom), and picking
   a player up moves them to the other half.

Each player needs a Pro Controller or a pair of Joy-Con: a single sideways Joy-Con has one stick, which can't both
move and look around. Each player's rumble plays on their own controller.

### Clocks and battery

In handheld mode Horizon runs the GPU at **460.8 MHz**, a speed the Switch offers games (the system's default is
307.2 MHz). While splitscreen has another player in handheld mode it runs at **614.4 MHz**: an overclock like
sys-clk's, which uses more battery and runs warmer, and goes back down when you play alone again. Docked, the
system's 768 MHz stays. Both can be changed in [`env.txt`](#settings-envtxt); turn sys-clk off while playing, or it
and the game keep setting different clocks.

## Settings: `env.txt`

A text file `switch/horizon/env.txt`, next to `horizon.nro`, sets options at startup, one `NAME=value` per line
(`#` starts a comment). Nothing in it is needed to play.

| Setting | What it does |
|---|---|
| `HORIZON_NX_GPU_CLOCKS=system` | The Switch's own handheld GPU speed (307.2 MHz) instead of 460.8 MHz. `max` also raises the memory to 1600 MHz. |
| `HORIZON_NX_SPLIT_GPU_MHZ=…` | Splitscreen's handheld GPU speed in MHz (614.4 by default); `0` leaves the GPU at its normal speed. |
| `HORIZON_NX_CPU_BOOST=0` | Keep normal CPU clocks while loading (they're raised by default). |
| `HORIZON_NX_MEMORY_PARTS=SECONDS` | How often `log.txt` gets what holds the memory, by part (default 120, `0` for never). |
| `HORIZON_NX_THREAD_LOG=1` | Log every new thread in `log.txt`, with who created it (slower: each line is an SD card write). |
| `HORIZON_NX_SRV=0` | Don't look up a server's SRV record: join the address as typed. |
| `HORIZON_MSA_CLIENT_ID=…` | A different Azure app for the Microsoft sign-in. |
| `HORIZON_NX_PACK=0` | Read the resources folder file by file instead of `resources.hzpak` (slow; only to rule the pack out). |
| `HORIZON_NX_LINKS=0` | Turn off the SD card links that resource packs and mods need (only to rule them out). |
| `MESA_SHADER_CACHE_DISABLE=true` | Compile every shader at every start instead of keeping them on the SD card. |
| `BEVY_HORIZON_CPU_PROFILE=1` | Write `cpu-profile.txt`, where the processor's time goes, every 30 s (slows the game slightly). |

## Troubleshooting

The game's logs are in `switch/horizon/`: `log.txt`, `logs/latest.log`, `panic.txt` and `crash-reports/`.
Atmosphère's own reports are in `atmosphere/crash_reports/`.

<details>
<summary><b>The game closes when a world opens: "… re-run horizon-prepare" (after updating from 1.x)</b></summary>

The game files in `switch/horizon/resources/` were prepared by an older Horizon than this version needs: an update
in the game replaces `horizon.nro`, not the game files, which come from your own Minecraft. Update the Horizon
launcher on your computer and press **Play** once to prepare them again (step 2), copy them to
`switch/horizon/resources/` replacing the old ones (step 3), and delete `switch/horizon/resources.hzpak` so it's
packed again. [Horizon NX Companion](https://github.com/KoHaku743/horizon-nx-companion) does the copying and
packing.
</details>

<details>
<summary><b>The whole Switch freezes, in the game or when a homebrew app (Goldleaf) opens <code>switch/horizon</code></b></summary>

Something in the `switch/horizon` folder makes the Switch's file system hang, so anything that reads it can freeze
the console, the game included (for one player it froze on joining any server, as skins started to load). Rename
the folder (for example to `horizon-old`) on your computer, install again with
[Horizon NX Companion](https://github.com/KoHaku743/horizon-nx-companion), and check the game and Goldleaf work.
Then copy back what you want to keep from `horizon-old` one item at a time (`saves/`, `account.json`,
`options.txt`, `servers.dat`), opening the folder in Goldleaf after each, so the item that caused it stays out.
</details>

<details>
<summary><b>The game closes after loading with "expected value at line 1 column 1"</b></summary>

macOS's hidden `._` files are in the resources folder. Run `dot_clean -m` on the SD card (see
[step 3](#3-copy-everything-to-the-sd-card)), or copy over FTP or MTP.
</details>

<details>
<summary><b>The game closes about 10 seconds after the world appears</b></summary>

The log shows `channel 2 lost: … (fifo idle timeout)`: the GPU hung. That happened with versions 1.0.0 and 1.1.0,
built with the public graphics driver, when two of its shader optimizations (`plat`, `ofold`) were on. Later
versions use release 0.1.0's driver, which runs them without hanging. If it happens anyway, remove `NAK_DEBUG` from
`env.txt`.
</details>

<details>
<summary><b>Loading is slow every time</b></summary>

The resources pack (`switch/horizon/resources.hzpak`) isn't in use: the log says why on its `[Pack]` line. It's
built once, about 20 minutes after the first start; if you quit before `[Pack] done`, it's built again next time.
Preparing the resources again on your computer makes the game pack them again. Deleting `resources.hzpak` is safe.
</details>

<details>
<summary><b>A server's resource pack disconnects me with "Configuration took too long"</b></summary>

The first time with a new pack, the game builds its files; a large pack can take longer than the server waits. Join
again: the second time it's ready. After a join, the pack is bundled into one file in the background
(`.horizon-cache/resourcepacks/*.hzpak`), and later joins apply it in seconds.
</details>

<details>
<summary><b>The console freezes when I press HOME</b></summary>

Custom HOME menus (SwitchU, and front ends built on it such as HomeNX and CardsNX) froze the console when HOME was
pressed over Horizon; with Nintendo's own HOME menu it works. Remove or turn off the custom HOME menu.
</details>

<details>
<summary><b>The world takes very long to load the first time</b></summary>

The first time a world opens, the driver compiles the terrain's shaders and the world generates around you. Later
starts read the compiled shaders from the SD card (`.mesa/` at the card's root; deleting it is safe).
</details>

<details>
<summary><b>A second controller doesn't join splitscreen</b></summary>

Check that splitscreen is on (Options → Horizon Settings → Splitscreen) and connect the controller on Setup's
controller screen. A single Joy-Con can't be a player; a Pro Controller or a pair of Joy-Con can.
</details>

## Building

The build runs in a container (Docker or Podman, on Linux):

```sh
git clone -b switch-v010 https://github.com/KoHaku743/horizon-nx
cd horizon-nx
packaging/switch/build.sh all     # the toolchain image, the graphics driver (slow, once), then the NRO
```

It clones [bevy-horizon](https://github.com/JohnUzoka/bevy-horizon) (the Switch target, toolchain image and patched
libraries) next to the repository. The result is `target/aarch64-nintendo-horizon/release/horizon.nro`.

The graphics driver comes from [JohnUzoka/mesa-switch](https://github.com/JohnUzoka/mesa-switch) at the revision
release 0.1.0 used (`7789c1d`): the same game built with the public driver
([danfromtico/mesa-switch](https://github.com/danfromtico/mesa-switch) `606441c`) runs at about 30 fps instead of
60. CI builds JohnUzoka's by default; the "Switch NRO" workflow's `driver` input picks `public` instead.

Every push to `switch-v010` and `switch-next` runs the same build in
[GitHub Actions](.github/workflows/switch-nro.yml) (the `horizon-switch-…` file at the bottom of a run; you need to
be signed in to GitHub). Its log and the build's function symbols (for naming the offsets in a crash report or a
`cpu-profile.txt`) are kept on the `ci-logs` branch. To publish a release, run the "Switch NRO" workflow by hand
(**Run workflow**) on `switch-v010` with a version in `release` (for example `v2.2.1`): it builds, tags the commit
and attaches the zip. Details: [docs/switch.md](docs/switch.md).

### Branches

| Branch | What it is |
|---|---|
| **`switch-v010`** | **Releases.** Horizon NX on today's Horizon, the source of every release. |
| `switch-next` | Development: changes land here first and are tested on a console before a release. |
| `switch` | The original port's development branch. |
| `switch-build` | The build setup's first version, on top of `switch`. |
| `pr/*` | Fixes and features offered back to the original port as pull requests. |
| `ci-logs` | The CI build logs and the latest build's symbols. |

### What this fork changes

- **A public build:** `packaging/switch/` and the patched graphics, window, input and network libraries in
  `third_party/`, which the original port built in a private setup.
- **Today's Horizon (2.0):** the Switch platform carried onto current Horizon, so mods made for it work.
- **Microsoft sign-in on the console**, the title screen's account screen.
- **Speed:** a network thread that no longer spins on a CPU core, helper threads behind drawing, culling and HUD
  defaults for the Switch, John Uzoka's merged water surfaces, and the GPU clocks above.
- **A faster memory allocator:** the Switch's own (newlib's malloc) took about half of the game's CPU time; Rust's
  blocks now come from a TLSF heap (`crates/horizon-nx/src/allocator/tlsf_heap.rs`, [rlsf](https://github.com/yvt/rlsf)).
- **Resources in one file:** each file opened on the SD card costs about 10 ms, and the resources are 28,000 files.
  `crates/horizon-nx/src/vlinks/pack.rs` packs them into `resources.hzpak` and serves opens, listings and reads from
  it: loading went from about 3.5 minutes to 15 seconds. Server resource packs are bundled the same way after the
  first join.
- **Links on the SD card:** resource packs and mods are folders of links into the game files, which the SD card's
  file system (FAT) can't store; `crates/horizon-nx/src/vlinks` keeps each folder's links in a small `.hzlinks`
  file, so packs and mods load without copying ~650 MB.
- **Switch controls and splitscreen:** each controller its own player, the Switch's controller screen, HD rumble per
  controller (`crates/horizon-nx/src/rumble.rs`), gyro aiming (`crates/horizon-nx/src/motion.rs`), touch on the
  on-screen keyboard and server click actions in chat.
- **Get Mods** and the **in-game updater**, for full releases only.

## Credits

- **[Horizon](https://github.com/horizon-rust/horizon)**: Ludovic Marie and the Horizon contributors.
- **The Switch port**: [JohnUzoka](https://github.com/JohnUzoka) ([horizon-nx](https://github.com/JohnUzoka/horizon-nx),
  [bevy-horizon](https://github.com/JohnUzoka/bevy-horizon)).
- **The Switch graphics driver**: [mesa-switch](https://github.com/danfromtico/mesa-switch) by danfromtico and
  NaGaa95, built on Mesa's NVK, with [JohnUzoka's changes](https://github.com/JohnUzoka/mesa-switch).
- **The TLSF allocator**: [rlsf](https://github.com/yvt/rlsf) by yvt.
- **The Switch homebrew toolchain**: [devkitPro](https://devkitpro.org) and [libnx](https://github.com/switchbrew/libnx).
- **Horizon NX, this fork** (the Switch build's features, releases and the
  [companion app](https://github.com/KoHaku743/horizon-nx-companion)): [KoHaku743](https://github.com/KoHaku743).
- **Create**: the Create Team / [The Creators of Create](https://github.com/Creators-of-Create/Create), ported to
  Rust for Horizon in this fork (`crates/horizon-create`, MIT).

## License

LGPL-3.0-or-later, as Horizon: see [LICENSE](LICENSE). Third-party code and its licenses:
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Minecraft is a trademark of Mojang Synergies AB.

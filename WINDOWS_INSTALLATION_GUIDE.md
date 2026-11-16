# DND55 — Windows Installation Guide

DND55 is a desktop tabletop RPG for the fifth-edition 2024 rules (SRD 5.2.1). One person is the
**Dungeon Master (DM)**: they host the game on their PC. Everyone else **joins** from their own PC.

This guide has two parts:

* **Method A — Install the finished game.** For everyone. About 5 minutes, no programming.
* **Method B — Build the game from the source code.** For developers or curious students. About 30–60
  minutes, mostly waiting for downloads.

> **Honest status (please read).** The Windows files were built on Linux with the MinGW cross-compiler
> and smoke-tested under Wine. As of this release nobody has run them on a real Windows 10/11 PC. If you
> are the first, see "Reporting a problem" at the end. The game logic has 150+ automated tests, but on
> Windows your graphics driver and sound card are the most likely sources of trouble. Both have
> fallbacks, described below.

---

## Requirements

| | Minimum |
|---|---|
| Windows | Windows 10 or Windows 11, **64-bit** |
| Graphics | A GPU with DirectX 12 or Vulkan (almost every GPU since 2015, including Intel integrated graphics). OpenGL is tried as a last resort. |
| Memory | 4 GB RAM |
| Disk | 150 MB for the game, plus your saves |
| Network | Only for multiplayer. LAN works without internet. |

---

## Method A — Install the finished game

You need **one** of these two files (they contain the same game):

* `DND55-Setup.exe`: the installer. **Recommended.** It adds a Start Menu entry and an uninstaller.
* `DND55-Windows-Portable.zip`: no installation. Unzip and run it, for example from a USB stick.

### Step 1 — Download the file

Save `DND55-Setup.exe` (or the zip) to your **Downloads** folder.

### Step 2 — (Optional) check the file is intact

Next to the files there is `SHA256SUMS.txt`. To compare:

1. Open the **Start menu**, type `powershell`, press **Enter**.
2. Type the following (replace the path if your file is elsewhere) and press **Enter**:

   ```powershell
   Get-FileHash $HOME\Downloads\DND55-Setup.exe -Algorithm SHA256
   ```

3. The long code printed must match the line for `DND55-Setup.exe` in `SHA256SUMS.txt`. If it doesn't, download again.

### Step 3 — Start the installer

Double-click `DND55-Setup.exe`.

**"Windows protected your PC" (SmartScreen).** This is expected: the game is not signed with a paid
code-signing certificate. Click **More info**, then **Run anyway**. (Do this only for files you got
from someone you trust.)

### Step 4 — Choose the language and accept the license

Pick **English** or **Italiano**, click **Next**, and accept the MIT license. Third-party licenses and
the SRD attribution are shown too.

### Step 5 — Choose components

* **DND55 (required)**: the game.
* **Desktop shortcut** (optional): an icon on your desktop.
* **Windows Firewall rule** (optional, recommended for DMs): allows friends on your network to connect
  to the game you host. Players who only join don't need it.

### Step 6 — Choose the folder and install

The default `C:\Program Files\DND55` is fine. Click **Install**, then **Finish**. Tick
**Run DND55** to start it right away.

### Step 7 — First start

Open **Start menu → DND55**. The first start can take a few seconds while Windows prepares the graphics.

* **The game says it could not start the graphics system:** update your graphics driver (from the
  NVIDIA, AMD or Intel website). The game already tries DirectX 12 → Vulkan → OpenGL automatically. You
  can also pick a backend in **Settings → Graphics backend**.
* **No sound:** if no audio device is found, the game runs silently and says so in Settings. Plug in
  headphones or speakers and restart the game.

### Step 8 — Settings

Main menu → **Settings** (or **⚙** in the game's top bar): language (English/Italiano), interface scale, resolution, windowed / borderless /
fullscreen, VSync, graphics backend, and master / effects / music / ambience volume. The **Open save
folder** button opens the folder with your saves and logs.

### Step 9 — Play

See **"Your first game in 10 steps"** below.

### Step 10 — Where are my files?

| What | Installed version | Portable version |
|---|---|---|
| Campaign saves | `%APPDATA%\DND55\campaigns.db` | `DND55\data\campaigns.db` |
| Settings | `%APPDATA%\DND55\settings.json` | `DND55\data\settings.json` |
| Logs & crash reports | `%APPDATA%\DND55\logs\` | `DND55\data\logs\` |

To open `%APPDATA%`, press **Win + R**, type `%APPDATA%\DND55` and press **Enter**.

### Portable version (zip)

1. Right-click `DND55-Windows-Portable.zip` → **Extract All…** → **Extract**.
2. Open the extracted `DND55` folder and double-click `DND55.exe`.
3. The file `portable.txt` keeps saves in the `data` folder next to the exe. Delete it to use
   `%APPDATA%\DND55` instead.

### Uninstalling

**Settings → Apps → Installed apps → DND55 → Uninstall** (or Start menu → DND55 → Uninstall). The
uninstaller **asks** whether to delete your saves in `%APPDATA%\DND55`. Answer **No** to keep your
campaigns for a later reinstall.

### Updating

Run the new `DND55-Setup.exe` over the old one. It replaces the program and keeps your saves. Old saves
are upgraded automatically when loaded, and every time DND55 starts it first copies `campaigns.db` to
`campaigns.db.bak`. Everyone in a session should use the **same
version**: the game refuses to connect mismatched versions and tells you why.

---

## Your first game in 10 steps

1. **DM:** Main menu → **Play — Quick Demo** (a two-room demo) or **Create Campaign**.
2. **DM:** click **✉ Invite players** in the top bar and **Copy** an invite link (see "Multiplayer" below for which one).
3. **DM:** send the link to your friends (chat, e-mail…).
4. **Players:** Main menu → **Characters** → **+ New** and go through the steps (class, origin,
   abilities, skills, equipment, details, optional PNG portrait). Or create the character later, inside the game.
5. **Players:** Main menu → **Join Campaign**, paste the invite link, type your name, pick your
   character, then click **Connect**.
6. **Players** click **✔ I'm ready**. **DM:** in the **Session** tab, wait until every player shows ✔, then click **Start session**.
7. **Everyone:** click your token, then click a cell to move, or click a door or chest to open it.
   Use the dice tray at the bottom for checks.
8. **DM:** open **🐉 Open bestiary** (Monsters tab), drag a monster onto the map, then **Start combat** (Session tab).
9. **In combat:** on your turn, pick an attack, a spell (📖 Spellbook) or a class feature in the action bar, then
   click the target. Press **End turn** when you're done. When you are hit, a window may offer a reaction (Shield, Uncanny Dodge…).
10. **DM:** after the fight, use **Award to party** (XP), **Short/Long Rest**, and move the party to the next
    room with the **🗺 Campaign map** (right-click a room → Travel here). Click **Save** in the top bar.

Controls: **WASD/arrows** pan · **mouse wheel** zoom · **middle-drag** pan · **right-click** cancels ·
DM: **Shift+click** teleports the selected creature · editor shortcuts: **Ctrl+C/V/D, Del, R, Ctrl+Z/Y**.

---

## Multiplayer

The DM's PC runs the game ("host"). Players connect to it. There are three ways:

| Situation | What to use | Setup |
|---|---|---|
| Same house / same Wi-Fi | **Same network** invite (`dnd55://direct/192.168.x.x:7878/CODE`) | None. Allow the Windows Firewall prompt (or tick the firewall rule in the installer). |
| Different houses, easiest | **Relay** invite (`dnd55://relay/…`) | Someone must run a relay server (see below). No router changes. |
| Different houses, no relay | **Internet (direct)** | Open TCP port 7878 on the DM's router: try the **UPnP** button, or set up port forwarding manually. |

A VPN such as Tailscale or ZeroTier also works: use the DM's VPN IP with the **Same network** method.

### The relay server: what it is and what it costs

A relay is a small program (`dnd55-relay`) on a computer with a public internet address. The DM's
game and each player connect **out** to it, so nobody has to open router ports.

* **There is no public relay run by the authors.** You need your own. Options:
  * A small cloud server (VPS): about **4–6 USD/€ per month** (for example Hetzner, DigitalOcean,
    Vultr, OVH). Some providers have free tiers (for example the Oracle Cloud "Always Free" VMs), but
    their conditions change, so check them yourself.
  * A friend with a PC that can open one port (TCP 7979) on their router.
* It uses very little: a few KB/s per player. The cheapest VPS is plenty.
* Setup instructions: [`docs/RELAY.md`](docs/RELAY.md). In the game the DM enters the relay address
  (`relay.example.com:7979`) in **✉ Invite players** → **Go online** and shares the relay invite.

### Security, honestly

* Joining requires the 6-character session code (part of every invite). The DM's tools are protected by a secret that never leaves the DM's PC. Hidden information (secret doors, invisible monsters, DM notes) is never sent to players.
* Connections are **not encrypted** (no TLS yet). A relay operator, or someone on your Wi-Fi, could read game traffic. Don't type passwords or personal secrets into the chat.
* To revoke old invites, stop hosting and host again: you get a new code.

### When something doesn't connect

The game explains each error. The most common ones:

| Message contains | Meaning / fix |
|---|---|
| "refused" | Nothing listens at that address: wrong IP or port, the DM hasn't started hosting, or the DM's firewall blocks DND55. |
| "timed out" | A router or firewall drops the connection. Use the relay or port forwarding. |
| "no game with that invite code" | The DM's game closed or restarted. Ask for a fresh relay invite. |
| "version" | Players and DM use different DND55 versions. Everyone should install the same release. |

If the connection drops during play, the game **reconnects automatically** (10 attempts over about 3
minutes) and you keep your character. A **Reconnect** button appears if it gives up.

---

## Method B — Build from source (Windows)

You will install three free tools, download the code, and compile it. Commands go into **PowerShell**
(Start menu → type `powershell` → Enter). Paste each command and press **Enter**. Wait for it to finish
before typing the next one.

### B1 — Install Git

Download it from <https://git-scm.com/download/win> and run the installer with the default options.

Check it worked. Open a **new** PowerShell window and type:

```powershell
git --version
```

You should see something like `git version 2.x`.

### B2 — Install the Microsoft C++ build tools (MSVC)

Rust uses Microsoft's linker on Windows.

1. Download **Build Tools for Visual Studio** from <https://visualstudio.microsoft.com/visual-cpp-build-tools/>.
2. In the installer, tick **Desktop development with C++**. Make sure **MSVC … build tools** and a
   **Windows 11 SDK** (or Windows 10 SDK) are selected on the right. The SDK is needed for the game icon.
3. Click **Install** (several GB) and restart the PC when it's done.

### B3 — Install Rust

1. Download **rustup-init.exe (64-bit)** from <https://rustup.rs> and run it.
2. Press **Enter** to accept the default installation.
3. Close and reopen PowerShell, then check:

   ```powershell
   rustc --version
   cargo --version
   ```

The project pins its Rust version in `rust-toolchain.toml`. The first build downloads it
automatically.

### B4 — Get the source code

The release includes `DND55-Source.zip`. Extract it to, for example, `C:\dev\dnd55`. If the project is
on a Git server, you can clone it instead:

```powershell
git clone <repository-url> C:\dev\dnd55
```

Then go into the folder:

```powershell
cd C:\dev\dnd55
```

### B5 — Run the tests (optional, recommended)

```powershell
cargo test --workspace
```

The first time, this downloads and compiles all dependencies (5–20 minutes). At the end every line
should say `test result: ok`.

### B6 — Build the game

```powershell
cargo build --release -p dnd_desktop
```

The finished program is `target\release\DND55.exe`. Release builds don't open a console window.

### B7 — Launch

```powershell
.\target\release\DND55.exe
```

Or double-click `DND55.exe` in Explorer.

### B8 — Create a shortcut

In Explorer, right-click `target\release\DND55.exe` → **Show more options** → **Send to** → **Desktop
(create shortcut)**.

### B9 — Build the installer yourself (optional)

1. Install **NSIS** from <https://nsis.sourceforge.io/Download>.
2. Run the PowerShell build script:

   ```powershell
   powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1
   ```

   It builds the game, stages the documents, creates the portable zip, and creates `DND55-Setup.exe` in
   `dist\` if Inno Setup 6 or NSIS is installed.

### B10 — Update to a newer version

```powershell
cd C:\dev\dnd55
git pull
cargo build --release -p dnd_desktop
```

(If you used the zip, extract the new zip over the old folder.)

### Troubleshooting the build

| Problem | Fix |
|---|---|
| `linker 'link.exe' not found` | Step B2 is missing or incomplete. Reopen the Build Tools installer and add "Desktop development with C++". |
| `rc.exe` / icon warning | The Windows SDK is missing (B2). The game still builds, just without the embedded icon. |
| `cargo` is not recognized | Close and reopen PowerShell after installing Rust, or log out and back in. |
| Build runs out of memory | Close other programs, or build with fewer jobs: `cargo build --release -p dnd_desktop -j 2` |
| Antivirus deletes or locks files under `target\` | Add an exclusion for the project folder. Fresh, unsigned executables often trigger heuristics. |
| `failed to run custom build command for libsqlite3-sys` | The C compiler from B2 is missing. |

---

## Troubleshooting the game

| Problem | What to do |
|---|---|
| Error window: "could not start the graphics system" | Update the GPU driver. Try **Settings → Graphics backend** → Vulkan / OpenGL. Remote Desktop and some VMs have no 3D acceleration. |
| The game closes immediately | Look in `%APPDATA%\DND55\logs\` for `crash-*.txt` and `dnd55.log`. |
| No sound | Settings shows "No audio device was found". Connect an output device, tick **Enable sound**, and restart the game. |
| Text too small/large | **Settings → Interface scale**. |
| Players can't see the DM's game on the LAN | Allow DND55 in Windows Firewall (Windows Security → Firewall → Allow an app). Both PCs must be on the same network. Check the IP shown in the invite. |
| "Windows protected your PC" | See Step 3 (unsigned executable). |
| A save won't load | Close DND55. **Before starting it again** (each start overwrites the backup), replace `campaigns.db` with `campaigns.db.bak` in the data folder. |

### Reporting a problem

Send `dnd55.log` and any `crash-*.txt` from `%APPDATA%\DND55\logs\`, your Windows version, your GPU
model, and what you clicked just before the problem. These files contain no passwords. They may contain
character and campaign names.

---

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the
Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative
Commons Attribution 4.0 International License, available at
https://creativecommons.org/licenses/by/4.0/legalcode.

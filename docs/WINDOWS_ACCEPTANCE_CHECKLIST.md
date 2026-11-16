# Windows Manual Acceptance Checklist

Use this on a real Windows 10/11 PC. The automated tests and the Linux/Wine runs can't replace it.
Tick each box and note the GPU, the Windows version, and anything odd. Attach `%APPDATA%\DND55\logs\dnd55.log`.

PC: ____________________  Windows: 10 / 11 (build ______)  GPU/driver: ____________________

## 1. Install & start
- [ ] `DND55-Setup.exe` runs. After **More info → Run anyway** (unsigned), the installer appears in EN and IT.
- [ ] Optional desktop shortcut and firewall rule are created when ticked.
- [ ] Start menu → DND55 starts **without a console window**. Window icon and taskbar icon show the d20.
- [ ] Portable zip: extract → `DND55.exe` runs; saves appear in `DND55\data\`.
- [ ] First start creates `%APPDATA%\DND55\` with `settings.json`, `campaigns.db` and `logs\dnd55.log`.

## 2. Graphics & audio
- [ ] Settings → Resolution, Windowed / Borderless / Fullscreen, VSync and Interface scale apply immediately.
- [ ] Settings → Graphics backend: DirectX 12, Vulkan and OpenGL each start after a restart (note any that fail).
- [ ] With speakers/headphones: dice, hits, door, spell sounds; room music/ambience change; volume sliders work.
- [ ] With **no** audio device (disable it in Sound settings): the game starts and Settings shows "No audio device".
- [ ] Alt+Tab, minimize/restore and moving between monitors don't crash.

## 3. Single PC smoke test (DM + player on the same PC)
- [ ] Play — Quick Demo: the map, lights and tokens render. Pan (WASD), zoom (wheel) and middle-drag work.
- [ ] Start a second DND55 window → Join with the **This computer** invite → create a character with a PNG portrait.
- [ ] The player moves, opens a door, rolls dice. The DM sees it.

## 4. DM tools (every button)
- [ ] Editor: Pencil, Eraser, Rect, Fill, Object, Remove, Entry, Light; Undo/Redo; layer 👁 🔒 ⏶ ⏷; ambient light.
- [ ] Select tool: click, Shift+click, box select, drag-move, Copy/Paste/Duplicate/Rotate/Delete buttons **and** Ctrl+C/V/D, R, Del, Esc, Ctrl+Z/Y.
- [ ] Object properties: label, hidden, open, blocks, light; container contents (add potion) → the player loots it.
- [ ] Draw tool: freehand, line, arrow, circle, text; temporary/DM-only; Clear; "Players may draw".
- [ ] Bestiary: search, type and CR filters, stat block, drag a monster onto the map, Place on map, custom monster (save/edit/delete).
- [ ] Campaign map: drag rooms, connect (right-click → Connect), delete edge, lock edge (right-click on ✖), travel, new room (double-click), delete room.
- [ ] Events: each template (trap, lever, chest, boss, dialogue, skill check) → Save → trigger it in play; Fire now; Re-arm.
- [ ] Session: start session, free movement, combat with surprise, next turn, end combat, ±HP, conditions, reveal/hide.
- [ ] Rest (short/long), Award XP, Milestone level, Give item, end an active effect, room music/ambience.
- [ ] Save (with a label) → Leave → Load Campaign → the state is intact.

## 5. Player features
- [ ] Spellbook: cast a cantrip, an area spell (shows a template), a multi-target spell, upcast; Prepare tab saves.
- [ ] Concentration shows ◎ and End works. The Shield prompt appears when hit (Wizard with Shield prepared).
- [ ] Class feature buttons: Fighter (Second Wind, Action Surge), Barbarian (Rage), Rogue (Cunning Action), Monk (Flurry), Paladin (Lay On Hands), Bard (Inspiration).
- [ ] Level up dialog after XP (ASI at level 4, spells for casters).
- [ ] Inventory: equip/unequip outside combat, drink a potion, give to an adjacent ally; light a torch.
- [ ] Dialogue window choices work; narration messages can be dismissed.

## 6. Multiplayer between two PCs
- [ ] LAN: the player joins with the **Same network** invite. The Windows Firewall prompt was allowed (or the installer rule exists).
- [ ] Relay (`dnd55-relay` on a VPS): Go online → green status → the player joins from another network.
- [ ] Unplug the player's network for 10 s: the game auto-reconnects, and the character and turn state are intact.
- [ ] Wrong code / wrong version / stopped host: clear error messages.
- [ ] Hidden monsters, DM notes and hidden objects are **never** visible to the player.
- [ ] UPnP button: note the result (many routers disable UPnP).

## 7. Uninstall
- [ ] Uninstall asks whether to delete saves. Answering **No** keeps `%APPDATA%\DND55`. Answering **Yes** removes it.
- [ ] The firewall rule and shortcuts are removed.

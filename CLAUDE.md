# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Keystroke is a touch-typing game built with Bevy 0.19 and modelled on keybr.com.

- Lessons use only the keys you've unlocked.
- The next key unlocks once every unlocked key reaches the difficulty's target speed.
- There are no asset files. The font is Bevy's built-in FiraMono subset, sounds are synthesised in code, and the word list is compiled in with `include_str!`.

## Commands

```sh
cargo run                                   # dev build; dependencies are still optimised (see Cargo.toml profiles)
cargo run --release
KEYSTROKE_DATA_DIR=/tmp/ks cargo run        # throwaway save instead of ~/.local/share/keystroke/progress.json
RUST_LOG=info,keystroke=debug cargo run     # also logs each generated lesson ("New lesson: ...")
cargo test                                  # unit tests plus headless Bevy app tests
cargo test typing::tests::stop_mode_miss_holds_cursor   # a single test
cargo clippy --all-targets -- -D warnings
cargo fmt
```

On Linux you need alsa-lib, libudev, wayland and libxkbcommon, and a Vulkan driver. A clean build of Bevy takes about 5 minutes.

## Architecture

**Pure logic.** These modules have no systems and are unit-tested without an `App`:

- `typing.rs`: `TypingSession` holds one lesson.
  - It tracks the cursor, the per-character `CharState`, and the two `MistakeMode`s. Easy and Medium stop on error. Hard advances and you fix mistakes with Backspace, and you can't pass a space while the current word has a mistake.
  - It computes WPM and accuracy.
  - `tally()` returns per-letter timing samples. A sample is taken only for a letter typed right first time straight after a clean character, with a gap of at most `MAX_SAMPLE_GAP`.
  - It also has `lesson_score`.
- `progress.rs`: `Profile` holds one difficulty's progress.
  - Per key: an EMA of the time to type it and a `best_ms`. Confidence is target time divided by `best_ms`.
  - `apply_lesson` unlocks at most one letter of `LETTER_ORDER` per lesson, and only when every unlocked letter has confidence ≥ 1.
  - `focus_key` returns the weakest unlocked letter. Letters with no data count as weakest.
- `lesson/`: `LessonGenerator`.
  - An order-2 Markov `PhoneticModel` is trained on `words_en.txt`. That file is whitespace-separated and also serves as the real-word dictionary.
  - Each lesson uses real words when at least 15 fit the unlocked letters, and fills the rest with pseudo-words. Every word contains the focus letter.
  - Hard adds capitals and `PUNCTUATION`.
- `difficulty.rs`: the Easy/Medium/Hard rule table (target WPM, mistake mode, capital and punctuation rates, score multiplier).
- `ui/keyboard.rs`: the `ROWS` layout table and `key_for_char`. Capitals and symbols hint the Shift key on the opposite hand.
- `audio.rs`: the synthesis functions and `wav_bytes`.

**Bevy plugins.**

- `practice.rs` has two systems, both in the `TypingSystems` set. Every presentation system runs `.after(TypingSystems)`.
  - `handle_typing` reads `MessageReader<KeyboardInput>`, updates the `TypingSession` resource and sends `KeyFeedback`.
  - `finish_lesson` applies a finished lesson to the profile and saves. It then replaces the session in place with the next lesson and sends `LessonCompleted`.
- `save.rs`: the `SaveData` resource holds all three profiles, the selected difficulty and the sound toggle.
  - It is loaded when the plugin is built and written with `save::persist` (write to a temp file, then rename).
  - A corrupt save is moved to `progress.json.bak`.
- `audio.rs` plays the sounds in response to `KeyFeedback` and `LessonCompleted`.
- `ui/mod.rs` spawns the camera and a persistent layout: `ScreenRoot` above the on-screen keyboard, which is visible in every state.
  - Screens (`menu.rs`, `hud.rs` with `passage.rs`) spawn under `ScreenRoot` with `DespawnOnExit`.
  - `UiScale` is fitted to the window, against a design size of 1280×800.

**States.** `AppState::Loading` → `Menu` ⇄ `Practice`. `Loading` exists because Bevy runs the initial state's `OnEnter` *before* `Startup`. The `show_menu` Startup system moves to `Menu` once the layout exists.

**Gotchas:**

- **Keyboard input uses logical keys.** Both `handle_typing` and `menu_keys` read `KeyboardInput` messages and match on `logical_key`, never on `ButtonInput<KeyCode>`.
  - This follows the user's layout.
  - It is also the only thing wtype-driven tests can rely on: wtype's virtual keyboard sends arbitrary physical `KeyCode`s.
  - Physical codes are used only for display: the held-key highlight and the wrong-key flash.
- **Stale input is dropped after a state change.** A system's message reader doesn't advance while the system isn't running, so each input system drops queued messages on its first run after a state change:
  - `handle_typing` does this when `session.is_added()`, so the menu key that started practice isn't typed.
  - `menu_keys` does this when `State` has changed, so keys typed just before Esc don't trigger menu actions.
- **Lesson rollover.** It assigns `*session = …` rather than re-inserting the resource; re-inserting would drop keystrokes between lessons. The passage respawns its spans on `LessonCompleted`.
- **Tuning constants:**
  - difficulty table: `difficulty.rs`
  - `SMOOTHING`, `STARTING_KEYS`, `LETTER_ORDER`: `progress.rs`
  - `LESSON_LETTERS`, `PUNCTUATION`: `lesson/mod.rs`
  - `MAX_SAMPLE_GAP`: `typing.rs`
  - colours: `ui::theme`
- **Fonts.** The built-in FiraMono subset covers only printable ASCII (U+0020–U+007E).
  - UI strings use `|` and `Left/Right`, not `·`, arrows or dashes.
  - Adding such symbols means shipping a font.

## Checking the UI end to end

This machine runs Hyprland on Wayland, with `grim`, `wtype` and `hyprctl` available.

1. Run the binary in the background with `KEYSTROKE_DATA_DIR` and `RUST_LOG=info,keystroke=debug`.
2. Find the window with `hyprctl clients -j` (title `Keystroke`) and focus it with `hyprctl dispatch focuswindow 'title:^Keystroke$'`.
3. Read the lesson text from the log and type it with `wtype -d 90 "<text>"`.
4. Take screenshots with `grim -g "x,y wxh"`.

**Ask the user before using wtype.** It types into whichever window has focus, so check `hyprctl activewindow -j` before each call.

## Bevy 0.19 API notes

Bevy's API changes every release, so don't write it from memory of older versions:

- **Messages.** Buffered events are messages: `#[derive(Message)]`, `app.add_message::<T>()`, and `MessageReader` / `MessageWriter::write`. `KeyboardInput` is a message.
- **UI.**
  - `border_radius` is a field on `Node`, not a component.
  - Use `BorderColor::all(..)`, and the `px()` / `percent()` helpers for `Val`.
- **Text.**
  - A `Text::default()` root with `TextSpan` children.
  - `TextFont::from_font_size(..)` and `TextLayout::justify(Justify::..)`.
  - `TextBackgroundColor`, and `Underline` / `UnderlineColor`.
- **States.**
  - `DespawnOnExit(state)` despawns an entity when its state is left.
  - `NextState::set` re-runs `OnEnter` even when the new state is the current one.
- **Queries.** `Query::single()` returns a `Result`. Prefer `Single<..>` parameters, which skip the system when nothing matches.
- **Cargo features.**
  - Bevy uses `default-features = false` with `ui`, `audio` and `wav`. `wav` decodes the synthesised sounds.
  - If a type is missing, add a feature collection such as `"2d"` rather than enabling all defaults.
- **Headless tests.** Use `MinimalPlugins + StatesPlugin + InputPlugin`.
  - Messages don't rotate between `app.update()` calls without a fixed-timestep tick, so `iter_current_update_messages` returns stale messages.
  - Collect messages with a recording system instead, as `practice.rs` tests do.

## keybr.com

The unlock and focus rules and the pseudo-word approach come from keybr.com, which is AGPL-3.0. Reimplement the ideas only: never copy its code or data.

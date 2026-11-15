# Contributing to hams

Thanks for helping. hams is small on purpose, so a short read of this page saves everyone time.

## The one rule

**Everything stays on the user's machine.** Audio, transcripts, history and settings never leave
it. The only network call hams makes is downloading a model the user asked for in Settings. Pull
requests that add telemetry, analytics, crash reporting, cloud APIs or "phone home" checks will
not be merged, even if they are opt-in.

## Getting set up

You need Rust (stable), Node 22+ (see `.nvmrc`), CMake and a C/C++ compiler. On Linux, also the
[Tauri prerequisites](https://tauri.app/start/prerequisites/) and `libasound2-dev`.

```sh
npm install
npm run tauri dev       # first build compiles whisper.cpp and llama.cpp: several minutes
```

To check the speech engine without the UI, download a model from Settings (or put
`ggml-base.en.bin` in the models folder listed in the README) and run:

```sh
src-tauri/target/debug/hams --transcribe clip.wav --model base.en --verbose
```

On macOS, a rebuilt app needs re-approving under Privacy & Security → Accessibility (remove the
old hams entry, add the new one). This is the most common "the key does nothing" report.

## Before you open a pull request

CI runs these; running them first is faster:

```sh
npm run build                         # TypeScript typecheck + frontend build
npm run sidecar                       # once, builds hams-polish
cd src-tauri
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- Keep pull requests focused: one fix or feature each.
- Add a test when you change transcript processing in `core.rs` (tidy, dictionary, snippets).
  Those rules are the easiest place to break someone's text without noticing.
- OS-specific code belongs in `src-tauri/crates/platform/` (`macos.rs`, `linux.rs`), behind
  the shared interface in `lib.rs`.
- Say in the pull request which OS and CPU you tested on (for example "macOS 26, Apple
  Silicon" or "Fedora 43, Wayland").
- For larger changes (a new feature, a new dependency, a change to stored file formats), open
  an issue first so we can agree on the approach before you spend time on it.

## Where help is most useful

- **Linux.** The Linux code compiles but has had very little real-world use. Reports and fixes
  from X11 and Wayland desktops are very welcome.
- **Intel Macs.** Supported, with the GPU turned off. Reports on speed and accuracy help.
- **Languages other than English**, including the right-to-left and Urdu/Arabic punctuation
  handling in the tidy rules.

## Reporting bugs

Use the bug report form. Include your OS, CPU (Apple Silicon / Intel / x86_64 Linux), the
speech model, and the output of the command above with `--verbose` if transcription is wrong.
Please do not attach recordings of private speech.

Security problems: see [SECURITY.md](SECURITY.md) instead of opening a public issue.

## License

By contributing you agree that your contributions are licensed under the [MIT License](LICENSE).
Everyone taking part is expected to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

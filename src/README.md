# Keyview
A terminal-based viewer for Sled databases with hierarchical key navigation and MessagePack value decoding.
## Features
- Browse Sled database files using a file picker
- Navigate hierarchical keys using '/' as a delimiter
- View MessagePack-encoded values as formatted JSON
- Simple terminal UI with two panels:
- Left panel: Key browser
- Right panel: Value viewer
## Controls
- `o`: Open a database file
- `↑`/`↓`: Navigate through keys
- `Enter`: Select a key/descend into key hierarchy
- `Backspace`: Go up one level in the key hierarchy
- `q`: Quit the application
## Building
Make sure you have Rust installed, then:
```bash
cargo build --release
```
The binary will be available at `target/release/keyview`
## Dependencies
- ratatui: Terminal UI framework
- sled: Embedded database
- rmp-serde: MessagePack encoding/decoding
- crossterm: Terminal manipulation
- rfd: File dialog support
## Use Case
This tool is particularly useful when you need to:
- Inspect the contents of Sled databases
- Navigate complex hierarchical key structures
- View MessagePack-encoded values in a human-readable format
## Example
If your Sled database contains keys like:
```
users/1/name
users/1/email
users/2/name
users/2/email
```
You can navigate through the hierarchy:
1. Select "users"
2. Select a user ID
3. View individual fields for each user
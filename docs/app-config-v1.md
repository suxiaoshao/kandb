# App Config v1

`kandb` currently stores user-editable application configuration in `config.toml`.

## Paths

The app uses platform-standard directories and keeps config and data separate.

- Linux
  - Config: `~/.config/kandb/config.toml`
  - Data: `~/.local/share/kandb/`
- macOS
  - Config: `~/Library/Preferences/kandb/config.toml`
  - Data: `~/Library/Application Support/kandb/`
- Windows
  - Config: `%APPDATA%\kandb\config.toml`
  - Data: `%LOCALAPPDATA%\kandb\`

`config.toml` is created automatically on first launch. Invalid TOML is treated as an error and is not overwritten.

## File Shape

```toml
version = 1
default_connection_id = "local-main"

[[connections]]
id = "local-main"
name = "Local Main"
provider = "sqlite"

[connections.config]
read_only = false
create_if_missing = false

[connections.config.location]
kind = "path"
path = "~/data/main.sqlite"
```

## Notes

- `connections.id` must be unique.
- `default_connection_id` must refer to an existing connection.
- Relative SQLite file paths are resolved relative to the directory containing `config.toml`.
- `~`-prefixed SQLite file paths are expanded against the user's home directory.
- Unknown provider entries are preserved so future providers can be added without rewriting the file format.

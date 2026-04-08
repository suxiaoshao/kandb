---
name: kandb-icon-usage
description: Use when adding or changing icons in the kandb repo. App UI code must use kandb_assets::IconName or ProviderIconName. Always check crates/kandb-assets/src/lib.rs first; if the icon is missing, add the Lucide icon to kandb-assets before using it from feature code.
---

# kandb icon usage

`crates/kandb-assets` is the app-level icon source of truth.

## Rules

- In app code, prefer `kandb_assets::IconName`.
- For provider or vendor logos, use `ProviderIconName`.
- Do not introduce direct `gpui_component::IconName` usage in app feature code when the icon belongs in `kandb-assets`.
- Before adding a new icon, inspect `crates/kandb-assets/src/lib.rs`.
- If the icon is missing, add it in `define_icon_assets!` and then use the new variant from app code.

## Workflow

1. Check whether `kandb_assets::IconName` already exposes the icon you need.
2. If it exists, use it directly.
3. If it does not exist, find the matching Lucide icon and add it to `define_icon_assets!` in `crates/kandb-assets/src/lib.rs`.
4. After adding it to `kandb-assets`, use the new `kandb_assets::IconName` variant from the calling code.

## Example

```rust
define_icon_assets!(
    ChevronDown => "chevron-down",
    RefreshCw => "refresh-cw",
    Table => "table",
);
Icon::new(kandb_assets::IconName::RefreshCw)
```

## Review Checklist

- Final app code uses `kandb_assets::IconName` or `ProviderIconName`
- No unnecessary direct app-level use of another icon enum was added
- Any missing Lucide icon was first added in `crates/kandb-assets/src/lib.rs`
- The chosen variant name and Lucide slug match

---
name: kandb-icon-usage
description: Add, change, or review app-owned UI icons and provider logos in kandb.
---

# kandb icon usage

`crates/kandb-assets/src/lib.rs` owns app icons. Feature code uses `kandb_assets::IconName`; provider/vendor logos use `ProviderIconName`, rather than an unrelated icon enum.

Missing Lucide icons are declared in `define_icon_assets!` before use. The variant and slug must match:

```rust
define_icon_assets!(
    RefreshCw => "refresh-cw",
);
Icon::new(kandb_assets::IconName::RefreshCw)
```

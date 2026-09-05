---
name: kandb-i18n-discipline
description: Add, change, or review user-visible text and Fluent localization in kandb.
---

# kandb i18n discipline

- User-visible text goes through `I18n` from `crates/kandb/src/i18n.rs`, using `cx.global::<I18n>()` and `t(...)` or `t_with_args(...)`.
- Keep additions, changes and removals aligned in `crates/kandb/locales/{en-US,zh-CN}/main.ftl`. Remove old keys only after their references are gone; update affected localized assertions.
- Use Fluent messages and matching arguments for interpolation, without hardcoded UI strings or ad hoc sentence assembly.

```rust
let mut args = FluentArgs::new();
args.set("version", version);
i18n.t_with_args("about-version", &args)
```

---
name: kandb-i18n-discipline
description: Use when adding, deleting, or changing user-facing text in the kandb repo. All app-visible strings must go through the i18n layer in crates/kandb/src/i18n.rs, and every text change must be checked against crates/kandb/locales/en-US/main.ftl and crates/kandb/locales/zh-CN/main.ftl.
---

# kandb i18n discipline

This repo requires user-facing text to go through the project i18n layer.

## Rules

- Do not hardcode user-visible strings in app code.
- Use `cx.global::<I18n>()` and `i18n.t(...)` or `i18n.t_with_args(...)`.
- Every addition, deletion, or change to user-visible text must be reflected in:
  - `crates/kandb/locales/en-US/main.ftl`
  - `crates/kandb/locales/zh-CN/main.ftl`
- If a string is removed from code, check whether the locale keys should also be removed.
- If text uses interpolation, add or update the matching Fluent message and arguments instead of building ad hoc strings in code.
- Review nearby code for newly introduced hardcoded strings before finishing.

## Workflow

1. Identify whether the code change introduces, removes, or changes any user-visible text.
2. If it does, route the text through `I18n` instead of hardcoding it.
3. Add or update the matching keys in both locale files.
4. If text was deleted, check whether the old locale keys are now unused and should be removed.
5. Review the surrounding file to make sure no nearby new strings were missed.

## Repo Entry Points

- i18n implementation: `crates/kandb/src/i18n.rs`
- English locale: `crates/kandb/locales/en-US/main.ftl`
- Chinese locale: `crates/kandb/locales/zh-CN/main.ftl`

## Example

```rust
let mut args = FluentArgs::new();
args.set("version", version);
i18n.t_with_args("about-version", &args)
```

## Review Checklist

- No new user-facing string was hardcoded in Rust UI code
- New or changed copy was added to both locale files
- Removed copy had its locale keys reviewed for cleanup
- `t_with_args` is used where interpolation is needed
- Tests or assertions touching localized output were updated if needed

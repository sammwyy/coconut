# Contributing

These rules apply to human contributors and coding agents.

Read [CODESTYLE.md](CODESTYLE.md) before changing source code. Follow the
[application guide](docs/dev/APPS.md) for apps and shared application UI, and the
[shell guide](docs/dev/SHELL.md) for shell, session, and system integration work.

## Commit messages

Use Conventional Commits. Write messages in English with a lowercase type and
an optional scope:

```text
type: description
type(scope): description
```

| Type | Use |
| --- | --- |
| `feat` | Add user-facing functionality. |
| `fix` | Correct a bug. |
| `refactor` | Restructure code without changing its behavior. |
| `perf` | Improve performance. |
| `docs` | Change documentation. |
| `test` | Add or update tests. |
| `build` | Change build tooling, packaging, or dependencies. |
| `ci` | Change continuous integration workflows. |
| `style` | Change formatting without changing behavior. |
| `chore` | Perform maintenance that does not fit another type. |

Use a short, concrete description in the imperative, without a trailing period.
Name the behavior or component affected; avoid vague messages such as
`fix: fixes` or `chore: update stuff`. When a scope helps, use a consistent
component name such as `editor`, `terminal`, or `files`.

```text
feat(editor): add Markdown preview
fix(terminal): restore focus after closing the command palette
docs: move theme guide into documentation directory
```

For changes that need context, add a blank line followed by a concise body
explaining the problem, the resulting behavior, and any relevant tradeoffs.
Mark breaking changes with `!` before the colon and describe the required
migration in a `BREAKING CHANGE:` footer.

## Commit boundaries

- Give each commit one coherent purpose. Keep it small enough to review and
  explain independently; file count alone does not define its scope.
- Include the implementation, relevant tests, and documentation for that
  purpose together. Each commit should leave the affected component usable.
- Separate unrelated features, fixes, refactors, and formatting changes.
  Include a refactor with a feature only when it is necessary for that feature.
- Split independent changes to different apps into separate commits. For
  example, terminal and file manager palette migrations belong in separate
  commits even when they use the same shared component.
- Commit shared CreamUI changes in the CreamUI repository, then commit each
  app's integration separately. Order dependent commits so prerequisites come
  first.
- A new app may start with one larger commit when its files form a cohesive,
  working implementation. Keep unrelated changes outside that commit.

## Before committing

Review the staged diff and remove unrelated changes, generated artifacts, and
local configuration. Format changed Rust code with `cargo fmt`, run the checks
appropriate to the affected component, and check whitespace with
`git diff --cached --check`. For documentation-only changes, verify the content
and local links.

Keep relevant READMEs current and record deferred work in [TODO.md](TODO.md).
In pull requests, describe the concrete problem, the resulting behavior, and
the validation performed. State any material limitations or checks that could
not be run.

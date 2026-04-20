## Summary

- What changed?
- Why was this change needed?

## Scope

- [ ] CLI (`convert` / `markdown` / `reprocess`)
- [ ] Web API / WebSocket
- [ ] Docker / deployment
- [ ] Documentation only
- [ ] Other

## Validation

- [ ] `cargo fmt -- --check`
- [ ] `cargo clippy --features web -- -D warnings`
- [ ] `cargo test --features web`
- [ ] Docker CPU smoke path checked (or N/A)

## Stable Support Note

- [ ] I confirmed this change is validated for stable support scope:
      Windows 11 + WSL2 + Docker Compose
- [ ] If relevant, I documented limitations for CPU-only / ROCm paths

## Release Notes Label

Add at least one label for release categorization:

- `feature`, `enhancement`, `bug`, `fix`, `perf`, `performance`,
  `docs`, `ci`, `dependencies`, `refactor`, `breaking`, `chore`
- Use `skip-changelog` only when this PR should not appear in release notes.

## Checklist

- [ ] No unrelated files were modified
- [ ] Backward compatibility impact is described (if any)
- [ ] Docs updated (if behavior changed)

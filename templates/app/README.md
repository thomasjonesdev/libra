# Out-of-tree Libra app (PLAN-SDK S3)

This crate is **not** a member of the Libra workspace. Runtime deps are
`libra` + `serde`. It path-depends on `../../crates/*` so it builds from
a Libra checkout. Prefer `libra new myapp` — that writes this shape.

```sh
libra new myapp                 # path deps if this CLI was built here
libra new myapp --git https://github.com/thomasjonesdev/libra
cd myapp
libra check
libra test
libra build
libra package                   # needs cargo-packager
```

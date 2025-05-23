# Contributing

Welcome, we really appreciate if you're considering to contribute to `dra`!

## Development

1. Fork this repo
2. Clone your forked repo
   ```
   git clone https://github.com/<username>/dra && cd dra
   ```
3. Create a new branch with `git checkout -b my-branch`
4. Install [Rust stable](https://www.rust-lang.org/tools/install) and run `make install-components` to install `rustfmt`
   and `clippy`
5. Build project with `make build`

Other `make` targets:

- `test`: run unit tests
- `integration-tests`: run integration tests. Read the [docs](./tests/README.md) for more info.
- `format`: format code with `rustfmt`
- `format-check`: check code is properly formatted
- `lint`:  run `cargo-clippy`

### Code style

- Always format the code with `rustfmt`
- Split your changes into separate atomic commits (where the build, tests and the system are
  all functioning).
- Make sure your commits are rebased on the `main` branch.
- The commit message must have the format "category: brief description". The category should be the name of a rust
  module or directory.

  See this commits for reference:
    - [cli: add docs comment for --output option](https://github.com/devmatteini/dra/commit/8412dd1dcb16df3c489441d39a1774f7a8b2a495)
    - [ci: only run when something is pushed to main branch ](https://github.com/devmatteini/dra/commit/ad598100c73a2c2dd3a8195fb0364fe8b2bdeb35)

### Cross compilation

Install [cross](https://github.com/cross-rs/cross) to cross compile.

Export these environment variables:

```shell
export CARGO_BIN=cross
# Change CARGO_TARGET value with the desired target (https://doc.rust-lang.org/rustc/platform-support.html)
export CARGO_TARGET=arm-unknown-linux-gnueabihf
```

Then you can build and test (only unit tests) by running:

```shell
make build
make test
```

## License

By contributing, you agree that your contributions will be licensed under [MIT License](LICENSE).

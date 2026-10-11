# wmde-comp
Compositor for the WMDE desktop environment

## Development

This project uses [just](https://github.com/casey/just) as a command runner.

### Available Commands

#### Building
- `just build-debug` - Compile with debug profile
- `just build-release` - Compile with release profile (default)
- `just build-vendored` - Compile release profile with vendored dependencies
    - Requires vendoring first, which can be done with `just vendor`

#### Code Quality
- `just check` - Run clippy linter with pedantic warnings

#### Installation
- `just install` - Install the binary, the default schemas and the user unit (the session files come from wmde-session)
- `just uninstall` - Remove installed files

#### Cleanup
- `just clean` - Run `cargo clean`
- `just clean-dist` - Run `cargo clean` and remove vendored dependencies

#### Vendoring
- `just vendor` - Vendor dependencies locally and create vendor.tar
- `just vendor-extract` - Extract vendored dependencies from vendor.tar

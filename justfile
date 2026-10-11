name := 'wmde-comp'

rootdir := ''
prefix := '/usr'
sysconfdir := '/etc'

base-dir := absolute_path(clean(rootdir / prefix))
sysconfdir-dir := absolute_path(clean(rootdir / sysconfdir))

bin-dir := base-dir / 'bin'
lib-dir := base-dir / 'lib'
share-dir := base-dir / 'share'

bin-src := cargo-target-dir / 'release' / name
bin-dst := bin-dir / name

keybindings-src := 'data' / 'keybindings.ron'
keybindings-dst := share-dir / 'wmde' / 'fun.wmde.Settings.Shortcuts' / 'v1' / 'defaults'

tiling-exceptions-src := 'data' / 'tiling-exceptions.ron'
tiling-exceptions-dst := share-dir / 'wmde' / 'fun.wmde.Settings.WindowRules' / 'v1' / 'tiling_exception_defaults'

systemd-comp-service-src := 'data' / 'wmde-comp.service'
systemd-comp-service-dst := lib-dir / 'systemd' / 'user' / 'wmde-comp.service'

import 'cargo.just'

# Default recipe which runs `just build-release`
[private]
default: build-release

# WMDE: the compositor ships only its binary, its unit and the default schemas. The
# wayland-session entry, wmde-session[-pre].target and the session helper belong to
# wmde-session, so there is no install-bare-session recipe here.

# Installs files
install:
    install -Dm0755 {{bin-src}} {{bin-dst}}
    install -Dm0644 {{keybindings-src}} {{keybindings-dst}}
    install -Dm0644 {{tiling-exceptions-src}} {{tiling-exceptions-dst}}
    install -Dm0644 {{systemd-comp-service-src}} {{systemd-comp-service-dst}}

# Uninstalls installed files
uninstall:
    rm {{bin-dst}} {{keybindings-dst}} {{tiling-exceptions-dst}} {{systemd-comp-service-dst}}

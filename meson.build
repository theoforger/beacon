# SPDX-License-Identifier: GPL-3.0-or-later

project(
  'beacon',
  version: '0.1.0',
  license: 'GPL-3.0-or-later',
  meson_version: '>= 1.4.0',
)

i18n = import('i18n')
gnome = import('gnome')

application_id = 'com.theoforger.Beacon'
gettext_package = meson.project_name()

prefix = get_option('prefix')
bindir = prefix / get_option('bindir')
datadir = prefix / get_option('datadir')
localedir = prefix / get_option('localedir')

cargo = find_program('cargo')

subdir('data')
subdir('po')

cargo_env = environment()
cargo_env.set('BEACON_LOCALEDIR', localedir)

if get_option('buildtype') == 'release'
  cargo_profile = 'release'
else
  cargo_profile = 'debug'
endif

custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  console: true,
  output: 'beacon',
  env: cargo_env,
  install: true,
  install_dir: bindir,
  command: [
    find_program('build-aux/cargo-build.sh'),
    cargo,
    meson.project_source_root(),
    meson.project_build_root() / 'target',
    cargo_profile,
    '@OUTPUT@',
  ],
)

test(
  'cargo-test',
  cargo,
  args: [
    'test',
    '--workspace',
    '--manifest-path', meson.project_source_root() / 'Cargo.toml',
    '--target-dir', meson.project_build_root() / 'target',
  ],
  env: cargo_env,
  timeout: 600,
)

gnome.post_install(
  glib_compile_schemas: true,
  gtk_update_icon_cache: true,
  update_desktop_database: true,
)

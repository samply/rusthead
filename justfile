set shell := ["bash", "-cue"]

ARCH := `docker version --format '{{.Server.Arch}}'`
export BRIDGEHEAD_CONFIG_PATH := env("BRIDGEHEAD_CONFIG_PATH", "/etc/bridgehead")
CONFIG_FILE := BRIDGEHEAD_CONFIG_PATH / "config.toml"

run:
  cargo run

up: run
  #!/usr/bin/env bash
  srv_dir=$(cat {{ CONFIG_FILE }} | grep -v '#' | grep srv_dir | sed 's/.*=\s*\"\(.*\)\"/\1/')
  $srv_dir/bridgehead start

dockerize:
  cargo build --release
  mkdir -p artifacts/binaries-{{ ARCH }}/
  cp target/release/rusthead artifacts/binaries-{{ ARCH }}/rusthead
  docker build -t rusthead .

bootstrap:
  cp -n example.config.toml {{CONFIG_FILE}}
  echo "Change the site id in {{CONFIG_FILE}}"
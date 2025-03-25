set unstable
set shell := ["bash", "-cue"]

ARCH := `docker version --format '{{.Server.Arch}}'`
CONFIG_PATH := canonicalize(env("BRIDGEHEAD_CONFIG_PATH", "/etc/bridgehead"))
CONFIG_FILE := CONFIG_PATH / "config.toml"
SRV_PATH := canonicalize(shell("""cat {{ CONFIG_FILE }} | grep -v '#' | grep srv_dir | sed 's/.*=\\s*\\"\\(.*\\)\\"/\\1/'""")) || "/srv/docker/bridgehead"

run: build
  docker run --rm -v {{ SRV_PATH }}:{{ SRV_PATH }} -v {{ CONFIG_PATH }}:{{ CONFIG_PATH }} -e BRIDGEHEAD_CONFIG_PATH={{ CONFIG_PATH }} rusthead

up: down_bg run
  {{ SRV_PATH }}/bridgehead compose up

[private]
down_bg:
  {{ SRV_PATH }}/bridgehead compose down &

down:
  {{ SRV_PATH }}/bridgehead compose down

build:
  cargo build --release
  mkdir -p artifacts/binaries-{{ ARCH }}/
  cp target/release/rusthead artifacts/binaries-{{ ARCH }}/rusthead
  docker build -t rusthead .

bootstrap:
  cp -n example.config.toml {{ CONFIG_FILE }}
  echo "Change the site id in {{ CONFIG_FILE }}"
set unstable
set shell := ["bash", "-cue"]

ARCH := `docker version --format '{{.Server.Arch}}'`
CONFIG_PATH := canonicalize(env("BRIDGEHEAD_CONFIG_PATH", "/etc/bridgehead"))
CONFIG_FILE := CONFIG_PATH / "config.toml"
SRV_PATH := shell("""cat $1 | grep -v '#' | grep srv_dir | sed 's/.*=\\s*\\"\\(.*\\)\\"/\\1/'""", CONFIG_FILE) || "/srv/docker/bridgehead"

run: build
  docker run --rm -v {{ SRV_PATH }}:{{ SRV_PATH }} -v {{ CONFIG_PATH }}:{{ CONFIG_PATH }} -e BRIDGEHEAD_CONFIG_PATH={{ CONFIG_PATH }} samply/rusthead update

up: down_bg run
  {{ SRV_PATH }}/bridgehead compose up

[private]
down_bg:
  {{ SRV_PATH }}/bridgehead compose down &

down:
  {{ SRV_PATH }}/bridgehead compose down

bridgehead *args: run
  {{ SRV_PATH }}/bridgehead {{ args }}

build:
  cargo build --release
  mkdir -p artifacts/binaries-{{ ARCH }}/
  cp target/release/rusthead artifacts/binaries-{{ ARCH }}/rusthead
  docker build -t samply/rusthead .

bootstrap: build
  bash <(docker run --rm samply/rusthead bootstrap)
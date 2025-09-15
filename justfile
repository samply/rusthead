set unstable
set shell := ["bash", "-cue"]

ARCH := `docker version --format '{{.Server.Arch}}'`
CONFIG_PATH := canonicalize(env("BRIDGEHEAD_CONFIG_PATH", "./bridgehead"))
export TAG := env("TAG", "localbuild")

run: build
  docker run --rm -u "$(id -u bridgehead):$(id -g bridgehead)" -v {{ CONFIG_PATH }}:{{ CONFIG_PATH }} -e BRIDGEHEAD_CONFIG_PATH={{ CONFIG_PATH }} samply/rusthead:$TAG update

up: down_bg run
  {{ CONFIG_PATH }}/bridgehead compose up

[private]
down_bg:
  {{ CONFIG_PATH }}/bridgehead compose down &

down:
  {{ CONFIG_PATH }}/bridgehead compose down

bridgehead *args: run
  {{ CONFIG_PATH }}/bridgehead {{ args }}

build:
  cargo build --release
  mkdir -p artifacts/binaries-{{ ARCH }}/
  cp target/release/rusthead artifacts/binaries-{{ ARCH }}/rusthead
  docker build -t samply/rusthead:$TAG .

bootstrap: build
  bash <(docker run --rm samply/rusthead:$TAG bootstrap)

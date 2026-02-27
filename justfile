set shell := ["bash", "-cue"]

ARCH := `docker version --format '{{.Server.Arch}}'`
CONFIG_PATH := env("BRIDGEHEAD_CONFIG_PATH", "./bridgehead")
export IMAGE := env("IMAGE", "samply/rusthead:localbuild")

run: build ensure_bootstrap
  sudo {{ CONFIG_PATH }}/bridgehead install

up: down_bg run
  {{ CONFIG_PATH }}/bridgehead compose up

[private]
down_bg:
  {{ CONFIG_PATH }}/bridgehead compose down &

[private]
ensure_bootstrap:
  if ! {{ path_exists(CONFIG_PATH / "bridgehead") }}; then IMAGE=$IMAGE just bootstrap; fi

down:
  {{ CONFIG_PATH }}/bridgehead compose down

bridgehead *args: build ensure_bootstrap
  {{ CONFIG_PATH }}/bridgehead {{ args }}

build:
  cargo build
  mkdir -p artifacts/binaries-{{ ARCH }}/
  cp target/debug/rusthead artifacts/binaries-{{ ARCH }}/rusthead
  docker build -t $IMAGE .

bootstrap: build
  mkdir -p {{ CONFIG_PATH }}
  cd {{ CONFIG_PATH }} && bash <(docker run --rm $IMAGE bootstrap)

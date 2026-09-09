set shell := ["bash", "-cue"]
set positional-arguments

CONFIG_PATH := env("BRIDGEHEAD_CONFIG_PATH", "./bridgehead")
CONFIG_FILE := if CONFIG_PATH =~ '\.toml$' { CONFIG_PATH } else { CONFIG_PATH / "config.toml" }
BINARY := "target/x86_64-unknown-linux-musl/debug/rusthead"
export IMAGE := env("IMAGE", "samply/rusthead:localbuild")

# Build and install the local executable, including enrollment and systemd setup.
run: build bootstrap
  sudo {{ quote(BINARY) }} --config {{ quote(CONFIG_FILE) }} --no-self-update install

# Stop and start Compose sequentially using the native executable.
up: run
  {{ quote(BINARY) }} --config {{ quote(CONFIG_FILE) }} compose down
  {{ quote(BINARY) }} --config {{ quote(CONFIG_FILE) }} compose up

down:
  {{ quote(BINARY) }} --config {{ quote(CONFIG_FILE) }} compose down

# Forward arguments literally; update's restart-needed status (3) is successful.
bridgehead *args: build bootstrap
  status=0; {{ quote(BINARY) }} --config {{ quote(CONFIG_FILE) }} --no-self-update "$@" || status=$?; if [ "$status" -eq 3 ] && [ "${1:-}" = update ]; then exit 0; fi; exit "$status"

# The scratch distribution image requires a statically linked executable.
build:
  cargo build --locked --target x86_64-unknown-linux-musl
  mkdir -p artifacts
  cp {{ quote(BINARY) }} artifacts/rusthead
  docker build --platform linux/amd64 -t "$IMAGE" .

# Create an editable local configuration without overwriting existing settings.
bootstrap:
  if [ ! -e {{ quote(CONFIG_FILE) }} ]; then mkdir -p {{ quote(parent_directory(CONFIG_FILE)) }}; printf 'site_id = "local"\nhostname = "localhost"\nimage = "%s"\n' "$IMAGE" > {{ quote(CONFIG_FILE) }}; fi
  @echo "Local configuration: "{{ quote(CONFIG_FILE) }}" (edit it to enable modules)."

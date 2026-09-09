#!/usr/bin/env bash
set -euo pipefail

image=${IMAGE:-samply/rusthead:latest}
if [[ -n ${BRIDGEHEAD_CONFIG_PATH:-} ]]; then
    config_file=$BRIDGEHEAD_CONFIG_PATH
    [[ "$config_file" == *.toml ]] || config_file=$config_file/config.toml
    config_dir=$(dirname -- "$config_file")
    config_name=$(basename -- "$config_file")
else
    read -r -p 'Installation directory [.]: ' config_dir
    config_dir=${config_dir:-.}
    config_name=config.toml
fi
mkdir -p -- "$config_dir"
config_dir=$(cd -- "$config_dir" && pwd -P)
config_file=$config_dir/$config_name
create_config=true
if [[ -e "$config_file" || -L "$config_file" ]]; then
    create_config=false
    echo "Using existing configuration at $config_file"
else
    read -r -p 'Site ID: ' site_id
    default_hostname=$(hostname -f 2>/dev/null || hostname)
    read -r -p "Hostname [$default_hostname]: " hostname
    hostname=${hostname:-$default_hostname}
    read -r -p "HTTPS proxy [${HTTPS_PROXY:-None}]: " proxy
    proxy=${proxy:-${HTTPS_PROXY:-}}
fi

read -r -p 'Binary directory (ideally on PATH) [/usr/local/bin]: ' bin_dir
bin_dir=${bin_dir:-/usr/local/bin}

as_root=()
if ! mkdir -p -- "$bin_dir" 2>/dev/null || [[ ! -w "$bin_dir" ]]; then
    as_root=(sudo)
    sudo mkdir -p -- "$bin_dir"
fi
bin_dir=$(cd -- "$bin_dir" && pwd -P)
binary=$bin_dir/rusthead

tmp=$(mktemp -d)
container=
cleanup() {
    if [[ -n "$container" ]]; then docker rm "$container" >/dev/null || true; fi
    rm -rf -- "$tmp"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

if [[ ${BOOTSTRAP_SKIP_PULL:-0} != 1 ]]; then
    echo "Pulling $image..."
    docker pull --platform linux/amd64 "$image"
fi
container=$(docker create --platform linux/amd64 "$image")
docker cp "$container:/usr/local/bin/rusthead" "$tmp/rusthead"
docker rm "$container" >/dev/null
container=
chmod 755 "$tmp/rusthead"
"$tmp/rusthead" --version

managed_binary=$config_dir/.rusthead/bin/rusthead
mkdir -p -- "$config_dir/.rusthead/bin"
chmod 2770 "$config_dir/.rusthead"
install -m 755 -T -- "$tmp/rusthead" "$managed_binary"
if [[ "$binary" != "$managed_binary" ]]; then
    "${as_root[@]}" ln -sfnT -- "$managed_binary" "$binary"
fi

if "$create_config"; then
    cat > "$config_file" <<EOF
site_id = "$site_id"
hostname = "$hostname"
image = "$image"
EOF
    if [[ -n "$proxy" ]]; then
        echo "https_proxy_url = \"$proxy\"" >> "$config_file"
    fi
fi

printf '\nInstalled rusthead at %s\nConfiguration: %s\n' "$binary" "$config_file"
printf '\nEdit config.toml to enable your modules, then run:\n\n'
printf '  cd %q\n' "$config_dir"
printf '  sudo rusthead install\n'

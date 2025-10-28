#!/usr/bin/env bash
set -e

DEFAULT_CONFIG_DIR="."

# Check if config.toml exists
if [ ! -f "${DEFAULT_CONFIG_DIR}/config.toml" ]; then
    echo "Setting up configuration for bridgehead"

    read -p "Installation directory [$DEFAULT_CONFIG_DIR]: " config_dir
    config_dir="${config_dir:-$DEFAULT_CONFIG_DIR}"
    config_dir="$(readlink -f $config_dir)"

    read -p "Site ID: " site_id

    default_hostname=$(hostname -f)
    read -p "Hostname [$default_hostname]: " hostname
    hostname="${hostname:-$default_hostname}"

    # Create config.toml
    mkdir -p "$config_dir"
    cat > "${config_dir}/config.toml" << EOF
site_id = "$site_id"

hostname = "$hostname"
EOF

    read -p "Proxy [${HTTPS_PROXY:-None}]: " proxy
    proxy="${proxy:-$HTTPS_PROXY}"
    [ -n "$proxy" ] && echo "proxy = \"$proxy\"" >> "${config_dir}/config.toml"
    [ -n "$TAG" ] && echo "version_tag = \"$TAG\"" >> "${config_dir}/config.toml"

    echo "Configuration file created at ${config_dir}/config.toml"
else
    config_dir="$(readlink -f $DEFAULT_CONFIG_DIR)"
    echo "Using already provided configuration from ${config_dir}/config.toml"
fi

docker run --rm \
    -v $config_dir:$config_dir \
    -e BRIDGEHEAD_CONFIG_PATH=$config_dir \
    samply/rusthead:${TAG:-latest} update
sudo $config_dir/bridgehead install

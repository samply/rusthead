# Rusthead

A tool for generating docker compose files for the `Samply.Bridgehead` based on a simple toml config.

## Installation

### Prerequisites

- An x86_64 Linux host with Bash and Git installed.
- Docker with the Docker Compose plugin, running and accessible to your user.
- Sudo access to create the service account and configure systemd, if available.

### Setup

The rustehead can installed by running the installation wizard:

```bash
bash <(docker run --rm samply/rusthead bootstrap)
```

This will install the rusthead binary. This can then be used to do the final installtion by running the intall command in the config directory.

```bash
sudo rusthead install
```

After that you have an empty rusthead installation that does not start any services yet.

## Adding services

Run the commands below from your installation directory. Your account needs write
access to that directory and permission to use Docker. To run an update as the
service account, use `sudo -u bridgehead rusthead update commit`.

Edit `config.toml` to enable the services you need. See the
[example configuration](tests/configs/example.config.toml) for available options.
Then apply your changes:

```bash
rusthead update commit
```

This generates the service configuration, pulls the required container images,
and commits your configuration changes together with the generated files. You do
not need to make a Git commit first.

If the services require a new Beam network, enroll the installation and follow
the instructions printed by the command:

```bash
sudo rusthead enroll
rusthead update commit
```

Once enrollment is approved, start the services:

```bash
sudo systemctl start bridgehead
```

For an already running installation, use `sudo systemctl restart bridgehead`
after an update requests a restart.

## Configuration files

| File | What it is for |
| --- | --- |
| `config.toml` | Your site settings and enabled services. Edit this to configure the installation. |
| `config.local.toml` | Local credentials and generated secrets. Kept out of Git; preserve it when backing up the installation. |
| `docker-compose.override.yml` | Optional Docker Compose customizations. Kept out of Git. |
| `services/`, `.env`, `docker-image.lock.yml` | Generated service configuration and image versions. Change the configuration or Compose override instead of editing these files. |

For any custom changes to the Compose configuration, create a top-level
`docker-compose.override.yml` next to `config.toml`. Rusthead includes this file
when running Compose. Other Compose files are regenerated during updates, so
direct edits may be rejected or overwritten.

`config.local.toml` can contain initial plaintext passwords under
`basic_auth_users.<username>.pw`. Save these in a password manager; you can then
remove the plaintext values while keeping the password hashes.

Service data uses Docker named volumes by default. Set `volume_dir` in
`config.toml` to store it in a directory instead, such as `./data`. Keep backups of
service data as well as configuration and credentials; the Git history is not a
complete backup.

## Updating

There are two update commands:

| Command | When to use it |
| --- | --- |
| `rusthead update commit` | After editing configuration or Compose overrides. Applies and records your changes. |
| `rusthead update sync` | For routine updates when there are no pending local edits. |

Both commands update rusthead itself when a different binary is available in the
configured image, regenerate service configuration, and pull service images.
Updates leave your edits in place: `update sync` asks you to run `update commit`
when local changes need to be accepted. Manual edits to generated files are
rejected; move those customizations into your configuration or Compose override.

When an image pull fails, fix the reported problem and retry with
`rusthead update commit`.

On installations using systemd with Docker, `install` enables daily updates at
06:00 using `bridgehead-update.timer`. Services restart automatically when the
scheduled update requires it. Check the schedule and update logs with:

```bash
systemctl list-timers bridgehead-update.timer
journalctl -u bridgehead-update.service
```

To choose a different rusthead version or image for future updates, set `image`
at the top level of `config.toml`:

```toml
image = "samply/rusthead:latest"
```

For a new installation, you can also select the image during bootstrap:

```bash
IMAGE=samply/rusthead:my-tag bash <(docker run --rm samply/rusthead bootstrap)
```

The selected image is written into the new configuration. Bootstrap preserves an
existing configuration and can reinstall the rusthead command if needed.
Rusthead runs directly on the host; the services still run in Docker.

## Managing services

`rusthead compose` accepts Docker Compose commands and selects the installation's
configuration files for you:

```bash
rusthead compose ps
rusthead compose logs -f
```

If you manage services without systemd, start or recreate them with
`rusthead compose up -d`, and stop them with `rusthead compose down`.

## Reviewing changes and using a Git remote

The installation keeps a local Git history of `config.toml` and generated service
files. Use `git log` to review updates and `git show` to inspect the latest commit.
`update commit` includes all nonignored changes in the installation directory, so
keep unrelated files elsewhere.

Remote synchronization is optional and disabled by default. To enable it, set
`git_sync = true` at the top level of `config.toml` and configure a Git remote and
an upstream branch. With it enabled, `update sync` pulls upstream changes before
updating, and both update commands push their commits. Conflicting branch
histories must be resolved manually; rusthead does not stash your edits or merge
conflicts for you.

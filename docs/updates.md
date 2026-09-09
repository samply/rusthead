# Native installation updates

For a new x86_64 Linux installation with Docker available, run:

```sh
bash <(docker run --rm samply/rusthead bootstrap)
```

The script prompts for a site directory, site ID, hostname, optional HTTPS proxy,
and binary directory (default `/usr/local/bin`). It extracts the executable from
the scratch image and creates `config.toml`, then prints the sudo install command
to run after enabling the desired modules. Existing configuration files are never
overwritten; bootstrap can install the binary and link for an existing config too.
Bootstrap stores the binary in `.rusthead/bin/rusthead` and creates a symlink
in the selected binary directory.

To select the binary image and the image tracked by future self-updates, set
`IMAGE` for the host Bash process:

```sh
IMAGE=samply/rusthead:my-tag bash <(docker run --rm samply/rusthead bootstrap)
```

This value is written to `config.toml`; it does not need to be passed into the
container that prints the script.

Run commands against the installation directory or its selected configuration file:

```sh
rusthead --config /srv/bridgehead/config.toml update commit
rusthead --config /srv/bridgehead/config.toml update sync
```

`update commit` accepts your edited inputs, regenerates service definitions and
image pins, pulls the pinned images, and commits all nonignored installation
changes together. This includes staged, unstaged, and unrelated nonignored files;
use a dedicated installation repository. Customize configuration or
`docker-compose.override.yml`, rather than generated service files or `.env`.
Manual generated-file edits are rejected and preserved.

`update sync` requires a clean repository and unchanged local inputs. It is the
command used by the daily timer. Pending edits to ignored configuration,
public trust certificates, or overrides require an explicit `update commit` too. The
first update initializes a shared local Git repository and accepts the existing
inputs. An existing repository must be rooted at the installation directory;
worktrees are supported. A configured `volume_dir` inside the installation is
excluded from Git and input fingerprints. Use a dedicated subdirectory (such as
`./data`) or an external directory, not the installation root or an ancestor.
Previously tracked runtime data must be untracked before updating.

## Remote synchronization

Set `git_sync = true` at the top level of the configuration to enable network Git
operations. The default is `false`, even when a remote exists. Configure the
current branch's upstream with Git before enabling synchronization.

With synchronization enabled, `update sync` fetches the upstream branch and
fast-forwards before generation. A locally ahead branch can proceed. Both update
commands push successful local commits to the upstream; `update commit` never
pulls. The setting loaded at command start controls that invocation, even if a
pull changes it.

Divergent histories require an administrator to reconcile them using Git.
Rusthead never stashes, rebases, merges divergent branches, resets, or force-pushes.
A failed push preserves the local update and commit; retry with `update sync`
when the remote is reachable and the histories permit synchronization.

## Local state and recovery

Ignored `.rusthead/` metadata stores update fingerprints, an update lock, and a
separate `enrollment.json` containing completed enrollment records. It contains
no copies of secret contents and is accessible only to the installation owner
and group. `state.json` stores only ignored `local_inputs` and generated `outputs`,
with paths relative to the site. Git-tracked inputs are not fingerprinted.
Runtime changes are derived from outputs and local inputs other than
`config.local.toml`; there is no separate stored runtime map.
Runtime change detection includes generated files, pinned image
versions, environment values, overrides, and public trust certificates; a config-only commit
need not request a restart. No-op or ignored-only changes create no empty commits.

`enrollment.json` is durable operational state: resetting `state.json` must not
remove it. Required Beam networks are derived from the configured services and
are not persisted. The seed, credentials, and user configuration remain in
`config.local.toml`.

Enrollment saves each completed network independently. Receipt changes alone do
not dirty update inputs. `pki/` and `traefik-tls/` are excluded from fingerprints.
Custom TLS certificate/key paths are not added to the fingerprint inputs; keep
those files outside the repository or ignored by Git.
Their contents need not be readable by the user running updates, and rotation
does not block `update sync` or trigger a restart. Handle any required reload or
restart separately. TLS paths in configuration remain tracked inputs. A missing
site private key invalidates its enrollment records, retaining the existing
reenrollment behavior.

Generation renders all service templates before replacing service files, but the
whole update is not a filesystem transaction: certificate creation, output
writes, or a fast-forward can remain after a later failure. No successful update
commit is made when generation or image pulling fails. After an image failure,
`update commit` can retry outputs recorded by the failed run, provided nobody has
edited those outputs. Earlier write failures require inspecting partial outputs.

A migrated installation without a fingerprint baseline establishes one on its
first successful update and conservatively requests a restart. Untracked legacy
`.env` edits cannot be identified before this baseline exists.

Compose commands warn about uncommitted Git changes, local input changes, or
changed outputs, but do not detect committed configuration changes awaiting
generation. They remain usable, including diagnostics and shutdown with invalid
source configuration. Updates do not check for edits made while they run; the
update lock still prevents simultaneous rusthead updates. Generation, image pulling, and
Compose commands all use `docker compose`; pulling and launching include the
image lockfile after the override file.

## Automation status

| Exit status | Meaning |
| --- | --- |
| 0 | Successful update; runtime artifacts unchanged |
| 3 | Successful update; restart needed |
| 4 | Local update succeeded and needs restart, but push failed |
| 1 | Other update failure, or push failure without runtime changes |

The generated systemd update service accepts status 3 as success and requests a
restart for 3 or 4. Status 4 remains a service failure so the push problem stays
visible. Standalone enrollment key or certificate changes are accepted with
`update commit`; installation accepts these changes before enabling the timer.

## Updating the executable

Both update modes check the distribution image selected by the top-level `image`
configuration (default `samply/rusthead:latest`). Git preflight runs first; `sync`
also pulls configuration first, so a remote configuration can select a different
image tag or digest. Compose and enrollment commands do not self-update.

The updater pulls `linux/amd64`, inspects the immutable image ID, creates a stopped
container, and copies `/usr/local/bin/rusthead` out. It never starts the distribution
container. `.rusthead/binary.json` caches the image ID and executable SHA-256;
unchanged image IDs and executable content avoid extraction, and identical binary
content avoids replacement even when the image ID changes.

A changed executable must be an x86_64 ELF file and successfully report `--version`
before [self-replace](https://docs.rs/self-replace/latest/self_replace/) replaces
the running executable. The new version continues the requested update with the
same absolute configuration path and update mode; the original process returns
its exit status. An internal handoff prevents a second binary-update check during
that continuation. A binary replacement alone does not restart services: generated
runtime changes still determine the restart status.

Bootstrap creates the executable at `.rusthead/bin/rusthead` and a symlink in the
selected binary directory (default `/usr/local/bin`). `install` assigns ownership
to the `bridgehead` service account and points systemd at the managed binary. When
invoked from another executable, such as a development build, it copies that
executable into the managed location. It does not create or change PATH symlinks.
Manual commands through the symlink and systemd use the same binary; self-updates
replace the managed target and preserve the symlink.

Ensure the selected directory precedes other rusthead installations on `PATH`.
Bootstrapping another site into the same launcher directory points the command at
that site's managed binary. The launcher directory stays protected; only the managed
directory needs to be writable by `bridgehead`. Explicitly invoking some other
binary by its path still updates that copy instead. Pull, extraction, and
executable validation failures abort generation and leave the existing executable
in place.

The distribution image is `FROM scratch` and contains only the static executable.
The host needs Git and Docker with Compose. Secret synchronization runs
`docker.verbis.dkfz.de/cache/samply/secret-sync-local:latest` with an isolated cache
and read-only key/certificate mounts; host `proxy` and `local` programs are no
longer needed.

## Building and testing

CI builds and tests `x86_64-unknown-linux-musl`, then builds the size-optimized
release binary. It packages the tested release artifact using Samply's reusable
`docker-ci.yml` workflow, retaining its image-tag publishing conventions.

With Rust's musl target, `musl-gcc`, Docker Compose, and cargo-nextest installed:

```sh
cargo nextest run --locked --target x86_64-unknown-linux-musl --no-fail-fast
unshare --user --map-root-user cargo nextest run --locked --target x86_64-unknown-linux-musl --no-fail-fast --test install --run-ignored only
cargo build --locked --release --target x86_64-unknown-linux-musl
mkdir -p artifacts
cp target/x86_64-unknown-linux-musl/release/rusthead artifacts/rusthead
docker build --platform linux/amd64 -t rusthead-local .
docker run --rm rusthead-local --version
```

CI runs root-only installer tests with `sudo`; the local command above uses an
isolated user namespace. Self-update tests replace private executable copies and fake
Docker transport; they never replace the developer's compiled binary. CI also
smoke-tests the real scratch image and compares its extracted binary with the
release artifact.


## Local development with just

`just bootstrap` builds the local image and runs the interactive bootstrap script
if `bridgehead/config.toml` is missing. It prompts for site settings and a binary
directory, extracting from the local image without pulling from a registry.
Edit the config to enable the modules you want; subsequent commands preserve it.
Set `BRIDGEHEAD_CONFIG_PATH` to use another directory or a `.toml` file.

- `just build` builds the debug musl executable and the local scratch image
  (`IMAGE`, default `samply/rusthead:localbuild`).
- `just run` builds and installs the local executable with sudo, including
  enrollment and systemd setup.
- `just up` runs installation, then stops and starts Compose sequentially.
- `just down` stops Compose without rebuilding.
- `just bridgehead update commit` generates and accepts local configuration edits.
  Other CLI arguments can be passed through `just bridgehead` too.

Development recipes use `--no-self-update` so locally built binaries are not
replaced by a registry image. The installer forwards this option to its update
processes and generated systemd commands. Service image pulls and Git behavior
are unchanged. Install again without this option to enable scheduled binary
updates. The recipes run `static/bootstrap.sh`, the same script emitted by the
native `bootstrap` subcommand.

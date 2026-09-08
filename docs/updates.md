# Native installation updates

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
certificates, keys, or overrides require an explicit `update commit` too. The
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
and group. Runtime change detection includes generated files, pinned image
versions, environment values, overrides, and certificates; a config-only commit
need not request a restart. No-op or ignored-only changes create no empty commits.

`enrollment.json` is durable operational state: resetting `state.json` must not
remove it. Required Beam networks are derived from the configured services and
are not persisted. The seed, credentials, and user configuration remain in
`config.local.toml`.

Enrollment saves each completed network independently. Receipt changes alone do
not dirty update inputs; changes to actual keys or certificates still do. A missing
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

Compose commands warn about pending inputs but remain usable, including diagnostics
and shutdown with invalid source configuration. Generation, image pulling, and
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

# rilesc555's Sonora fork

This fork adds Spotify episodes to mixed playlists. Its `main` branch keeps the fork changes
and merges `sonorahq/sonora`'s `main`. Do not reset it to upstream or force-sync it.

## Updates

- `fork-upstream.yml` checks upstream daily at 08:17 UTC. It opens one `sync/upstream-*` pull
  request at a time. It never replaces the patched `main` branch.
- `fork-package.yml` checks the proposed merge with formatting, strict Clippy, the existing
  tests, and a Linux release build. Only a passing automated upstream PR is merged. If the
  head or base changes during the checks, the merge stops.
- A merge made with `GITHUB_TOKEN` does not trigger push workflows, so the sync run explicitly
  dispatches a new package run on `main`. Ordinary pushes to `main` also start the package run.
- Only a passing build of `main` is signed and published. Pull requests have no signing key.
  A failed build, conflict, or deployment leaves the last published repository in place.
- Packages and the pacman database are signed with the dedicated fork key. The package is
  `sonora-rilesc555-bin`, provides `sonora`, and conflicts with the upstream packages. It keeps
  `/usr/bin/sonora`, the Sonora launcher, and the existing user data paths.
- GitHub Pages serves the repository at `https://rilesc555.github.io/sonora/arch/x86_64`.
  GitHub Releases keeps signed packages from earlier builds for rollback.

Start an upstream check now:

```sh
gh workflow run fork-upstream.yml --repo rilesc555/sonora --ref main
```

Rebuild and publish the current `main`:

```sh
gh workflow run fork-package.yml --repo rilesc555/sonora --ref main
```

## Failed updates

An open upstream PR blocks later automatic updates. Inspect that PR and its Actions run.
If a dependency or build service failed, retry the workflow for the same PR:

```sh
gh workflow run fork-package.yml --repo rilesc555/sonora --ref main -f pull_request=NUMBER
```

For a merge conflict, fetch the PR branch, merge `origin/main` into it, resolve the conflict,
and push the resolution. Then run the same command above. Do not remove the podcast changes
just to make the merge pass. Closing the PR without merging allows a later daily run to try
again, but does not fix the conflict.

Versions have the form `0.47.0.r123.gabcdef123456-7`: upstream version, source commit count,
source commit, and package workflow run number. A new package does not install itself into a
running app. Restart Sonora after `omarchy update` installs one.

## This machine

The repository configuration is owned by the package at
`/etc/pacman.d/sonora-rilesc555.conf` and requires both package and database signatures.
`/etc/pacman.conf` includes it after the existing repositories. Normal `omarchy update` runs
update the fork through pacman; there is no local Rust build during an update.

Install `packaging/arch/omarchy-repository-hook.sh` as an Omarchy `pre-refresh-pacman` hook.
It restores only the include line if an Omarchy pacman configuration refresh removes it.
The root helper it calls is owned by the signed package at
`/usr/lib/sonora-fork/enable-repository`. No packaged Omarchy files are changed.

The public signing key and fingerprint are in `packaging/arch/signing-key.*`. The dedicated
private key and revocation certificate are in `~/.local/share/sonora-fork/signing`, with the
directory restricted to its owner. The private key is also the repository's `FORK_SIGNING_KEY`
Actions secret. It has no passphrase because CI must sign without an interactive prompt.
Keep an encrypted backup of that directory outside this machine. Never commit the private key
or print it in a log. If it is exposed, remove the secret, revoke the key, and replace the
local trusted key and published public key before publishing more packages.

The exact trusted fingerprint is:

```text
29C66D08EF19C8A3B5DAF5436095B3107B498FF0
```

## Rollback

Download a prior `.pkg.tar.zst` and its `.sig` from this fork's Releases, then run
`sudo pacman -U ./sonora-rilesc555-bin-VERSION-x86_64.pkg.tar.zst`. Pacman verifies its signature.
Temporarily add the package to `IgnorePkg` if it must stay at that version. Remove the hold
after a corrected build is available.

To stop using the fork, remove its Omarchy hook and repository include, then explicitly
replace `sonora-rilesc555-bin` with upstream's `sonora-bin`. Do not delete the Sonora data folder.

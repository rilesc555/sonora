# rilesc555's Sonora fork

This fork adds Spotify episodes to mixed playlists. Its `main` branch keeps the fork changes
and imports source changes from official stable releases of `sonorahq/sonora`, not its
development `main`. Do not reset the fork to upstream or force-sync it.

## Updates

- `fork-upstream.yml` checks GitHub's latest official release daily at 08:17 UTC. It rejects
  drafts, prereleases, and tags other than `vMAJOR.MINOR.PATCH`, then fetches the exact tag
  commit. It applies the change from the last imported stable release as one fork-owned commit,
  excluding `.github/workflows`. It opens a `sync/release-TAG-COMMIT` PR and retries that
  release's existing PR unless its checks are active. It never uses upstream `main` as an
  update target.
- `packaging/fork-upstream.json` records the last imported official tag and commit. The sync
  verifies both tags against the official repository and stops if the new release does not
  follow the recorded one. If the fork already contains a later commit from that release's
  history, the import starts after that commit instead of applying the same source twice.
- `fork-package.yml` checks the proposed merge with formatting, strict Clippy on Linux and
  Windows, the existing tests, and release builds for both platforms.
  Only a passing bot-created release PR is merged. Its one update commit must start at the
  current `main`, record the latest official tag and commit, and leave the fork workflows
  unchanged. If the head or base changes during the checks, the merge stops.
- The source job waits up to three minutes for GitHub to prepare a current merge ref, and stops
  immediately for a confirmed conflict. PR review runs use a separate concurrency group so
  they cannot cancel the dispatched automatic merge. GitHub may ask for approval of a bot's
  separate PR review run; the trusted dispatch on `main` does not depend on that approval.
- A merge made with `GITHUB_TOKEN` does not trigger push workflows, so the sync run explicitly
  dispatches a new package run on `main`. Ordinary pushes to `main` also start the package run.
- Publication reuses a successful same-repository PR or trusted dispatch build when its entire
  Git tree and workflow match `main`. Both platform artifacts and the source record must still
  exist. Missing records, expired artifacts, or different trees cause a full build instead.
  Reused binaries and the Windows installer keep their checked build version. Release notes
  identify both the published commit and the original build commit and run.
- Only checked source identical to `main` is signed and published. Pull requests have no signing key.
  A failed build, conflict, or deployment leaves the last published repository in place.
- The build uses both `rust-cache` for Cargo dependencies and `sccache` with GitHub's cache
  backend for compiler results. Cache statistics are printed even when a build fails. Test
  and release profiles keep separate compiler results; the final executable link is not cached.
- Packages and the pacman database are signed with the dedicated fork key. The package is
  `sonora-rilesc555-bin`, provides `sonora`, and conflicts with the upstream packages. It keeps
  `/usr/bin/sonora`, the Sonora launcher, and the existing user data paths.
- GitHub Pages serves the repository at `https://rilesc555.github.io/sonora/arch/x86_64`.
  GitHub Releases keeps signed packages from earlier builds for rollback.

Start an official release check now:

```sh
gh workflow run fork-upstream.yml --repo rilesc555/sonora --ref main
```

Rebuild and publish the current `main`:

```sh
gh workflow run fork-package.yml --repo rilesc555/sonora --ref main
```

## Windows

Each passing package run on `main` adds `Sonora-Setup.exe` to the same GitHub Release as the
Arch package. The installer supports x64 Windows. It uses the upstream installer identity,
shortcuts, and user data paths, so it can replace an upstream installation in place.
Its version includes `rilesc555` and the source commit. `SHA256SUMS` includes the installer.
The Windows installer has no code signature.

Download `Sonora-Setup.exe` from this fork's newest `fork-*` release and run it to install or
update. Fork builds do not check for upstream updates. Install later Windows builds with the
installer from a newer fork release.

## Failed updates

An open release PR is retried on the next sync while it remains the latest official release.
A newer official release can supersede a failed older release. Inspect a repeatedly failing
PR and its Actions run; no failing version is published.
If a dependency or build service failed, retry the workflow for the same PR:

```sh
gh workflow run fork-package.yml --repo rilesc555/sonora --ref main -f pull_request=NUMBER
```

For a source conflict, prepare and review a manual update in a maintenance branch. Keep the
fork workflow files, preserve the podcast changes, and update `packaging/fork-upstream.json`
to the exact official tag and commit. Manual updates use ordinary pull request checks and are
not merged automatically. A manual merge to the fork's `main` must still pass the package
workflow before publication.

The fork followed development `main` before this release-only policy was introduced. Those
earlier merges remain in history. Source-only imports do not add later upstream commits to the
fork's ancestry. The recorded tag and commit define the next stable-release change instead.

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

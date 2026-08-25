# Security Model

YNAB Pulse holds two things worth protecting:

1. **The YNAB Personal Access Token.** A PAT is not read-only — it grants full
   API access to every budget on the account. Treat it as a password.
2. **Cached financial data.** `~/.cache/io.github.elevate08.ynab-glance/overview_cache.enc`
   contains budget names, category balances, income, and spending history -
   sealed with the same per-install key as the token, so the file on disk is
   an envelope rather than readable JSON.

The adversary this design assumes is **another process running as you** (or as
another local user) on the same machine — a browser extension host, a compromised
AUR package, a shared workstation account. Remote attackers are covered by TLS;
local ones are the interesting case for a desktop widget.

## Trust boundaries

| Boundary | Untrusted input | Control |
|---|---|---|
| YNAB API → `ynab-cli` | JSON response body | Typed deserialization; response bodies capped at 16 MiB; `decimal_digits` clamped; currency symbols/separators stripped of control characters and capped at 8 chars; budget/group/category names stripped of control characters and capped at 120 chars; saturating money arithmetic; entity counts capped before rendering; error bodies truncated to 200 chars |
| `ynab-cli` stdout → QML | JSON text | Parsed in `try/catch`; every `Text` pins `textFormat: Text.PlainText`; `plainLabel` neutralizes shared button controls; no `RichText`, no `eval`, no dynamic QML |
| User keyboard → `ynab-cli` | The PAT | Passed on **stdin**, never argv; charset- and length-validated, then sealed with XChaCha20-Poly1305 before it reaches the keyring |
| Disk → `ynab-cli` | Encryption key file | Opened `O_NOFOLLOW|O_NONBLOCK`; must be a regular file of exactly 32 bytes and not group/world-accessible, or the token is treated as unreadable. Its directory is refused if it is a symlink or not owned by you |
| Local process → IPC socket | `tab(index)` and friends | Index clamped to the valid tab range |
| API data → desktop notification | Formatted currency text | Sent as an argument list; never through a shell |
| Disk → `ynab-cli` | Cache file | Opened `O_NOFOLLOW|O_NONBLOCK`, then checked through the descriptor: must be a regular file, ≤ 8 MiB, and not group/world-accessible, or it is ignored. Then AEAD-opened: a cache that fails the tag is deleted, never parsed |

## What the controls actually do

- **The token never appears in `argv`.** `/proc/<pid>/cmdline` is world-readable
  by default on Linux, so a token passed as an argument is legible to every
  process on the machine for as long as the helper runs — and lands in shell
  history for CLI users. `ynab-cli auth set` reads stdin and *refuses* an
  argument, so the unsafe form cannot be reintroduced by accident.
- **The keyring never holds the token itself.** It holds an envelope:
  XChaCha20-Poly1305, a 32-byte key generated per install, a fresh random
  192-bit nonce per write, with the version bytes authenticated as associated
  data. The key lives in
  `~/.local/share/io.github.elevate08.ynab-glance/token.key` (0600, in a 0700
  directory - under the plugin id rather than `omarchy/`, which Omarchy
  symlinks to its root-owned system install), so a process that dumps the
  Secret Service gets ciphertext and nothing else. See *What the encryption layer is and is
  not* below - this is a real but bounded control.
- **The envelope is stored in the Secret Service** (`org.freedesktop.secrets` -
  GNOME Keyring, KWallet, or any compliant implementation), never in a
  dotfile. The Secret Service is what makes the credential outlive a reboot,
  encrypted at rest under your login password. The kernel keyutils keyring is
  not used: it does not survive a reboot, and its persistent keyring expires
  on a timeout of its own.
- **Revoking destroys the key along with the credential.** `auth clear`
  removes the keyring entry, deletes `token.key`, and purges the cache - so a
  keyring entry recovered later from a backup is not openable. In process memory it is held in `Zeroizing<String>`, and
  the API client keeps no copy — it lives only in a header marked sensitive.
- **Redirects are refused outright** (`Policy::none()` plus `https_only`). The
  YNAB API does not redirect, and refusing them removes every path by which the
  bearer token could be replayed to another host.
- **Budget ids are validated as UUIDs** at three points: the CLI flag, each API
  call that interpolates one into a request path, and the QML that builds a URL
  for `xdg-open`. An id carrying `?`, `#`, or `../` would otherwise silently
  retarget a token-bearing request or steer the browser.
- **Remote names cannot trip Qt into rendering markup.** Qt's `Text` defaults to
  `Text.AutoText`, which parses incoming strings as HTML the moment they contain
  markup like `<img src="...">`. If a collaborator on a shared budget names a
  category or budget with an image tag pointing to a loopback or remote host, Qt
  would trigger network requests to fetch the resource. Every `Text` element
  across the plugin's QML explicitly pins `textFormat: Text.PlainText`, and
  labels passed to shared UI kit controls (like `Button`) route through
  `Model.plainLabel()`, which HTML-escapes markup and safely wraps entities to
  prevent rich-text execution.
- **No API data ever reaches a shell.** Every subprocess is spawned with an
  argument list. This matters most for the two right-click notifications (the
  panel's and the bar widget's): the Omarchy bar's `run()` helper executes
  through `bash -lc`, and the amounts they display are assembled from the
  currency symbol and separators supplied by the API response. Built as a shell
  string, a symbol of `$(...)` would have run as a command on right-click;
  built as argv, it is inert text. The symbols are additionally sanitized where
  they enter the backend.
- **Upstream cannot exhaust the machine.** Response bodies are refused above
  16 MiB (checked against `content-length` and again on a short read, so a
  chunked body that lies about its size is still caught), and the number of
  category groups, categories, and pie slices turned into panel content is
  capped — an unbounded response would otherwise make the shell build delegates
  until the bar hangs.
- **The cache cannot be swapped out from under the read.** The file is opened
  with `O_NOFOLLOW` and `O_NONBLOCK` and then inspected through the resulting
  descriptor, so a symlink planted in its place is refused by the kernel, a
  FIFO fails instead of hanging the widget on an open that waits for a writer,
  and there is no window between the check and the read in which another local
  process could substitute a different file.
- **Both state directories are named for the plugin id.** Not `omarchy/`:
  Omarchy symlinks `~/.local/share/omarchy` to its root-owned system install,
  which made the key file impossible to create, and a cache under the same
  name would break the same way if `~/.cache/omarchy` ever became a symlink -
  silently, because cache write failures are deliberately non-fatal. A cache
  left over from before the rename is deleted on the next successful write and
  by `auth clear`, so no readable copy of the budget survives the upgrade.
- **The cache is sealed too, with the same key.** The panel's data is as
  personal as the credential that fetches it - category names ("Legal fees",
  "Therapy") often more so than the numbers - and a scraper after financial
  data has no reason to go near the keyring when a readable copy is sitting in
  `~/.cache`. Sealing it closes that asymmetry. The AEAD tag also means a cache
  another local process rewrote is discarded rather than displayed: a forged
  "Ready to Assign" on your bar is a plausible nudge, not a harmless prank.
- **The payload carries no account identifier.** `user_id` used to ride along
  in the cached payload and nothing read it; the settings screen takes it from
  `auth status` instead. Dropping it also removed a `/user` API call per fetch.
- **Revoking the token erases the cache.** `auth clear` deletes the stored
  credential *and* the cached financial data — disconnecting should not leave a
  readable copy of your budget on disk.

## What the encryption layer is and is not

**It is** protection against untargeted credential theft. The Secret Service is
unlocked for as long as you are logged in, and any process running as you can
enumerate it over D-Bus. A stealer that walks the keyring harvesting anything
token-shaped - the common case, and the one that does not know or care which
application an item belongs to - comes away with an AEAD envelope.

**It is not** protection against an attacker who is looking for *this plugin*.
The key sits in a file readable by anything running as you, which is the same
reach that let it read the keyring in the first place. Two files instead of one
is a higher bar, not a wall.

Real separation would need key material that same-UID code cannot reach at
will: a passphrase typed at unlock, a TPM-sealed key, or a FIDO2 token. Each of
those requires you to be present when the token is read, which a bar widget
that refreshes on a timer cannot ask for. That trade - unattended refresh over
a stronger key boundary - is the deliberate choice this plugin makes.

## The bundled prebuilt helper

The plugin ships `bin/ynab-cli` as a compiled binary inside the repository,
which deserves an honest accounting of what you are being asked to trust.

**The repository is already the trust anchor.** Installing this plugin runs
arbitrary, unsandboxed QML from this repo inside your long-lived
omarchy-shell process - `omarchy plugin add` prints exactly that warning
before cloning. Code in `Panel.qml` handles your token end to end: it reads
it from the settings UI, pipes it to the helper, and renders whatever the
helper reports. Anyone in a position to swap a malicious binary into this
repo can equally commit a malicious `Panel.qml`, so the prebuilt helper does
not create a new trust requirement - the repo itself was always the thing
you were trusting.

**The binary is only ever born in CI.** `bin/ynab-cli` stays listed in
`.gitignore`, and the only commits that touch it are made by this repo's
GitHub Actions workflow after clippy, tests, and a smoke test pass on the
exact source commit named in the commit message (`ci: bundle ynab-cli
<sha>`). Hand-uploaded binaries are excluded by construction rather than by
policy. Tagged releases go through the same pipeline plus packaging and
checksums.

**You can verify what actually landed on your disk:**

```bash
PLUGIN=~/.config/omarchy/plugins/io.github.elevate08.ynab-glance

# Sigstore build provenance, anchored to this repo's OIDC identity:
gh attestation verify "$PLUGIN/bin/ynab-cli" --repo Elevate08/qs-ynab-api

# Byte-for-byte comparison against a tagged release's artifacts:
sha256sum "$PLUGIN/bin/ynab-cli"   # vs SHA256SUMS on the release page
```

**The source-build escape hatch remains.** If you would rather compile the
code you audited than audit the bytes you downloaded, `./build.sh` in the
plugin directory rebuilds the helper from the checkout in front of you;
`git restore bin/ynab-cli` returns to the CI-built version. The committed
`Cargo.lock` pins dependencies for both paths, and CI runs `cargo audit`
against it.

## Data handling

- Financial data is cached locally and never leaves the machine except in
  requests back to `api.ynab.com`. There is no telemetry, no analytics, and no
  third-party endpoint.
- The cache is refreshed on every successful fetch and removed by `auth clear`.
  To erase everything manually:
  ```bash
  ynab-cli auth clear
  rm -rf ~/.cache/io.github.elevate08.ynab-glance \
         ~/.local/share/io.github.elevate08.ynab-glance
  ```
  Deleting the key alone is enough to make the cache unreadable, but delete
  both: an envelope nobody can open is still a copy of your budget.

## Known limitations

- **The helper path is not configurable.** `ynab-cli` is resolved relative to
  the installed plugin directory, with no fallback path and no environment
  override, so the binary that receives your token cannot be redirected by
  anything that can set your environment. If that resolution ever fails, the
  helper does not run at all.
- **The keyring is only as strong as your session.** Once the Secret Service is
  unlocked, any process running as you can read the token from it - through
  D-Bus, or out of the keyutils cache with `keyctl`. This plugin
  cannot fix that; it only avoids *widening* the exposure.
- **`omarchy-notification-send` and `xdg-open` are resolved via `PATH`.**
  Standard for desktop tooling, but a poisoned `PATH` is a poisoned desktop.

## Reporting

Please report security issues by opening a GitHub issue marked `security`, or
privately to the maintainer if the issue is exploitable as described above.

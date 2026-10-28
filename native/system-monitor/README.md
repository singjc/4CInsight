# 4C Insight native system monitor (UX7)

This is an **optional**, read-only companion. It uses native Rust `sysinfo` to report system RAM and CPU use to the existing browser application. It does not access cohort files or add browser/Rayon threads. 4C Insight continues to work without it.

## Run on Linux, macOS, or Windows

From the 4C Insight Git checkout:

```bash
cargo fmt --manifest-path native/system-monitor/Cargo.toml
cargo check --manifest-path native/system-monitor/Cargo.toml
cargo test --manifest-path native/system-monitor/Cargo.toml
cargo run --release --manifest-path native/system-monitor/Cargo.toml
```

Keep the terminal open. It prints a new **32-character session token** each time it starts.

1. Open <https://singjc.github.io/4CInsight/>.
2. Open **Settings > Optional native system monitor**.
3. Paste the token and click **Connect native monitor**. If your browser asks to allow access to the local loopback service, approve it only if you started this helper yourself.
4. Read total/available/used host RAM, total CPU usage, logical CPU count, and swap. Readings update about every three seconds while connected.
5. Click **Disconnect** to clear the token from app memory. Press `Ctrl+C` in the helper terminal to stop it.

For local site development on port 8000:

```bash
cargo run --release --manifest-path native/system-monitor/Cargo.toml -- --allow-origin http://127.0.0.1:8000
```

The helper is not a separate science engine. RAM readings refer to the host as reported by `sysinfo`, not the Firefox process, WASM allocation, or cgroup limits. Neither reading sets hard memory or CPU quotas.

## Connection and security

The helper binds **only `127.0.0.1:47831`**. It answers only `GET /v1/metrics`, accepts browser requests only from the selected exact `Origin` and the correct loopback `Host`, and requires a per-launch random bearer token. CORS `OPTIONS` preflight is supported for that origin. No cookie, storage, filesystem browsing, or write endpoint exists. The browser does **not** store the token in `localStorage` or save states. Metrics stay on the local computer and are not uploaded by 4C Insight.

Browsers can still block a public HTTPS website's request to a local helper. Current Chrome/Edge variants may ask for loopback-network access permission. Firefox can behave differently across versions and policy settings. A failed connection never prevents scientific analyses; the browser retains the manual RAM fallback.

You can inspect a running helper with a valid token (replace the token locally with the one it printed):

```bash
curl --fail-with-body -H 'Origin: https://singjc.github.io' -H 'Authorization: Bearer <session-token>' http://127.0.0.1:47831/v1/metrics
```

Never publish or paste session tokens into issue reports or chat. The token expires when the monitor process exits.

## Tests

```bash
node --test app/tests/*.test.mjs
cargo fmt --manifest-path native/system-monitor/Cargo.toml -- --check
cargo check --manifest-path native/system-monitor/Cargo.toml
cargo test --manifest-path native/system-monitor/Cargo.toml
```

Native Rust compilation must be validated on a machine with Rust installed. Static JavaScript tests cannot prove that the localhost helper works in a particular browser.

# Ubuntu VPS Setup

This is the production runbook for installing the MelCloud runtime on an Ubuntu VPS after cloning this repository from GitHub.

Target assumptions:

- Ubuntu 24.04
- native build on the VPS
- no Docker
- install path: `/opt/melcloud`
- service user: `melcloud`
- site bind is configured in `melcloud-site/melcloud-site.yaml`
- current site port: `8787`

## 1. Install prerequisites

```bash
sudo apt update
sudo apt install -y build-essential pkg-config curl git ca-certificates rsync
curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal --default-toolchain stable
source "$HOME/.cargo/env"
rustc --version
cargo --version
```

On a small VPS, keep builds single-threaded:

```bash
export CARGO_BUILD_JOBS=1
```

If the build is killed by the kernel because RAM is too low, add temporary swap before rebuilding.

## 2. Clone and configure

```bash
git clone <repo-url> melcloud
cd melcloud
```

Create `.env` in the repository root:

```dotenv
login=your@email
password=your-password
language=ru
```

Values may also be quoted, for example `password="your password"`.

Confirm the site port before building:

```bash
grep '^bind_addr:' melcloud-site/melcloud-site.yaml
```

Expected:

```text
bind_addr: 0.0.0.0:8787
```

## 3. Build the runtime package

```bash
chmod +x ./build.sh
CARGO_BUILD_JOBS=1 ./build.sh
```

Expected runtime output:

```text
build/
  bin/melcloud-cli
  bin/melcloud-site
  melcloud-cli/presets/
  melcloud-cli/state/
  melcloud-site/state/
  melcloud-site/cache/
  melcloud-site/public/
  melcloud-site/site-assets/
  melcloud-site/melcloud-site.yaml
```

## 4. Smoke check before install

Run from the repository root:

```bash
./build/bin/melcloud-cli auth test
./build/bin/melcloud-cli devices sync
./build/bin/melcloud-cli status --json
./build/bin/melcloud-cli preset list --json
```

Start the site in the foreground:

```bash
cd build
./bin/melcloud-site
```

In another shell:

```bash
curl http://127.0.0.1:8787/api/state
```

Stop the foreground process with `Ctrl+C` after the smoke check.

## 5. Install to `/opt/melcloud`

```bash
sudo useradd --system --home /opt/melcloud --shell /usr/sbin/nologin melcloud || true
sudo mkdir -p /opt/melcloud
sudo rsync -a build/ /opt/melcloud/
sudo chown -R melcloud:melcloud /opt/melcloud
```

The service writes runtime state under:

```text
/opt/melcloud/melcloud-cli/presets/
/opt/melcloud/melcloud-cli/state/
/opt/melcloud/melcloud-site/state/
/opt/melcloud/melcloud-site/cache/
```

## 6. Configure systemd

Create `/etc/systemd/system/melcloud-site.service`:

```ini
[Unit]
Description=MelCloud Site
Wants=network-online.target
After=network-online.target

[Service]
Type=simple
User=melcloud
Group=melcloud
WorkingDirectory=/opt/melcloud
ExecStart=/opt/melcloud/bin/melcloud-site
Restart=on-failure
RestartSec=5
NoNewPrivileges=true
PrivateTmp=true

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now melcloud-site
sudo systemctl status melcloud-site
```

Logs:

```bash
sudo journalctl -u melcloud-site -f
```

Restart after updating `/opt/melcloud`:

```bash
sudo systemctl restart melcloud-site
```

## 7. Network access

The app listens on `0.0.0.0:8787` by config. Restrict access at the VPS firewall to the trusted home public IP.

Example with `ufw`:

```bash
sudo ufw allow from <HOME_PUBLIC_IP> to any port 8787 proto tcp
sudo ufw status verbose
```

Do not expose this service broadly unless a separate reverse proxy/auth layer is added.

## 8. Upgrade flow

```bash
cd ~/melcloud
git pull
CARGO_BUILD_JOBS=1 ./build.sh
sudo rsync -a build/ /opt/melcloud/
sudo chown -R melcloud:melcloud /opt/melcloud
sudo systemctl restart melcloud-site
sudo systemctl status melcloud-site
```

Optional full test pass before install:

```bash
cargo test --workspace
cargo test -p melcloud-site -- --ignored
npm --prefix melcloud-site run test:js
```

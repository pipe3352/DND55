# Running a DND55 relay server

The relay lets the DM host a game for players in other houses **without port forwarding**. The DM's
game and every player make an *outgoing* connection to the relay, and the relay pipes bytes between
them. It never sees the DM's secret, holds no campaign data, and can't change the game. It does see
the (unencrypted) traffic. See "Security" below.

```
 Player A ──►┐
 Player B ──►├──► relay.example.com:7979 ◄── DM's PC (hosts the game, registers on the relay)
 Player C ──►┘
```

## What you need

* A machine with a **public IP address** and one open **TCP port** (default 7979). Options:
  * a small Linux VPS: 1 vCPU and 512 MB–1 GB RAM is plenty. Typical price is **about 4–6 USD/€ per month**
    (Hetzner, DigitalOcean, Vultr, OVH, …). Some clouds have "always free" small VMs; check their
    current terms yourself.
  * any PC whose router forwards TCP 7979 to it (a friend's PC, a home server…).
* Bandwidth: a few KB/s per connected player. Monthly traffic for a weekly group is tiny.

## Option 1 — Linux server with systemd (recommended)

On your own PC (or on the server) build the relay:

```bash
cargo build --release -p dnd_relay        # produces target/release/dnd55-relay
```

Copy it to the server and install it as a service:

```bash
sudo install -m 755 dnd55-relay /usr/local/bin/dnd55-relay
sudo useradd --system --no-create-home dnd55relay
sudo cp packaging/relay/dnd55-relay.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now dnd55-relay
sudo systemctl status dnd55-relay      # should say "active (running)"
```

Open the port in the server firewall (and in the cloud provider's firewall/security group, if it has one):

```bash
sudo ufw allow 7979/tcp
```

## Option 2 — Docker

_(The Dockerfile was written for this release but could not be built in the development environment, which has no Docker daemon. The systemd route above is the tested one: the `dnd55-relay` binary was built and started there.)_

```bash
docker build -f packaging/relay/Dockerfile -t dnd55-relay .
docker run -d --restart unless-stopped -p 7979:7979 --name dnd55-relay dnd55-relay
```

## Option 3 — Windows

`DND55-Relay-Windows.zip` contains `dnd55-relay.exe`. Double-click it (a console window shows the
address it listens on) and allow it through Windows Firewall. The PC needs a public IP or a router
port-forward of TCP 7979. This option suits testing more than a permanent setup.

## Using it

1. DM: in the game, click **✉ Invite players**, type the relay address (for example
   `relay.example.com:7979` or `203.0.113.10:7979`) and click **Go online**.
2. The status turns green and a **Internet (relay)** invite appears. **Copy** it and send it to the players.
3. Players paste the invite into **Join Campaign**. That's all.

If the relay restarts, the DM's game re-registers automatically. The invite code then changes, so send
the new invite.

## Limits and behaviour

* Up to 200 hosted games and 16 simultaneous players per game (constants in `crates/dnd_relay`).
* A joining player waits up to 10 s for the DM's game to accept; otherwise they get a clear error.
* Unknown invite codes are rejected with "no game with that invite code".

## Security

* Traffic is **not encrypted** (plain TCP). The relay operator, or anyone on the path, could read game
  messages. Run the relay yourself or use one run by someone you trust. TLS is on the roadmap.
* Invite codes are random and short-lived (they change when the DM re-hosts). Players still need the
  session code, and DM powers require a secret that never leaves the DM's PC.
* The relay only forwards bytes and does not parse game data, so a malicious relay can't do more than
  read, drop or replay traffic. It cannot gain DM rights.

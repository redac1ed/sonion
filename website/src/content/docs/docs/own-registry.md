---
title: Run your own registry
description: Run a private or separate registry.
---

Most people should use the default registry. Run your own for testing or for a separate network with its own domains.

```sh
sonion registry --data-dir ./registry-data --cert fullchain.pem --key privkey.pem --addr 0.0.0.0:6767
```

`--addr` defaults to `127.0.0.1:6767`, so set it to accept outside connections. The registry also serves the `domains.son` page at its root.

## Use it

Clients need to trust its certificate, so use a real certificate on a hostname. Then point the CLI at it:

```sh
sonion register --registry sonion://registry.example.com:6767 --username you --password "something long"
```

In the [browser settings](/docs/browser/), set `registry` to the same address and `pin_key` to the registry's public key:

```sh
sonion fetch sonion://<your-ip>:6767/v1/key --insecure-dev
```

The `key` value in the response is what goes in `pin_key`. Without a pin, the browser trusts whichever signing key the registry sends.

## The domains page

To get the page at a domain name, register one on your registry that points at the registry's own hostname:

```sh
sonion put-domain domains.son --registry sonion://registry.example.com:6767 --api-key <key> --endpoints registry.example.com:6767
```

## Back up your data

`--data-dir` holds two files:

- **`db.json`:** all accounts and domains.
- **`signing.key`:** the key that signs domain records. If you lose it, the registry creates a new one, and browsers with the old key pinned reject every domain.

## TLDs

The allowed TLDs are built into the binary. To use your own list, point `SONION_TLDS` at a JSON file:

```json
["son", "rizz", "example"]
```

## Limits

Per IP address, the registry allows 5 new accounts per hour and 20 domain changes per 10 minutes. After 5 failed logins, login is locked for 60 seconds.

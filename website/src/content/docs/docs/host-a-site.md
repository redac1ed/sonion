---
title: Host a site
description: Put a folder of files online at your own sonion domain.
---

You need a machine with a public IP address, with port `6767` open and the [CLI](/docs/install/) installed on it.

## 1. Create a certificate

```sh
sonion cert --host <your-ip> --out ./certs
```

This writes `certs/cert.pem` and `certs/key.pem`, and prints the certificate's fingerprint.

## 2. Start the server

```sh
sonion serve --root ./my-site --cert certs/cert.pem --key certs/key.pem
```

Your folder needs an `index.html`. Keep this running with `systemd`, `tmux` etc.

## 3. Create an account

```sh
sonion register --username you --password "something long"
sonion login --username you --password "something long"
```

`login` prints an API key. Use it in the next step.

## 4. Register your domain

```sh
sonion put-domain yourname.son --api-key <key> --endpoints <your-ip>:6767 --cert certs/cert.pem
```

The `--cert` flag matters. It publishes your certificate's fingerprint in the domain record, so browsers can trust your self-signed certificate. Without it, your site won't open.

## 5. Open it

Go to `sonion://yourname.son` in the browser.

## How files are served

- `/about/` serves `about/index.html`, and `/about` redirects to `/about/` if that folder exists.
- A missing path without a file extension serves your root `index.html`, so single-page apps work.
- A missing path with an extension returns a `404`. Add `404.html` to your folder to customise it.
- Files are limited to 10 MB.
- Static sites accept `GET`, `HEAD` and `OPTIONS`. For anything else, [run a backend](/docs/backend/).

You can also register and manage domains on the `domains.son` using its own [API](/docs/domains/) instead of the CLI.

---
title: Certificates
description: Self-signed certificates, pinning and real certificates.
---

Every sonion connection uses TLS 1.3, so every server needs a certificate. You have two options: 

## Self-signed with a pin

This is the easy option and works with just an IP address.

```sh
sonion cert --host <your-ip> --out ./certs
sonion put-domain yourname.son --api-key <key> --endpoints <your-ip>:6767 --cert certs/cert.pem
```

The domain record now contains the SHA-256 fingerprint of your certificate. The registry signs that record, and the browser only accepts a server with that exact certificate.

To see a certificate's fingerprint:

```sh
sonion fingerprint certs/cert.pem
```

On `domains.son`, paste that value into **Certificate fingerprint**.

## A real certificate

If you have a hostname and a certificate from an authority like Let's Encrypt, you do not need a pin. Use the hostname in your endpoint so the certificate's name matches:

```sh
sonion serve --root ./my-site --cert fullchain.pem --key privkey.pem
sonion put-domain yourname.son --api-key <key> --endpoints example.com:6767
```

## Changing certificates

With a pin, your site stops opening as soon as the certificate changes until you update the record. Do both together:

```sh
sonion cert --host <your-ip> --out ./certs
sonion put-domain yourname.son --api-key <key> --endpoints <your-ip>:6767 --cert certs/cert.pem
```

Then restart your server. If you run `put-domain` later without `--cert`, the existing pin is kept.

To remove a pin, clear the fingerprint box on `domains.son`.

## Old browsers

Browsers older than `v0.3.0` reject pinned domains with "record failed verification". Update the browser to fix it.

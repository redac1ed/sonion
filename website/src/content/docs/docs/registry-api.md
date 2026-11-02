---
title: Registry API
description: The JSON API served by a sonion registry.
---

The registry communicates in JSON over `sonion://`. The default one is at `sonion://registry.sonion.redac.me:6767`.

Routes marked **auth** need an `Authorization: Bearer <api key>` header. Errors look like `{"error": "message"}`.

## Accounts

|            Route             |            Body            |          Returns          |
| ---------------------------- | -------------------------- | ------------------------- |
| `POST /v1/accounts`          | `{"username", "password"}` | `{"ok": true}`            |
| `POST /v1/login`             | `{"username", "password"}` | `{"api_key", "username"}` |
| `POST /v1/logout` (**auth**) |                            | `{"ok": true}`            |

## Domains

|                 Route                  |              Returns               |
| -------------------------------------- | ---------------------------------- |
| `GET /v1/domains` (**auth**)           | `{"domains": [...]}`, your domains |
| `GET /v1/domains/<name>` (**auth**)    | One of your domains                |
| `PUT /v1/domains/<name>` (**auth**)    | `{"ok": true, "expires_at"}`       |
| `DELETE /v1/domains/<name>` (**auth**) | `{"ok": true}`                     |


The `PUT` body:

```json
{
  "endpoints": ["203.0.113.7:6767"],
  "ttl": 300,
  "cert_sha256": "64 lowercase hex characters",
  "description": "One line about your site",
  "listed": true
}
```

Only `endpoints` is required and the fields you leave out keep their current values. An empty `cert_sha256` removes the pin. 

Every `PUT` renews the domain for a year.


## Public lookups

|          Route           |                         Returns                         |
| ------------------------ | ------------------------------------------------------- |
| `GET /v1/resolve/<name>` | The signed record below                                 |
| `GET /v1/whois/<name>`   | `{"registered": false}`, or the domain's public details |
| `GET /v1/sites`          | `{"sites": [{"name", "description"}]}`                  |
| `GET /v1/tlds`           | `{"tlds": [...]}`                                       |
| `GET /v1/key`            | `{"key"}`, the registry's public signing key            |
| `GET /`                  | The `domains.son` page                                  |

## Signed records

```json
{
  "name": "meow.balls",
  "endpoints": ["203.0.113.7:6767"],
  "expires_at": 1825076457,
  "ttl": 300,
  "not_before": 1793540457,
  "key": "base64 Ed25519 public key",
  "signature": "base64 signature",
  "cert_sha256": "64 lowercase hex characters"
}
```

The signature is `Ed25519` over these fields joined with `|`:

```
name|endpoints|expires_at|not_before|ttl
```

`endpoints` is joined with commas. If the record has a `cert_sha256`, it is added as one more field at the end.

Clients should reject a record if the signature fails, if it has expired or if `not_before` is more than 5 minutes in the future.

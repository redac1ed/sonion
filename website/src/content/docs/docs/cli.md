---
title: CLI
description: Every sonion command and its options.
---

Commands that communicate to a registry accept `--registry <url>`, and if they use `SONION_REGISTRY`, then your [config file](/docs/browser/). They also accept `--insecure-dev`, which turns off certificate checks for local testing.

## Sites

### `sonion cert`

Creates a self-signed certificate and prints its fingerprint.

|     Option      |   Default   |                                              |
| --------------- | ----------- | -------------------------------------------- |
| `--host <name>` | `localhost` | Hostname or IP. Repeat it for more than one. |
| `--out <dir>`   | `./certs`   | Where to write `cert.pem` and `key.pem`.     |

### `sonion fingerprint <cert>`

Prints the SHA-256 fingerprint of a PEM certificate.

### `sonion serve`

Serves a folder or forwards to your app.

|        Option         |    Default     |                                                 |
| --------------------- | -------------- | ----------------------------------------------- |
| `--root <dir>`        | required       | The folder to serve.                            |
| `--cert <file>`       | required       | Certificate PEM.                                |
| `--key <file>`        | required       | Private key PEM.                                |
| `--addr <ip:port>`    | `0.0.0.0:6767` | Address to listen on.                           |
| `--proxy <host:port>` |                | Forward requests to [your app](/docs/backend/). |

### `sonion new <name>`

Creates a [React and Tailwind project](/docs/react/).

### `sonion dev`

Runs a local dev server with live rebuilds. It takes the same options as `serve`, except `--root` defaults to `.` and `--addr` defaults to `127.0.0.1:6767`.

### `sonion fetch <url>`

Fetches a `sonion://` URL and prints the response.

|       Option       |                                        |
| ------------------ | -------------------------------------- |
| `--head`           | Print the status and headers only.     |
| `--registry <url>` | Registry used to look up domain names. |
| `--pin-key <key>`  | Registry signing key to require.       |
| `--insecure-dev`   | Turn off certificate checks.           |

Hostnames are looked up in the registry as sonion domains. To reach a server directly, use its IP address.

## Accounts

### `sonion register`

Creates an account, takes `--username` and `--password`.

### `sonion login`

Prints a new API key, takes `--username` and `--password`.

### `sonion logout`

Revokes an API key, takes `--api-key`.

## Domains

### `sonion put-domain <name>`

Registers/updates a domain and renews it for a year.

|          Option           |                                                                     |
| ------------------------- | ------------------------------------------------------------------- |
| `--api-key <key>`         | Required.                                                           |
| `--endpoints <host:port>` | Required. Separate several with commas.                             |
| `--cert <file>`           | Pin this certificate. If left out, any existing pin is kept.        |
| `--ttl <seconds>`         | Cache time, from 1 to 3600. If left out, the current value is kept. |

### `sonion domains`

Lists your domains and when they expire, takes `--api-key`.

### `sonion delete-domain <name>`

Deletes a domain, takes `--api-key`.

### `sonion resolve <name>`

Prints a domain's signed record, `--pin-key` is optional.

## Running a registry

### `sonion registry`

Runs [your own registry](/docs/own-registry/).

|       Option       |      Default      |                                                   |
| ------------------ | ----------------- | ------------------------------------------------- |
| `--data-dir <dir>` | `./registry-data` | Where accounts, domains and the signing key live. |
| `--cert <file>`    | required          | Certificate PEM.                                  |
| `--key <file>`     | required          | Private key PEM.                                  |
| `--addr <ip:port>` | `127.0.0.1:6767`  | Address to listen on.                             |

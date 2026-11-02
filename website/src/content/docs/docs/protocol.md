---
title: Protocol
description: The SONION/1.0 wire format.
---

`SONION/1.0` looks a lot like HTTP/1.1 carried over TLS.

- **URL:** `sonion://host[:port]/path?query`
- **Port:** 6767 by default
- **TLS:** 1.3 only
- **ALPN:** `sonion/1`

## Request

```
GET /index.html SONION/1.0
Host: cool.rizz

```

Lines end with CRLF, and a blank line ends the headers. Methods are `GET`, `HEAD`, `POST`, `PUT`, `PATCH`, `DELETE` and `OPTIONS`.

## Response

```
SONION/1.0 200 ok
Content-Type: text/html
Content-Length: 18

<h1>hello</h1>
```

Status codes mean the same as in HTTP and clients follow up to 5 redirects.

## Connections

Connections stay open for more requests unless either side sends a `Connection: close`. A server closes a connection after 120 idle seconds or 1000 requests.

## Limits

|              |          |
| ------------ | -------- |
| Request line | 8 KB     |
| Status line  | 64 bytes |
| Headers      | 16 KB    |
| Body         | 10 MB    |

## Try it

```sh
sonion fetch sonion://domains.son/ --head
```

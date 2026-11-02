---
title: Run a backend
description: Put the server in front of your own app.
---

For a forum, an API or anything else with server code, run your app on localhost and let `sonion serve` forward requests to it.

```sh
sonion serve --root ./my-site --cert certs/cert.pem --key certs/key.pem --proxy 127.0.0.1:3000
```

Visitors connect over `sonion://`, and your app receives plain HTTP/1.1, so any normal web framework works. `--root` is still required, but with `--proxy` every request goes to your app.

## What your app receives

- All request methods and most headers, passed through as sent.
- `X-Forwarded-For`, with the visitor's IP address.
- `X-Forwarded-Proto: sonion`.
- `Connection: close`, because each request uses a new connection.

## Limits

- Your app has 5 seconds to accept the connection and 60 seconds to finish responding. After that, the visitor gets a `502` or `504`.
- Request and response bodies are limited to 10 MB.
- Responses are buffered, so streaming and WebSockets don't work.

Keep your app bound to `127.0.0.1` so nobody can reach it directly and fake the forwarded headers.

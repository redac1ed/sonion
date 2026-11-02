---
title: Build a React site
description: Start a React and Tailwind project with live rebuilds.
---

The CLI can create a starter project using React and Tailwind:

```sh
sonion new my-app
sonion cert --out certs
sonion dev --root my-app --cert certs/cert.pem --key certs/key.pem
```

`sonion dev` serves your project on `127.0.0.1:6767`. With [Bun](https://bun.sh) installed, it also:

- runs `bun install` if `node_modules` is missing
- rebuilds `src/main.jsx` into `dist/` when files change
- rebuilds Tailwind from `src/input.css` into `dist/style.css`

Without Bun, `.jsx` and `.tsx` files are compiled per request, and npm packages like React can not be imported, so install Bun for this template.

The certificate is self-signed, so tick **Allow self-signed certs** in the [browser settings](/docs/browser/) to open `sonion://localhost`. Turn it back off when you are done.

## Going live

`sonion dev` is for your own machine. To publish, copy your project to your server and [serve it](/docs/host-a-site/) with `sonion serve`, including the built `dist/` folder.

```sh
sonion serve --root my-app --cert certs/cert.pem --key certs/key.pem
```

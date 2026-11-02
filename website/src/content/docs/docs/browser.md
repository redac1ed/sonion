---
title: Browser settings
description: Configure the browser.
---

Change these in the browser's settings panel, or edit the config file and restart the browser. The CLI reads the same file.

- **Windows:** `%APPDATA%\sonion\config.json`
- **Linux and macOS:** `~/.config/sonion/config.json`

```json
{
  "registry": "sonion://registry.sonion.redac.me:6767",
  "pin_key": null,
  "insecure_dev": false,
  "homepage": "sonion://domains.son/#/sites"
}
```

## Options

- **`registry`:** the registry used to look up domains.
- **`pin_key`:** the registry's public signing key. Leave it as `null` for the default registry, which already has its key built in. Set it if you use [your own registry](/docs/own-registry/).
- **`insecure_dev`:** turns off certificate checks. Only use this for local testing, because it lets anyone impersonate a site.
- **`homepage`:** where the Home button goes. The browser still starts on the New Tab page.

## Environment variables

These override the file:

- `SONION_REGISTRY`: the registry address. This also applies to the CLI.
- `SONION_REGISTRY_KEY`: the registry's signing key.
- `SONION_INSECURE_DEV`: set to anything to turn on insecure dev.

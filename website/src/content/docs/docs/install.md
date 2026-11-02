---
title: Install
description: Install the Browser and CLI.
---

## Browser

Download your build from the [downloads page](/), unzip it and open the app.

- **Windows:** You may see a warning along the lines of "malware detected", because the app isn't signed by a paid certificate. For this, choose **More info** and select **Run anyway**.

- **macOS:** Builds are experimental, pick `aarch64` for Apple Silicon or `x86_64` for Intel. If macOS blocks the app, open **System Settings**, then **Privacy & Security** and select **Open Anyway**.

- **Linux:** Builds are experimental and need WebKitGTK:

```sh
sudo apt install libwebkit2gtk-4.1-0
```

## CLI

You only need the CLI to either host your sites or manage your domains from a terminal. To install, follow the steps:

**Linux and macOS:**

```sh
curl -fsSL https://raw.githubusercontent.com/redac1ed/sonion/main/install.sh | sh
```

Note: This installs to `/usr/local/bin`. Set `SONION_INSTALL_DIR` to choose another folder, or `SONION_VERSION` to install a specific release such as `v0.3.0`.

**Windows (PowerShell):**

```powershell
irm https://raw.githubusercontent.com/redac1ed/sonion/main/install.ps1 | iex
```

Note: This installs to `%LOCALAPPDATA%\sonion\bin` and adds it to your PATH. Restart your terminal afterwards.

To check that it worked:

```sh
sonion --version
```

## Update

Run the same install command, or download the browser again. Alongside, replace the old browser app with the new one. Your settings and bookmarks are kept even after an update.

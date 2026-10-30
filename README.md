![image](https://cdn.hackclub.com/01a0b9e8-7d95-768f-a588-8b9ffa4ef518/journal-1789825415608.png)

# sonion

A recreation of the World Wide Web (WWW), with its own protocol (`sonion://`), domains (TLDs like `.son`, `.rizz` etc.), server, registry and browser to tie it all together. 

Heavily inspired by [gurted.com](https://gurted.com).

## Features

- **a protocol**, with HTTP-ish requests over TLS 1.3 on port 6767.
- **a server**, which serves a folder or sits in front of your own app with `--proxy` (for apps like forums, APIs etc.)
- **a registry**, just grab a domain and point it at your server.
- **a browser**, with tabs, history, bookmarks, downloads, devtools and the normal stuff.
- **a cli**, with `sonion` command for all of the above.

## How To Install

### `sonion` CLI: 

To install the CLI on your computer, run: 

1. For Linux/MacOS:
```sh
curl -fsSL https://raw.githubusercontent.com/redac1ed/sonion/main/install.sh | sh
```

2. For Windows: 
```powershell
irm https://raw.githubusercontent.com/redac1ed/sonion/main/install.ps1 | iex
```

### Sonion Browser

Go over to the [latest release page](https://github.com/redac1ed/sonion/releases/latest) and select the build according to your OS: 

- **Windows**: `sonion-browser-x86_64-pc-windows-msvc.zip`.
- **MacOS**:  `sonion-browser-aarch64-apple-darwin.zip` (Apple Silicon) or `x86_64` (Intel).
- **Linux**: `sonion-browser-x86_64-unknown-linux-gnu.tar.gz`. 

Just unzip the download and open the respective build.

Note: The Linux build needs WebKitGTK, install by `sudo apt install libwebkit2gtk-4.1-0`. 

## Host A Site

On your server, run: 

```sh
sonion cert --host <your ip> --out ./certs
sonion serve --root ./my-site --cert certs/cert.pem --key certs/key.pem

sonion register --username you --password "something long"
sonion login --username you --password "something long"
sonion put-domain yourdomain.son --api-key <key> --endpoints <your ip>:6767
```

After that, navigate to `sonion://yourdomain.son`.

Note: for a backend (e.g. an API, forum), run it on localhost and add `--proxy 127.0.0.1:3000` to `serve`.

## Documentation

For more information about how to run your site, check out the [docs](https://sonion.redac.me/docs).

## License

MIT License.


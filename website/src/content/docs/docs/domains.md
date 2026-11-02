---
title: Domains
description: Register, renew and manage sonion domains.
---

Domains are free, and you can manage them at `sonion://domains.son` in the browser or with the [CLI](/docs/cli/).

## `domains.son` site

- **Search:** Check whether a name is free and register it.
- **Sites:** A public list of registered sites. We do not have search engine yet, so this is how people find other sites.
- **Whois:** See who owns a domain and where it points.
- **My domains:** Edit, renew or delete your domains.

## Name rules

- Names can use lowercase letters, numbers and dashes, and can't start or end with a dash.
- Each part can be up to 63 characters.
- The ending must be one of the registry's TLDs, such as `.son` or `.rizz`. The `domains.son` search box lists them all, or you can run:

```sh
sonion fetch sonion://domains.son/v1/tlds
```

## Renewing

A domain lasts one year. Saving any change renews it for another year, and so does the **Renew** button. Once it expires, anyone can register the name.

Check your expiry dates with:

```sh
sonion domains --api-key <key>
```

## What's public

Whois shows anyone the domain's owner username, server addresses, certificate fingerprint, description and dates.

Your domain appears on the Sites page by default. To hide it, edit it on `domains.son` and untick **Show on the Sites page**. It still works and still shows up in whois.

## Limits

- Up to 8 server addresses per domain. Browsers try them in order.
- Descriptions can be up to 140 characters.
- The cache time (TTL) can be 1 to 3600 seconds, and defaults to 300. After you change your server's address, some visitors may reach the old one until this runs out.

## Accounts

Usernames follow the same rules as names. Passwords must be 8 to 256 characters.

There is **no password reset yet**, so keep yours somewhere safe.

import {defineConfig} from "astro/config";
import starlight from "@astrojs/starlight";

export default defineConfig({
    site: "https://sonion.redac.me",
    integrations: [
        starlight({
            title: "sonion",
            favicon: "/favicon.svg",
            customCss: ["./src/styles/docs.css"],
            social: [
                {icon: "github", label: "GitHub", href: "https://github.com/redac1ed/sonion"}
            ],
            sidebar: [
                {
                    label: "Start here",
                    items: [
                        {label: "Introduction", link: "/docs/"},
                        {label: "Install", link: "/docs/install/"},
                        {label: "Host a site", link: "/docs/host-a-site/"}
                    ]
                },
                {
                    label: "Guides",
                    items: [
                        {label: "Domains", link: "/docs/domains/"},
                        {label: "Certificates", link: "/docs/certificates/"},
                        {label: "Run a backend", link: "/docs/backend/"},
                        {label: "Build a React site", link: "/docs/react/"},
                        {label: "Browser settings", link: "/docs/browser/"},
                        {label: "Run your own registry", link: "/docs/own-registry/"}
                    ]
                },
                {
                    label: "Reference",
                    items: [
                        {label: "CLI", link: "/docs/cli/"},
                        {label: "Registry API", link: "/docs/registry-api/"},
                        {label: "Protocol", link: "/docs/protocol/"}
                    ]
                }
            ]
        })
    ]
});
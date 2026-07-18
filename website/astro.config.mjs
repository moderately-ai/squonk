// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

import sitemap from "@astrojs/sitemap";
import starlight from "@astrojs/starlight";
import { defineConfig } from "astro/config";

// `site` is load-bearing for SEO: it sets the canonical origin Starlight stamps
// on every page's <link rel="canonical"> and Open Graph URL, and it is what
// @astrojs/sitemap uses to emit absolute URLs in sitemap-index.xml. Keep it in
// sync with the deployed custom domain (public/CNAME).
export default defineConfig({
  site: "https://squonk.dev",
  integrations: [
    starlight({
      title: "squonk",
      description:
        "A fast, typed, multi-dialect SQL parser for Rust, Python, and TypeScript — parse SQL to an AST and transpile between dialects.",
      logo: { src: "./src/assets/logo.svg", alt: "squonk" },
      favicon: "/favicon.svg",
      social: [
        { icon: "github", label: "GitHub", href: "https://github.com/moderately-ai/squonk" },
      ],
      editLink: {
        baseUrl: "https://github.com/moderately-ai/squonk/edit/main/website/",
      },
      sidebar: [
        { label: "Start here", items: [{ slug: "getting-started" }] },
        {
          label: "Guides",
          items: [
            { slug: "guides/parsing" },
            { slug: "guides/dialects" },
            { slug: "guides/transpiling" },
          ],
        },
        {
          label: "Reference",
          items: [
            { label: "Rust API (docs.rs)", link: "https://docs.rs/squonk", attrs: { target: "_blank" } },
            { label: "Architecture", link: "https://github.com/moderately-ai/squonk/blob/main/docs/architecture.md", attrs: { target: "_blank" } },
          ],
        },
      ],
    }),
    sitemap(),
  ],
});

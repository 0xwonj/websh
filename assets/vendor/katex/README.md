# KaTeX

Vendored from the [KaTeX 0.16.45 release](https://github.com/KaTeX/KaTeX/releases/tag/v0.16.45).
The JavaScript, font data, and MIT license are upstream files.

This app targets modern WebAssembly browsers and ships only WOFF2 fonts. The CSS
diff from upstream removes the WOFF and TTF alternatives from each `@font-face`
source list; all 20 WOFF2 faces remain. KaTeX documents this supported packaging
choice in its [font configuration](https://katex.org/docs/font).

When updating, take JavaScript, CSS, fonts, and `LICENSE` from the same release.
Keep only WOFF2 source entries and files (equivalent to an upstream build with
`USE_WOFF2=true USE_WOFF=false USE_TTF=false`), then check math rendering in the
reader. Do not remove font families based on the current articles: future
expressions may use any of them.

---
title: "content-backed homepage"
date: "2026-04-20"
tags: [notes, websh, homepage, renderer-test]
---

# Renderer test: content-backed homepage

This is intentional test content for the content-backed renderer path. It should be easy to recognize as non-final writing.

## Markdown coverage

The renderer should handle links, emphasis, code, lists, tables, and math in the same page:

- Route-backed content: `/writing/content-backed-homepage`
- Inline code: `content/writing/content-backed-homepage.md`
- Inline math: $e^{i\pi} + 1 = 0$

| Feature | Expected result |
| --- | --- |
| Markdown | rendered as article content |
| KaTeX inline | $a^2 + b^2 = c^2$ |
| KaTeX display | centered display equation |

## KaTeX display block

$$
\int_0^1 x^2\,dx = \frac{1}{3}
$$

$$
\begin{bmatrix}
1 & 2 \\
3 & 4
\end{bmatrix}
\begin{bmatrix}
x \\
y
\end{bmatrix}
=\begin{bmatrix}
x + 2y \\
3x + 4y
\end{bmatrix}
$$

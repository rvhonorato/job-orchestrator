# Slides

A slide deck showcasing `job-orchestrator` — motivation, architecture, tech stack, scheduling design, job lifecycle, and production deployment.

Built with [Typst](https://typst.app/) (no external packages required).

## Dependencies

```
sudo pacman -S typst zathura zathura-pdf-mupdf noto-fonts
```

## Build

```
typst compile slides.typ slides.pdf
```

## Watch mode (hot reload)

```
zathura slides.pdf &
typst watch slides.typ slides.pdf
```

Zathura auto-refreshes the PDF on each recompile.

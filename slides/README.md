# Slides

A slide deck showcasing `job-orchestrator` — motivation, architecture, tech stack, scheduling design, job lifecycle, and production deployment.

Built with [Typst](https://typst.app/) (no external packages required).

## Dependencies

```
sudo pacman -S typst zathura zathura-pdf-mupdf noto-fonts
```

## Build

```bash
# Both
make

# Light only
make light
# or: typst compile slides.typ slides-light.pdf --input theme=light

# Dark only
make dark
# or: typst compile slides.typ slides-dark.pdf --input theme=dark
```

## Watch mode (hot reload)

```bash
# Light
zathura slides-light.pdf &
typst watch slides.typ slides-light.pdf --input theme=light

# Dark
zathura slides-dark.pdf &
typst watch slides.typ slides-dark.pdf --input theme=dark
```

Zathura auto-refreshes the PDF on each recompile.

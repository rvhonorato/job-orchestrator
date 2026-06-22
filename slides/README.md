# Slides

## Dependencies

```
sudo pacman -S typst zathura zathura-pdf-mupdf noto-fonts
```

The `oxdraw` Typst package is fetched automatically on first compile (requires internet access once, then cached locally).

## Build

```
typst compile slides.typ slides.pdf
```

## Watch mode

```
zathura slides.pdf &
typst watch slides.typ slides.pdf
```

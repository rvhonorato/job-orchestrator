.PHONY: all dark light clean

all: light dark

dark: slides-dark.pdf
slides-dark.pdf: slides.typ
	typst compile slides.typ slides-dark.pdf --input theme=dark

light: slides-light.pdf
slides-light.pdf: slides.typ
	typst compile slides.typ slides-light.pdf --input theme=light

clean:
	rm -f slides-light.pdf slides-dark.pdf

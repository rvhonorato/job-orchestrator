.PHONY: all clean

all: slides.pdf

slides.pdf: slides.typ
	typst compile slides.typ slides.pdf

clean:
	rm -f slides.pdf

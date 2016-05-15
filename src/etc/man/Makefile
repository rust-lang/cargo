MAN1 =
MAN1 += cargo.md
MAN1 += cargo-build.md

all: $(patsubst %.md, %.1, $(MAN1))

%.1: %.md
	pandoc -s -t man $< -o $@

clean:
	rm *.1

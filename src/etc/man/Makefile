MAN1 =
MAN1 += cargo.adoc
MAN1 += cargo-build.adoc

all: $(patsubst %.adoc, %.1, $(MAN1))

%.1: %.adoc
	a2x --doctype manpage --format manpage $<

clean:
	rm *.1

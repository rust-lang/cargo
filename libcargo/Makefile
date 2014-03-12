LIBCARGO_LIB := $(shell rustc --crate-file-name cargo.rs)

default: target/$(LIBCARGO_LIB)

target:
	mkdir -p target

clean:
	rm -rf target

target/$(LIBCARGO_LIB): target cargo.rs
	rustc cargo.rs --out-dir target

.PHONY: default clean

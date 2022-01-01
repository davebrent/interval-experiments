.PHONY: bench clean test

all: 
	cargo test --no-run
	cargo bench --no-run

tests/256/:
	unzip tests/256.zip -d tests/256/

bench: tests/256/
	cargo bench -- --significance-level=0.1 --noise-threshold=0.05

test: tests/256/
	cargo test --release -- --nocapture --include-ignored

clean:
	rm -rf tests/256/
	rm -rf target/criterion/

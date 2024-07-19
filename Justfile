[private]
default:
	@just --list

# run a full test harness
test *args:
	cargo nextest run {{ args }}
	cargo test --doc

# compile the documentation
docs:
	typst compile docs/test-set-dsl.typ docs/test-set-dsl.pdf

# install typst-test using cargo
install:
	cargo install --path crates/typst-test-cli

[private]
default:
	@just --list

# run a full test harness
test *args:
	cargo nextest run {{ args }}
	cargo test --doc

# install typst-test using cargo
install:
	cargo install --path crates/typst-test

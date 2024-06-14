[private]
default:
	@just --list

# run a full test harness
test:
	cargo nextest run
	cargo test --doc

# install typst-test using cargo
install:
	cargo install --path crates/typst-test

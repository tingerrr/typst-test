[private]
default:
	@just --list

# run typst-test with the typst test-scripts
test root='.':
	cargo run -- run --root {{ root }}

# install typst-test using cargo
install:
	cargo install --path crates/typst-test

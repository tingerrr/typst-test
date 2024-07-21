[private]
default:
	@just --list

# run a full test harness
test *args:
	cargo nextest run {{ args }}
	# TODO: re-enable this once we know why this make my CPU skyrocket to 100%
	# usage
	# cargo test --doc

# compile the documentation
docs:
	cargo doc
	typst compile docs/test-set-dsl.typ docs/test-set-dsl.pdf

# install typst-test using cargo
install:
	cargo install --path crates/typst-test-cli

book-src := justfile_directory() / 'docs' / 'book'

[private]
default:
	@just --unsorted --list --list-submodules

# documentation
mod doc 'just/doc.just'

# testing
mod test 'just/test.just'

# checks and lints
mod check 'just/check.just'

# compile and run typst-test
run *args='--release':
	cargo run {{ args }}

# run tests and checks similar to CI
ci $RUSTFLAGS='-Dwarnings' $RUSTDOCFLAGS='-Dwarnings':
	just check clippy
	just check format
	cargo doc --workspace --no-deps

# clean all temporary directories and build artifacts
clean:
	rm -r target
	rm -r {{ book-src / 'book' }}

# install typst-test using cargo
install:
	cargo install --path crates/typst-test-cli

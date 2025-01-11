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
	# FIXME(tinger): See https://github.com/rust-lang/rust/issues/128538 if you get
	# high CPU doc tests
	cargo +1.80 test --workspace
	cargo +1.80 clippy --workspace
	cargo +1.80 fmt --all --check
	cargo +1.80 doc --workspace --no-deps

# clean all temporary directories and build artifacts
clean:
	rm -r target
	rm -r {{ book-src / 'build' }}

# install typst-test using cargo
install *args='--force':
	cargo install --path crates/typst-test-cli {{ args }}

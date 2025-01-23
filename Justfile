book-src := justfile_directory() / 'docs' / 'book'

# the cargo variable is used to run `cargo` in the nix dev shell, but
# `cargo +1.80` outside of it
CARGO_1_80 := env('CARGO_1_80', 'cargo +1.80')

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
	{{ CARGO_1_80 }} test --workspace
	{{ CARGO_1_80 }} clippy --workspace
	{{ CARGO_1_80 }} fmt --all --check
	{{ CARGO_1_80 }} doc --workspace --no-deps

# clean all temporary directories and build artifacts
clean:
	rm -r target
	rm -r {{ book-src / 'build' }}

# install typst-test using cargo
install *args='--force':
	cargo install --path crates/typst-test-cli {{ args }}

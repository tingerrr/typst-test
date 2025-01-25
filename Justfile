book-src := justfile_directory() / 'docs' / 'book'

# the cargo variable is used to run `cargo` in the nix dev shell, but
# `cargo +1.80` outside of it
CARGO-1-80 := env('CARGO_1_80', 'cargo +1.80')
CI-SET-ENV := 'export RUSTFLAGS="-Dwarnings" RUSTDOCFLAGS="-Dwarnings"'

[private]
default:
	@just --unsorted --list --list-submodules

# compile and run typst-test
run *args='--release':
	cargo run {{ args }}

# run lints project wide
check:
	# FIXME(tinger): mdbook-linkcheck is disabled, because some links are
	# deliberately pointing to the generated html files, see:
	# https://github.com/rust-lang/mdBook/issues/984
	# mdbook-linkcheck --standalone {{ book-src }}
	mdbook test {{ book-src }}
	cargo fmt --all --check
	cargo clippy --workspace --all-targets --all-features

# run tests project wide
test:
	# FIXME(tinger): see
	# https://github.com/nextest-rs/nextest/issues/16
	cargo test --workspace --doc
	cargo nextest run --workspace

# build and serve the book locally
book *args='--open':
	mdbook serve {{ book-src }} {{ args }}

# run tests and checks similar to CI
ci:
	{{ CI-SET-ENV }} && {{ CARGO-1-80 }} test --workspace
	{{ CI-SET-ENV }} && {{ CARGO-1-80 }} clippy --workspace
	{{ CI-SET-ENV }} && {{ CARGO-1-80 }} fmt --all --check
	{{ CI-SET-ENV }} && {{ CARGO-1-80 }} doc --workspace --no-deps
	@echo ""
	@echo These checks are not exactly the same as CI, but should get you there most of the way.
	@echo ""

# clean all temporary directories and build artifacts
clean:
	rm -r target
	rm -r {{ book-src / 'build' }}

# install typst-test using cargo
install *args='--force':
	cargo install --path crates/typst-test-cli {{ args }}

book-src := justfile_directory() / 'docs' / 'book'

# run all (doesn't include typos)
all: clippy format book

# check the whole project for typos
typos *args:
	typos {{ args }}

# run various book checks and linters
book: book-test book-linkcheck

# check book code examples
book-test:
	mdbook test {{ book-src }}

# check book links (disabled)
book-linkcheck:
	# TODO: this is currently disabled as the book is in progress and contains dead
	# links
	# mdbook-linkcheck --standalone {{ book-src }}

# run clippy lints
clippy *args='--all-targets --all-features':
	cargo clippy --workspace {{ args }}

# check rust formatting
format *args:
	cargo fmt --all --check {{ args }}

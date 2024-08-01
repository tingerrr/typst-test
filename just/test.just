book-src := justfile_directory() / 'docs' / 'book'

# run a full test harness (doc tests disabled)
all: && book doc
	cargo nextest run --workspace

# run the lib tests
lib *args:
	cargo nextest run --package typst-test-lib {{ args }}

# run the cli tests
cli *args:
	cargo nextest run --package typst-test-cli {{ args }}

# run the doc tests
doc *args:
	# TODO: this currently deadlocks my pc at 100% CPU
	# cargo test --workspace --doc {{ args }}

# run cargo insta and review the snapshot changes
review *args='--review':
	cargo insta test --test-runner nextest {{ args }}

# test the book
book *args:
	mdbook test {{ book-src }} {{ args }}

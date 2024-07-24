book-src := justfile_directory() / 'docs' / 'book'

# build all documentation
all: build

# build all documentation
build: book-build rust-build

# build and open the book
book *args: (book-serve '--open' args)

# build the book
book-build *args:
	mdbook build {{ book-src }} {{ args }}

# serve the book
book-serve *args='--open':
	mdbook serve {{ book-src }} {{ args }}

# compile and open the rust documentation
rust *args: (rust-build '--open' args)

# compile the rust documentation
rust-build *args:
	cargo doc --workspace {{ args }}

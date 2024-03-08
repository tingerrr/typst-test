[private]
default:
	@just --list

# run typst-test with the typst test-scripts
test root=justfile_directory():
	cargo run -- run --root {{ root }}

# install typst-test using cargo
install:
	cargo install --path crates/typst-test

# update and force push the ci-semi-stable tag
[confirm("this will update the ci-semi-stable tag [y/n]:")]
ci-tag:
	git rev-parse ci-semi-stable
	git tag -d ci-semi-stable
	git tag ci-semi-stable
	git push --tags --force

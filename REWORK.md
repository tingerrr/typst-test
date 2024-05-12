# Rework Tracking Document
This is a document tracking the rework progress as well as a general explanation of what this rework attempts to achieve.

# Why
- make typst-test accessible to others (lsp and editor extension authors)
- make typst-test faster and smarter
- make typst-test independent from the installed typst version
- make typst-test parse multiple test from one file

# Features
The following features are planned and should be made possible by the rework:
- [ ] in memory comparison of reference and test output
  - [x] png
  - [ ] svg
  - [ ] pdf
  - [ ] document structure (actual feasibility unclear)
- [ ] version tests, test regressions across different typst and package versions
  - [ ] different typst versions (references on the fly without reference script)
  - [ ] different package versions (requires source control support)
- [ ] persistent tests, load references from disk
- [x] ephemeral tests, create references on the fly
- [ ] compilation tests, don't compare anything
  - [x] expected pass
  - [ ] expected failure
- [ ] external reference retrieval (similar to git lfs)
- [ ] custom user actions at various stages
- [ ] smarter image diffing for visual tests
- [ ] allow system installed typst version to be used
- [ ] ship some default implementations for the last N versions for typst as opt in features
- [ ] easy test configuration

Some of these features may be cli only.

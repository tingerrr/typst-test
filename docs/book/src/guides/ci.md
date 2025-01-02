# Setting Up CI
Continuous integration can take a lot of manual work off your shoulders.
In this chapter we'll look at how to run `typst-test` in your GitHub CI to continuously test your code and catch bugs before they get merged into main.

We start off by creating a `.github/workflows` directory in our project and place a single `ci.yaml` file in this directory.
The name is not important, but should be something that helps you distinguish which workflow you're looking at.

<div class="warning">

If you simply want get CI working without any elaborate explanation, skip ahead to the bottom and copy the full file.

There's a good chance that you can simply copy and paste the workflow as is and it'll work, but the guide should give you an idea on how to adjust it to your liking.

</div>

First, we configure when CI should be running:
```yml
name: CI
on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]
```

The `on.push` and `on.pull_request` fields both take a `branches` fields with a single pattern matching our main branch, this means that this workflow is run on pull requests and pushes to main.
We could leave out the `branches` field and it would apply to all pushes or pull requests, but this is seldom useful.
If you have branch protection, you may not need the `on.push` trigger at all, if you're paying for CI this may save you money.

Next, let's add the test job we want to run, we'll let it run on `ubuntu-latest`, that's a fairly common runner for regular CI.
More often than not, you won't need matrix or cross platform tests for Typst projects as Typst takes care of the OS differences for you.
Add this below the job triggers:

```yml
# ...

jobs:
  tests:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v3
```

This adds a single step to our job (called `tests`), which checks out the repository, making it available for the following steps.

For now, we'll need `cargo` to download and install `typst-test`, so we install it and cache the installation with a package cache action.
And then we install `typst-test` straight from GitHub using the `backport` branch, this branch does not yet have features like test set expressions, but it is somewhat stable and receives fixes.

```yml
steps:
  # ...
  - name: Probe runner package cache
    uses: awalsh128/cache-apt-pkgs-action@latest
    with:
      packages: cargo
      version: 1.0

  - name: Install typst-test from github
    uses: baptiste0928/cargo-install@v3.0.0
    with:
      crate: typst-test
      git: https://github.com/tingerrr/typst-test.git
      branch: backport

```

Because the `typst-test` version at `backport` does not yet come with it's own Typst compiler, it needs a Typst installation in the runner. Add the following with your preferred Typst version:

```yml
steps:
  # ...
  - name: Setup typst
    uses: yusancky/setup-typst@v2
    with:
      version: 'v0.11.1'
```

Then we're ready to run our tests, that's as simple as adding a step like so:

```yml
steps:
  # ...
  - name: Run test suite
    run: typst-test run
```

CI may fail for various reasons, such as
- missing fonts
- system time dependent test cases
- or otherwise hard-to-debug differences between the CI runner and your local machine.

To make it easier for us to actually get a grasp at the problem we should make the results of the test run available.
We can do this by using an upload action, however, if `typst-test` fails the step will cancel all regular steps after itself, so we need to ensure it runs regardless of test failure or success by using `if: always()`.
We upload all artifacts since some tests may produce both references and output on-the-fly and retain them for 5 days:

```yml
steps:
  # ...
  - name: Archive artifacts
    uses: actions/upload-artifact@v4
    if: always()
    with:
      name: artifacts
      path: |
        tests/**/diff/*.png
        tests/**/out/*.png
        tests/**/ref/*.png
      retention-days: 5
```

And that's it, you can add this file to your repo, push it to a branch and open a PR, the PR will already start running the workflow for you and you can adjust and debug it as needed.

> The full workflow file:
>
> ```yml
> name: CI
> on:
>   push:
>     branches: [ main ]
>   pull_request:
>     branches: [ main ]
>
> jobs:
>   tests:
>     runs-on: ubuntu-latest
>     steps:
>       - name: Checkout
>         uses: actions/checkout@v3
>
>       - name: Probe runner package cache
>         uses: awalsh128/cache-apt-pkgs-action@latest
>         with:
>           packages: cargo
>           version: 1.0
>
>       - name: Install typst-test from github
>         uses: baptiste0928/cargo-install@v3.0.0
>         with:
>           crate: typst-test
>           git: https://github.com/tingerrr/typst-test.git
>           branch: backport
>
>       - name: Setup typst
>         uses: yusancky/setup-typst@v2
>         with:
>           version: 'v0.11.1'
>
>       - name: Run test suite
>         run: typst-test run
>
>       - name: Archive artifacts
>         uses: actions/upload-artifact@v4
>         if: always()
>         with:
>           name: artifacts
>           path: |
>             tests/**/diff/*.png
>             tests/**/out/*.png
>             tests/**/ref/*.png
>           retention-days: 5
> ```

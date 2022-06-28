fetch-hash
==========

[<img alt="github" src="https://img.shields.io/badge/github-fetch--hash-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/CarlKCarlK/fetch-hash)
[<img alt="crates.io" src="https://img.shields.io/crates/v/fetch-hash.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/fetch-hash)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-fetch--hash-66c2a5?style=for-the-badge&labelColor=555555&logoColor=white&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K" height="20">](https://docs.rs/bed-reader)
[<img alt="build status" src="https://img.shields.io/github/workflow/status/fastlmm/bed-reader/CI/master?style=for-the-badge" height="20">](https://github.com/CarlKCarlk/fetch-hash)

This crate helps you retrieve data file(s) from the Internet.
Files are only downloaded when needed. The contents of the files
are always verified via a SHA256 hash.

The ability to download data files makes creating sample code easier.
For example, here we download a genomics file and print its size:

```rust
use fetch_hash::sample_file;

let path = sample_file("small.fam")?;
println!("{}", std::fs::metadata(path)?.len()); // Prints 85

# use fetch_hash::FetchHashError; // '#' needed for doctest
# Ok::<(), FetchHashError>(())
```
Features
--------

* Thread-safe -- allowing it to be used with Rust's multithreaded testing framework.
* Inspired by Python's popular [Pooch](https://pypi.org/project/pooch/) and our PySnptools [filecache module](https://fastlmm.github.io/PySnpTools/#module-pysnptools.util.filecache).
* Avoids run-times by using [`ureq`](https://crates.io/crates/ureq) to download files via blocking I/O.

Suggested Usage (with Example Code)
-----

* Create a `registry.txt` file containing a whitespace-delimited list of files
  and their hashes. (This is the same format as [Pooch](https://pypi.org/project/pooch/)). You can put this file anywhere in your project. I put
  it in `tests/registry.txt` and I put data files in `tests/data`.
  See [Registry Creation](#registry-creation) for more information.

* In your code, create a global static `FetchHash` instance that
  reads your `registry.txt` file. You should also give it:
  -  the URL root from which to download the files
  -  an environment variable that can set the local data directory
     where files are stored.
   - a qualifier, organization, and application used to create a local data 
     directory when the environment variable is not set.

 * Define a public `sample_file` function that takes a file name and returns a `Result`
   containing the path to the file.

```rust
use fetch_hash::{ctor, FetchHash, FetchHashError};
use std::path::{Path, PathBuf};

#[ctor]
static STATIC_FETCH_HASH: FetchHash = FetchHash::new(
    include_str!("../tests/registry.txt"),
    "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/",
    "BAR_APP_DATA_DIR",
    "com",
    "Foo Corp",
    "Bar App",
);

pub fn sample_file<P: AsRef<Path>>(path: P) -> Result<PathBuf, FetchHashError> {
    STATIC_FETCH_HASH.fetch_file(path)
}

# Ok::<(), FetchHashError>(())
```

You and your users can now use your `sample_file` function to download your files as needed.

<a name="registry-creation"></a>Registry Creation (with Example Code)
------------------------

Here is one suggested method for creating a `registry.txt` file.

* Upload your data files to the Internet. For example, I put my data files
  in my project under `tests/data`, so they uploaded to [this GitHub folder](https://github.com/CarlKCarlK/fetch-hash/tree/main/tests/data). In GitHub, by looking at the [raw view of a data file](https://github.com/CarlKCarlK/fetch-hash/blob/main/tests/data/small.fam), I see the root URL for these files.
* Write code that
  - Creates a `FetchHash` instance without registry contents
  - Lists the files in your data directory.
  - Calls `gen_registry_contents` on your list of files. It will download
    the files, compute their hashes, and create a string of file names and hashes.
* Print this string and put it into a file called `registry.txt`.

```rust
use fetch_hash::{FetchHash, dir_to_file_list};

let fetch_hash = FetchHash::new(
    "",
    "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/",
    "BAR_APP_DATA_DIR",
    "com",
    "Foo Corp",
    "Bar App",
);
let file_list = dir_to_file_list("tests/data")?;
let registry_contents = fetch_hash.gen_registry_contents(file_list)?;
println!("{registry_contents}");
# use fetch_hash::FetchHashError; // '#' needed for doctest
# Ok::<(), FetchHashError>(())
```

Notes
-----

* Don't use our `sample_file`. Define your own `sample_file` that
  knows about your data files.
* You don't have to make your `FetchHash` instance global and static.
* You don't need to use a registry file or `FetchHash` instance. You can instead use utility functions such as `fetch`.
* Does binary downloads, so no line ending changes for Windows.

Project Links
-----

* [**Installation**](https://crates.io/crates/fetch-hash)
* [**Documentation**](https://docs.rs/fetch-hash/)
* [**Source code**](https://github.com/CarlKCarlK/fetch-hash)
* [**Discussion**](https://github.com/CarlKCarlK/fetch-hash/discussions/)
* [**Bug Reports and Feature Requests**](https://github.com/CarlKCarlK/fetch-hash/issues)

A lightweight, static Git web frontend written in Rust

## Installation

Build with Rust's cargo, then install (`PREFIX` defaults to `/usr/local`):

    make
    sudo make install

This installs `wit` to `${PREFIX}/bin`, its man page to `${PREFIX}/man/man1` (`man wit`; with mandoc, `makewhatis ${PREFIX}/man` adds it to `apropos`), and `style.css`, `fonts/`, `post-receive`, `README.md` and `LICENSE` to `${PREFIX}/share/doc/wit`. `make uninstall` removes them. `DESTDIR` and `MANPREFIX` work as usual.

At runtime wit needs libgit2 and git (for `git archive`).

Create dependencies:

- git2
- chrono
- pulldown-cmark
- anyhow
- imagesize
- sha2

## Usage:

// TODO

### Stylesheet

wit doesn't write a stylesheet. Copy `style.css` and the `fonts/` dir into the site dir (the one after `-o`), and copy them again after changing them.

### Rebuilding on every push

Copy `contrib/post-receive` into each bare repo's `hooks/` dir, keep it executable, and set its paths: the site dir after `-o`, then every repo the site lists. git runs the hook inside the pushed repo, so `-u .` rebuilds only that repo, plus any repo that has no pages yet; the index is rewritten from all of them. Run wit without `-u` to rebuild every repo, e.g. after updating wit or changing a description or owner. A rebuilt repo's old pages are deleted first.

### Releases

A release is a tag with files. Put a tag's files in the bare repo's `releases/<tag>/` folder (in a working copy, `.git/releases/<tag>/`), next to `description`; git ignores it. Upload them over SSH, then push the tag, so the hook builds the release:

```sh
rsync --mkpath foo-linux-x86_64 foo-linux-aarch64 server:/srv/git/foo.git/releases/v1.2/
git push origin v1.2
```

`--mkpath` (rsync 3.2.3 or later, needed on both machines) creates the folder; with `scp`, create it first: `ssh server mkdir -p /srv/git/foo.git/releases/v1.2`. Uploading alone doesn't run the hook: if the tag is already pushed, run wit by hand, e.g. `wit -o /var/www/git -u /srv/git/foo.git /srv/git/*.git`.

The repo gets a releases tab and a "latest release" row on its summary. The releases page shows each release's tag message as markdown and its files as download links, with their sizes and sha256 sums, followed by the tag's source code as `.zip` and `.tar.gz`, like GitHub's. wit makes those with `git archive` on every rebuild, so git must be installed where wit runs. Only regular files directly in the tag's folder count; a folder with no files isn't a release, and a folder named after no tag gets a warning.

wit hardlinks the files into the site, at `<repo>/releases/<tag>/`, so keep the repos and the site on one filesystem; otherwise it copies them on every rebuild. A hardlink has the upload's permissions, so the web server must be able to read the uploaded files. Serve the site over HTTPS only.

## Credits

The icons are from [Phosphor Icons](https://github.com/phosphor-icons/core), Copyright (c) 2023 Phosphor Icons, licensed under the [MIT License](https://github.com/phosphor-icons/core/blob/main/LICENSE) (`fonts/phosphor-icons-MIT.txt`). The font is [Recursive](https://github.com/arrowtype/recursive), Copyright 2019 The Recursive Project Authors, licensed under the SIL Open Font License 1.1 (`fonts/OFL.txt`).

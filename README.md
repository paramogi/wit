A lightweight, static Git web frontend written in Rust

## Installation

Build with Rust's cargo, then install (`PREFIX` defaults to `/usr/local`):

```
make
sudo make install
```

This installs `wit` to `${PREFIX}/bin`, its man page to `${PREFIX}/man/man1`, and `style.css`, `fonts/`, `post-receive`, `README.md` and `LICENSE` to `${PREFIX}/share/doc/wit`. `make uninstall` removes them. `DESTDIR` and `MANPREFIX` work as usual.

At runtime wit needs libgit2 and git (for `git archive`).

Create dependencies:

- git2
- chrono
- pulldown-cmark
- anyhow
- imagesize
- sha2

## Usage:

```sh
wit [-O owner] [-u repo] -o outdir repo...
```

wit writes a static site for the listed repos into `outdir`: an index of the repos, and for each one a summary, tree and file pages and a commit log for every branch and tag, a page for every commit, and its branches, tags and releases. A repo is a bare repo or a working copy, named after its folder without `.git`.

- `-o outdir`: the site dir.
- `-O owner`: the owner shown for repos whose `owner` file is missing or empty.
- `-u repo`: rebuild only this repo, plus any listed repo that has no pages yet (see Rebuilding on every push).

Each repo can have a `description` and an `owner` file, in a bare repo's dir or a working copy's `.git/`. Two repos with the same name stop the run; a repo whose HEAD has no commits or is detached is skipped with a warning.

For example, to build a site for every repo in `/srv/git`:

```sh
wit -O alice -o /var/www/git /srv/git/*.git
cp -r /usr/local/share/doc/wit/style.css /usr/local/share/doc/wit/fonts /var/www/git/
```

See `man wit` for the details.

### Stylesheet

wit doesn't write a stylesheet. Copy `style.css` and the `fonts/` dir (installed in `${PREFIX}/share/doc/wit`) into the site dir (the one after `-o`), and copy them again after changing them or updating wit.

### Rebuilding on every push

Copy `contrib/post-receive` into each bare repo's `hooks/` dir, keep it executable, and set its paths: the site dir after `-o`, then every repo the site lists. git runs the hook inside the pushed repo, so `-u .` rebuilds only that repo, plus any repo that has no pages yet; the index is rewritten from all of them. Run wit without `-u` to rebuild every repo, e.g. after updating wit or changing a description or owner. A rebuilt repo's old pages are deleted first.

### Releases

Put a tag's files in the bare repo's `releases/<tag>/` folder (in a working copy, `.git/releases/<tag>/`), next to `description`; git ignores it. Upload them over SSH, then push the tag, so the hook builds the release:

```sh
scp foo-linux-x86_64 user@server:<repo>/foo-linux-aarch64
git push origin v1.2
```

The repo gets a releases tab and a "latest release" row on its summary. The releases page shows each release's tag message as markdown and its files as download links, with their sizes and sha256 sums, followed by the tag's source code as `.zip` and `.tar.gz`.

wit hardlinks the files into the site, at `<repo>/releases/<tag>/`, so keep the repos and the site on one filesystem; otherwise it copies them on every rebuild. A hardlink has the upload's permissions, so the web server must be able to read the uploaded files. Serve the site over HTTPS only.

## Credits

The icons are from [Phosphor Icons](https://github.com/phosphor-icons/core), Copyright (c) 2023 Phosphor Icons, licensed under the [MIT License](https://github.com/phosphor-icons/core/blob/main/LICENSE) (`fonts/phosphor-icons-MIT.txt`). The font is [Recursive](https://github.com/arrowtype/recursive), Copyright 2019 The Recursive Project Authors, licensed under the SIL Open Font License 1.1 (`fonts/OFL.txt`).

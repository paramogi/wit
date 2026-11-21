A lightweight, static Git web frontend written in Rust

## Installation

Create dependencies:

- git2
- chrono
- pulldown-cmark
- anyhow
- imagesize

## Usage:

// TODO

### Stylesheet

wit doesn't write a stylesheet. Copy `style.css` into the site dir (the one after `-o`), and copy it again after changing it.

### Rebuilding on every push

Copy `contrib/post-receive` into each bare repo's `hooks/` dir, keep it executable, and set its paths: the site dir after `-o`, then every repo the site lists. git runs the hook inside the pushed repo, so `-u .` rebuilds only that repo, plus any repo that has no pages yet; the index is rewritten from all of them. Run wit without `-u` to rebuild every repo, e.g. after updating wit or changing a description or owner. A rebuilt repo's old pages are deleted first.

## Credits

The icons are from [Octicons](https://github.com/primer/octicons), Copyright (c) 2026 GitHub Inc., licensed under the [MIT License](https://github.com/primer/octicons/blob/main/LICENSE).

---

<a href="https://brainmade.org/">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://brainmade.org/88x31-dark.png">
    <source media="(prefers-color-scheme: light)" srcset="https://brainmade.org/88x31-light.png">
    <img alt="Brain made dot org" src="https://brainmade.org/88x31-light.png">
  </picture>
</a>

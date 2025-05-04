alias b := build

build:
  cargo build --release && cp target/release/sync_bookmarks ~/bin && cp target/release/sync_bookmarks.1 ~/man/man1

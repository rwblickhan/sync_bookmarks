alias i := install

build:
  cargo build --release

copy_bin: build
  cp target/release/sync_bookmarks ~/bin 

copy_man: build
  cp target/release/sync_bookmarks.1 ~/man/man1

copy_completions: build
  cp target/release/sync_bookmarks.fish ~/.config/fish/completions

install: build copy_bin copy_man copy_completions

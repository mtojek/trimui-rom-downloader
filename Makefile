.PHONY: dev build clean

dev:
	LIBRARY_PATH=/opt/homebrew/opt/sdl2/lib cargo run

build:
	cargo build --release

clean:
	cargo clean

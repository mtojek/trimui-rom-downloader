.PHONY: dev dev-dirs build build-tsp clean docker-build docker-shell tsp

IMAGE_NAME := trimui-rom-downloader-toolchain
WORKSPACE_DIR := $(shell pwd)

DEV_ROM_DIR := /tmp/trimui-rom-downloader/mnt/SDCARD/Roms

dev: dev-dirs
	RUST_BACKTRACE=1 \
	LIBRARY_PATH=/opt/homebrew/opt/sdl2/lib \
	TRD_ROM_BASE_DIR=$(DEV_ROM_DIR) \
	cargo run

dev-dirs:
	@mkdir -p "$(DEV_ROM_DIR)/Sony PlayStation (PS)"
	@mkdir -p "$(DEV_ROM_DIR)/Nintendo Entertainment System (FC)"
	@mkdir -p "$(DEV_ROM_DIR)/Game Boy Advance (GBA)"
	@mkdir -p "$(DEV_ROM_DIR)/Game Boy Color (GBC)"
	@mkdir -p "$(DEV_ROM_DIR)/Game Boy (GB)"
	@mkdir -p "$(DEV_ROM_DIR)/Super Nintendo Entertainment System (SFC)"
	@mkdir -p "$(DEV_ROM_DIR)/Sega Genesis (MD)"
	@mkdir -p "$(DEV_ROM_DIR)/Sega CD (SEGACD)"
	@mkdir -p "$(DEV_ROM_DIR)/Arcade (FBN)"

build:
	LIBRARY_PATH=/opt/homebrew/opt/sdl2/lib \
	cargo build --release

tsp: docker-build
	docker run --rm -v "$(WORKSPACE_DIR)":/workspace $(IMAGE_NAME) bash -c 'make build-tsp'

build-tsp:
	PKG_CONFIG_ALLOW_CROSS=1 \
	CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-nextui-linux-gnu-gcc \
	SDL2_INCLUDE_PATH="$${PREFIX}/include/SDL2" \
	SDL2_LIB_PATH="$${PREFIX}/lib" \
	RUSTFLAGS="-C link-args=-Wl,-rpath,$${PREFIX}/lib -L $${PREFIX}/lib" \
	cargo build --release --target aarch64-unknown-linux-gnu
	cp target/aarch64-unknown-linux-gnu/release/trimui-rom-downloader "ROM Downloader.pak/trimui-rom-downloader"
	chmod +x "ROM Downloader.pak/trimui-rom-downloader"

docker-build: Dockerfile
	docker build -t $(IMAGE_NAME) .

docker-shell: docker-build
	docker run -it --rm -v "$(WORKSPACE_DIR)":/workspace $(IMAGE_NAME) bash
	
clean:
	cargo clean
	rm -f "ROM Downloader.pak/trimui-rom-downloader"
	rm -rf /tmp/trimui-rom-downloader
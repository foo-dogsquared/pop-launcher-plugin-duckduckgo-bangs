BANGS_DB_URL = https://duckduckgo.com/bang.js
LAUNCHER_PATH = $(HOME)/.local/share/pop-launcher
PLUGIN_DIR = $(LAUNCHER_PATH)/plugins
PROFILE = "debug"

PHONY := build test clean format install uninstall

build: test
	cargo build -Z unstable-options --profile $(PROFILE)

test:
	cargo fmt -- --check && cargo clippy -- -Dwarnings

clean:
	cargo clean

format:
	rustfmt src/*.rs

install: build
	mkdir -p ${PLUGIN_DIR}/bangs
	install -Dm0644 src/plugin.ron ${PLUGIN_DIR}/bangs/plugin.ron
	ln -sf $(realpath target/debug/bangs) ${PLUGIN_DIR}/bangs/bangs
	curl ${BANGS_DB_URL} --output ${PLUGIN_DIR}/bangs/db.json

uninstall:
	rm -r ${PLUGIN_DIR}/bangs

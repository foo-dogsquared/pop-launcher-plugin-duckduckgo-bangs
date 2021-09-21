BANGS_DB_URL = https://duckduckgo.com/bang.js
LAUNCHER_PATH = $(HOME)/.local/share/pop-launcher
PLUGIN_DIR = $(LAUNCHER_PATH)/plugins

PHONY := build clean install uninstall

build:
	cargo build

clean:
	cargo clean

install:
	mkdir -p ${PLUGIN_DIR}/bangs
	install -Dm0644 src/plugin.ron ${PLUGIN_DIR}/bangs/plugin.ron
	ln -sf $(realpath target/debug/pop-shell-launcher-bangs) ${PLUGIN_DIR}/bangs/bangs
	curl ${BANGS_DB_URL} --output ${PLUGIN_DIR}/bangs/db.json

uninstall:
	rm -r ${PLUGIN_DIR}/bangs

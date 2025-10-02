PYTHON ?= python3
INDEX_PATH ?= $(HOME)/.local/state/hauski/index
NAMESPACE ?= default
VAULT ?= $(PWD)/../../seeds/obsidian.sample

GEWEBE_DIR := $(INDEX_PATH)/$(NAMESPACE)/.gewebe

.PHONY: all index graph related clean ensure-dirs

all: index graph related

index: ensure-dirs
	$(PYTHON) build_index.py --index-path "$(INDEX_PATH)" --namespace "$(NAMESPACE)"

graph: ensure-dirs
	$(PYTHON) build_graph.py --index-path "$(INDEX_PATH)" --namespace "$(NAMESPACE)"

related: ensure-dirs
ifeq ($(CHECK),1)
	$(PYTHON) update_related.py --index-path "$(INDEX_PATH)" --namespace "$(NAMESPACE)" --vault "$(VAULT)" --check
else
	$(PYTHON) update_related.py --index-path "$(INDEX_PATH)" --namespace "$(NAMESPACE)" --vault "$(VAULT)"
endif

clean:
	rm -rf "$(GEWEBE_DIR)"

ensure-dirs:
	@mkdir -p "$(GEWEBE_DIR)"

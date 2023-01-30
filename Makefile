DEBUG ?= 1
TARGET ?=

SHELL := /usr/bin/env bash

ifneq ($(DEBUG), 0)
	BUILD_ARGS :=
	TARGET_DIR = target/$(TARGET)/debug
else
	BUILD_ARGS := $(BUILD_ARGS) --release
	TARGET_DIR = target/$(TARGET)/release
endif

ifdef TARGET
  BUILD_ARGS := $(BUILD_ARGS) --target=$(TARGET)
endif

CARGO ?= cargo

PACKAGE_METADATA = $(shell cargo metadata --format-version=1 --no-deps)
PACKAGE_NAME = $(shell echo '$(PACKAGE_METADATA)' | jq -r .packages[0].name)
CURRENT_VERSION = $(shell echo '$(PACKAGE_METADATA)' | jq -r .packages[0].version)
CURRENT_REVISION = $(shell git rev-parse --short HEAD)

ifeq ($(RELEASE), 1)
	VERSION = $(CURRENT_VERSION)
else
	VERSION = $(CURRENT_VERSION)-$(CURRENT_REVISION)
endif

DIST_DIR = target/dist
PACKAGE_NAME_AND_VERSION = $(PACKAGE_NAME)-$(VERSION)-$(SUFFIX)
ARCHIVE_DIR = $(DIST_DIR)/$(PACKAGE_NAME_AND_VERSION)
ARCHIVE_NAME = $(PACKAGE_NAME_AND_VERSION).tar.zz

.DEFAULT_GOAL = help
.PHONY: dist build check-dist help container-binaries

dist: check-dist build
	rm -rf $(ARCHIVE_DIR)
	mkdir -p $(ARCHIVE_DIR)
ifneq (, $(findstring linux, $(TARGET)))
	$(info Building distribution with Linux layout)
	mkdir -p $(ARCHIVE_DIR)/etc/$(PACKAGE_NAME) \
		$(ARCHIVE_DIR)/usr/local/bin \
		$(ARCHIVE_DIR)/etc/systemd/system
	cp $(TARGET_DIR)/$(PACKAGE_NAME) $(ARCHIVE_DIR)/usr/local/bin/
	cp $(PACKAGE_NAME).service $(ARCHIVE_DIR)/etc/systemd/system/
	cp weathermen.toml.dist $(ARCHIVE_DIR)/etc/$(PACKAGE_NAME)/
else
	$(info Building distribution with basic layout)
	cp $(TARGET_DIR)/$(PACKAGE_NAME) $(ARCHIVE_DIR)/
	cp weathermen.toml.dist $(ARCHIVE_DIR)
endif
	tar -C $(DIST_DIR) -Jcvf $(ARCHIVE_NAME) $(PACKAGE_NAME_AND_VERSION)

build:
	touch -d 1970-01-01T00:00:00.00 build.rs
	PROMW_VERSION=$(VERSION) $(CARGO) build $(BUILD_ARGS)

check-dist:
ifndef SUFFIX
	_ := $(error SUFFIX must be defined)
endif

container-binaries: $(wildcard $(BINARY_ARCHIVE_DIR)/*/*.tar.zz)
	mkdir -p $(CONTAINER_BINARY_DIR)
	for archive in $^; do tar -C $(CONTAINER_BINARY_DIR) -Jxf $$archive ; done
	platforms=(linux/amd64 linux/arm64 linux/arm/v7); \
	targets=(x86_64-linux-static arm64-linux-static arm-linux-static); \
	let "len = $${#platforms[@]} - 1"; \
	for n in $$(seq 0 $$len); do \
	  platform=$${platforms[$$n]}; \
	  target=$${targets[$$n]}; \
	  mkdir -p $(CONTAINER_BINARY_DIR)/$${platform}; \
	  cp $(CONTAINER_BINARY_DIR)/prometheus-weathermen-*-$${target}/usr/local/bin/prometheus-weathermen $(CONTAINER_BINARY_DIR)/$${platform}; \
	done; \
	echo platforms=$${platforms[@]} | tr " " "," >> $(PLATFORM_FILE)

help:
	@echo "Targets:"
	@echo "Build dev target on the current machine: make build"
	@echo "Build distribution package:              make dist [DEBUG=1] [RELEASE=0] [TARGET=] [SUFFIX=]"
	@echo "Prepare container binaries:              make container-binaries [BINARY_ARCHIVE_DIR=] [CONTAINER_BINARY_DIR=] [PLATFORM_FILE=]"

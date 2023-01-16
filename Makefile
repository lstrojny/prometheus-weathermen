DEBUG ?= 1
TARGET ?=

ifneq ($(DEBUG), 0)
	BUILD_ARGS :=
	TARGET_DIR = target/$(TARGET)/debug/
else
	BUILD_ARGS := $(BUILD_ARGS) --release
	TARGET_DIR = target/$(TARGET)/release/
endif

ifdef TARGET
  BUILD_ARGS := $(BUILD_ARGS) --target=$(TARGET)
endif

CARGO ?= cargo

PACKAGE_METADATA = $(shell cargo metadata --format-version=1 --no-deps)
PACKAGE_NAME = $(shell echo '$(PACKAGE_METADATA)' | jq -r .packages[0].name)
CURRENT_VERSION = $(shell echo '$(PACKAGE_METADATA)' | jq -r .packages[0].version)
CURRENT_REVISION = $(shell git rev-parse --short HEAD)

ifneq ($(RELEASE), 0)
	VERSION = $(CURRENT_VERSION)
else
	VERSION = $(CURRENT_VERSION)-$(CURRENT_REVISION)
endif

DIST_DIR = target/dist
PACKAGE_NAME_AND_VERSION = $(PACKAGE_NAME)-$(VERSION)-$(SUFFIX)
ARCHIVE_DIR = $(DIST_DIR)/$(PACKAGE_NAME_AND_VERSION)
ARCHIVE_NAME = $(PACKAGE_NAME_AND_VERSION).tar.zz

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
	$(CARGO) build $(BUILD_ARGS)

check-dist:
ifndef SUFFIX
	_ := $(error SUFFIX must be defined)
endif

all: build

help:
	@echo "usage: make [DEBUG=1] [RELEASE=0] [TARGET=] [SUFFIX=]"

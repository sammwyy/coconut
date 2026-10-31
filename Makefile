.PHONY: setup build dev install test

setup:
	./scripts/setup.sh

build:
	./scripts/build.sh

dev:
	./scripts/dev.sh

install:
	./scripts/install.sh

test:
	cargo test --workspace
	PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests -p 'test_*.py' -v

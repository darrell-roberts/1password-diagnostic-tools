default: build test

taplo_fmt *args='':
    rg --files -g 'Cargo.toml' -g 'taplo.toml' | sort -u | xargs taplo fmt {{ args }}

format:
    echo "Formatting..."
    cargo fmt
    just taplo_fmt

check: format
    cargo clippy

build: check
    cargo build -p diagnostic-tui

install:
    cargo install --path diagnostic-tui

test:
    cargo test

clean:
    cargo clean

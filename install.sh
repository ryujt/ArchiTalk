#!/bin/sh
set -eu

usage() {
    cat <<'EOF'
Usage: ./install.sh [--prefix DIRECTORY]

Build and install architalk using Cargo (no sudo required).
By default Cargo chooses its normal installation root and bin directory.
Use --prefix DIRECTORY to install into DIRECTORY/bin instead.

Requires Rust/Cargo and a C compiler. Run this script from any directory.
EOF
}

install_prefix=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --prefix)
            if [ "$#" -lt 2 ] || [ -z "$2" ]; then
                printf '%s\n' 'Error: --prefix requires a directory.' >&2
                exit 2
            fi
            case "$2" in
                -*) printf '%s\n' 'Error: --prefix requires a directory, not an option.' >&2; exit 2 ;;
            esac
            install_prefix=$2
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            printf 'Error: unknown argument: %s\n' "$1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if ! command -v cargo >/dev/null 2>&1; then
    printf '%s\n' 'Error: Cargo was not found. Install Rust and reopen your terminal first.' >&2
    exit 1
fi

script_dir=$(CDPATH= cd -P -- "$(dirname -- "$0")" && pwd -P)
if [ ! -f "$script_dir/Cargo.toml" ]; then
    printf '%s\n' 'Error: run install.sh from the ArchiTalk source checkout.' >&2
    exit 1
fi

# Keep the caller's working directory so a relative prefix has the expected
# meaning. Reuse the checkout's release artifacts instead of a temporary build.
if [ -n "$install_prefix" ]; then
    cargo install --path "$script_dir" --bin architalk --locked --force \
        --target-dir "$script_dir/target" --root "$install_prefix"
else
    # Let Cargo honor CARGO_INSTALL_ROOT, install.root and CARGO_HOME itself.
    cargo install --path "$script_dir" --bin architalk --locked --force \
        --target-dir "$script_dir/target"
fi

printf '\n%s\n' 'ArchiTalk installation complete.'
if [ -n "$install_prefix" ]; then
    install_bin=$(CDPATH= cd -P -- "$install_prefix/bin" && pwd -P)
    printf 'Installed command: %s/architalk\n' "$install_bin"
    printf 'Ensure this directory comes first on PATH: %s\n' "$install_bin"
else
    printf '%s\n' 'Ensure the bin directory shown in Cargo output comes first on PATH.'
fi
if command -v architalk >/dev/null 2>&1; then
    printf 'Current PATH resolves architalk to: %s\n' "$(command -v architalk)"
else
    printf '%s\n' 'Reopen your terminal after updating PATH.'
fi
printf '%s\n' 'Run architalk from your project directory once PATH is configured.'

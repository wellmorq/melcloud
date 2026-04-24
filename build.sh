#!/usr/bin/env bash
set -euo pipefail

output_dir="build"
copy_env=1
copy_runtime_state=1

while [[ $# -gt 0 ]]; do
    case "$1" in
        --output-dir)
            if [[ $# -lt 2 ]]; then
                echo "--output-dir expects a value" >&2
                exit 2
            fi
            output_dir="$2"
            shift 2
            ;;
        --no-env)
            copy_env=0
            shift
            ;;
        --no-runtime-state)
            copy_runtime_state=0
            shift
            ;;
        -h|--help)
            cat <<'HELP'
Usage: ./build.sh [--output-dir DIR] [--no-env] [--no-runtime-state]

Builds the release workspace and creates a portable Linux runtime package.
HELP
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 2
            ;;
    esac
done

root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
output_path="$(realpath -m "$root/$output_dir")"

case "$output_path" in
    "$root")
        echo "Output directory must be a child directory inside the repository root: $output_dir" >&2
        exit 2
        ;;
    "$root"/*) ;;
    *)
        echo "Output directory must be a child directory inside the repository root: $output_dir" >&2
        exit 2
        ;;
esac

copy_required_file() {
    local source="$1"
    local destination="$2"
    if [[ ! -f "$source" ]]; then
        echo "Required file is missing: $source" >&2
        exit 1
    fi
    mkdir -p "$(dirname "$destination")"
    cp -f "$source" "$destination"
}

copy_required_dir() {
    local source="$1"
    local destination="$2"
    if [[ ! -d "$source" ]]; then
        echo "Required directory is missing: $source" >&2
        exit 1
    fi
    mkdir -p "$destination"
    cp -a "$source/." "$destination/"
}

copy_optional_dir_contents() {
    local source="$1"
    local destination="$2"
    mkdir -p "$destination"
    if [[ -d "$source" ]]; then
        cp -a "$source/." "$destination/"
    fi
}

write_text_file() {
    local path="$1"
    local content="$2"
    mkdir -p "$(dirname "$path")"
    printf '%s\n' "$content" > "$path"
}

cd "$root"
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" cargo build --release --workspace

rm -rf "$output_path"
mkdir -p \
    "$output_path/bin" \
    "$output_path/melcloud-cli/presets" \
    "$output_path/melcloud-cli/state" \
    "$output_path/melcloud-site/state" \
    "$output_path/melcloud-site/cache/weather-icons"

copy_required_file "$root/target/release/melcloud-cli" "$output_path/bin/melcloud-cli"
copy_required_file "$root/target/release/melcloud-site" "$output_path/bin/melcloud-site"
copy_required_dir "$root/melcloud-site/public" "$output_path/melcloud-site/public"
asset_version="$(date -u +%Y%m%d%H%M%S)"
sed -i "s/?v=dev/?v=$asset_version/g" "$output_path/melcloud-site/public/index.html"
write_text_file "$output_path/melcloud-site/public/js/build-version.js" "export const assetVersion = \"$asset_version\";"
copy_required_dir "$root/melcloud-site/site-assets" "$output_path/melcloud-site/site-assets"
copy_required_file "$root/melcloud-site/melcloud-site.yaml" "$output_path/melcloud-site/melcloud-site.yaml"

if [[ "$copy_env" -eq 1 && -f "$root/.env" ]]; then
    copy_required_file "$root/.env" "$output_path/.env"
else
    write_text_file "$output_path/.env.example" "login=your@email
password=your-password
language=ru"
fi

if [[ "$copy_runtime_state" -eq 1 ]]; then
    copy_optional_dir_contents "$root/melcloud-cli/presets" "$output_path/melcloud-cli/presets"
    copy_optional_dir_contents "$root/melcloud-cli/state" "$output_path/melcloud-cli/state"
    copy_optional_dir_contents "$root/melcloud-site/state" "$output_path/melcloud-site/state"
    copy_optional_dir_contents "$root/melcloud-site/cache" "$output_path/melcloud-site/cache"
fi

mkdir -p "$output_path/melcloud-site/cache/weather-icons"

write_text_file "$output_path/run-site.sh" '#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
exec ./bin/melcloud-site'
chmod +x "$output_path/run-site.sh"

write_text_file "$output_path/README_RUNTIME.txt" 'MelCloud runtime package

Run:
  ./bin/melcloud-site

Open:
  http://127.0.0.1:8787/

Runtime files:
  .env
  bin/
  melcloud-cli/presets/
  melcloud-cli/state/
  melcloud-site/state/
  melcloud-site/cache/
  melcloud-site/melcloud-site.yaml

The site calls bin/melcloud-cli from this same folder.'

echo "Runtime package created: $output_path"

#!/bin/sh
# Type-checks and lints srl for Windows, from Linux or macOS.
#
# Nothing is built or run for Windows: `cargo clippy` never links, so the C
# compiler that the bundled SQLite would need is replaced by a stub that
# writes empty object files. This catches code behind `cfg(windows)` that
# doesn't compile or breaks a lint; it can't show that it works. See
# docs/rust.md, "Platform independence".
set -eu

target=x86_64-pc-windows-gnu
rustup target add "$target" >/dev/null

stub=$(mktemp -d)
trap 'rm -rf "$stub"' EXIT
cat > "$stub/cc" <<'STUB'
#!/bin/sh
# Answers like a compiler, and writes an empty file for `-o FILE` or `-FoFILE`.
out=""
previous=""
for arg in "$@"; do
  [ "$previous" = "-o" ] && out="$arg"
  case "$arg" in -Fo*) out="${arg#-Fo}" ;; esac
  previous="$arg"
done
[ -n "$out" ] && : > "$out"
exit 0
STUB
chmod +x "$stub/cc"

ar=$(command -v llvm-ar || command -v ar)
CC_x86_64_pc_windows_gnu="$stub/cc" AR_x86_64_pc_windows_gnu="$ar" \
  cargo clippy --workspace --all-targets --target "$target" \
  --target-dir target/windows-check -- -D warnings

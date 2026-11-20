#!/usr/bin/env bash
# Checks memi-engine's Kotlin bindings on this computer (no Android needed): builds the library,
# generates the bindings, compiles them with Check.kt, and runs it.
#   KOTLINC   the Kotlin compiler command (default: kotlinc)
#   JNA_JAR   path to jna.jar (default: downloaded from Maven Central to target/)
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build -q
LIB=target/debug/libmemi_ffi.so
OUT=target/kotlin-check
rm -rf "$OUT" && mkdir -p "$OUT"
cargo run -q --features bindgen --bin uniffi-bindgen -- generate --library "$LIB" --language kotlin --out-dir "$OUT/src" --no-format
JNA_JAR=${JNA_JAR:-target/jna-5.17.0.jar}
[ -f "$JNA_JAR" ] || curl -sSfL -o "$JNA_JAR" https://repo1.maven.org/maven2/net/java/dev/jna/jna/5.17.0/jna-5.17.0.jar
${KOTLINC:-kotlinc} -nowarn -cp "$JNA_JAR" -d "$OUT/check.jar" -include-runtime "$OUT/src/app/memi/engine/memi_ffi.kt" tests/kotlin/Check.kt
java -Djna.library.path=target/debug -cp "$OUT/check.jar:$JNA_JAR" CheckKt

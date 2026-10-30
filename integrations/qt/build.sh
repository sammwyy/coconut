#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
output=${1:-$root/.build/qt}
for major in ${COCONUT_QT_MAJORS:-6}; do
    case "$major" in 5|6) ;; *) echo "Unsupported Qt major: $major" >&2; exit 1 ;; esac
    build="$output/build-qt$major"
    cmake -S "$root/integrations/qt/plugin" -B "$build" \
        -DCMAKE_BUILD_TYPE=Release -DCOCONUT_QT_MAJOR="$major" -DBUILD_TESTING=OFF
    cmake --build "$build" --parallel "${BUILD_JOBS:-4}"
    install -Dm0755 "$build/plugins/platformthemes/libcoconut-qt$major.so" \
        "$output/plugins/platformthemes/libcoconut-qt$major.so"
done

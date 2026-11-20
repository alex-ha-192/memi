#!/usr/bin/env bash
# Runs the screenshot tour (app/src/androidTest/.../ScreenTour.kt) in several passes and
# copies the pictures out. Used by the "Android screenshots" CI job.
set -u
adb install -r app/build/outputs/apk/debug/app-debug.apk
adb install -r app/build/outputs/apk/androidTest/debug/app-debug-androidTest.apk
run() {
  adb shell am instrument -w -e pass "$1" app.memi.test/androidx.test.runner.AndroidJUnitRunner | tail -25
}
adb shell settings put system font_scale 1.0
adb shell cmd uimode night no
run phone
adb shell cmd uimode night yes
adb shell settings put system font_scale 1.3
run phone-dark-bigtext
adb shell cmd uimode night no
adb shell settings put system font_scale 1.0
adb shell wm size 2560x1600
adb shell wm density 320
run tablet
adb shell wm size reset
adb shell wm density reset
mkdir -p shots
adb exec-out run-as app.memi tar c -C files shots | tar x -C . || true
ls -la shots

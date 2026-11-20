#!/usr/bin/env bash
# Runs the screenshot tour (app/src/androidTest/.../ScreenTour.kt) in several passes and
# copies the pictures out. Used by the "Android screenshots" CI job.
set -u
adb install -r app/build/outputs/apk/debug/app-debug.apk
adb install -r app/build/outputs/apk/androidTest/debug/app-debug-androidTest.apk
# The CI emulator is slow, and "System UI isn't responding" pop-ups would cover the screenshots
# and steal input from the tour. Hide them and give the system time to settle first.
adb shell settings put global hide_error_dialogs 1
adb shell settings put global anr_show_background 0
settle() {
  adb shell 'while [ "$(getprop sys.boot_completed)" != 1 ]; do sleep 2; done'
  sleep 20
  adb shell input keyevent KEYCODE_WAKEUP
  adb shell wm dismiss-keyguard || true
  adb shell am broadcast -a android.intent.action.CLOSE_SYSTEM_DIALOGS >/dev/null || true
}
run() {
  settle
  adb shell am instrument -w -e pass "$1" app.memi.test/androidx.test.runner.AndroidJUnitRunner | tail -60
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

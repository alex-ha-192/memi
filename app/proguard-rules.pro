# Compose ships its own consumer rules.

# memi-engine: the Kotlin bindings (app.memi.engine) reach libmemi_ffi.so through JNA, which finds
# classes, fields and callbacks by name, so neither may be renamed or removed.
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.** { public *; }
-keep class app.memi.engine.** { *; }
-dontwarn java.awt.**

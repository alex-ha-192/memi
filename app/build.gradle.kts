plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
}

android {
    namespace = "app.memi"
    compileSdk = 35

    defaultConfig {
        applicationId = "app.memi"
        minSdk = 26
        targetSdk = 35
        // Release builds from CI pass -PmemiVersion=1.2.3 (from the git tag), so every release
        // has a higher versionCode and updates install over the old one (Obtainium relies on this).
        val release = (project.findProperty("memiVersion") as String?)?.removePrefix("v")
        versionName = release ?: "0.6.2"
        versionCode = versionName!!.split('.').map { it.toIntOrNull() ?: 0 }.let { (it + listOf(0, 0, 0)).take(3) }
            .let { (major, minor, patch) -> major * 10_000 + minor * 100 + patch }
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    signingConfigs {
        create("release") {
            System.getenv("MEMI_KEYSTORE")?.let { path ->
                storeFile = file(path)
                storePassword = System.getenv("MEMI_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("MEMI_KEY_ALIAS")
                keyPassword = System.getenv("MEMI_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
            // CI signs with the key from the repo's secrets (see README > Automatic builds).
            // Locally, it falls back to your debug key so a release build installs straight away.
            signingConfig = if (System.getenv("MEMI_KEYSTORE") != null) signingConfigs.getByName("release")
            else signingConfigs.getByName("debug")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
    buildFeatures {
        compose = true
    }
}

// --- memi-engine (Rust, in ffi/ and engine/) --------------------------------------------------------
//
// Below the screens, the app runs memi-engine, the same Rust engine as the desktop and web apps.
// Gradle builds it for each Android ABI with cargo-ndk (`cargo install cargo-ndk`, plus the
// Android NDK and `rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android`),
// then generates its Kotlin bindings (package app.memi.engine) with UniFFI.

val rustDir = rootProject.file("ffi")
val rustJniLibs = layout.buildDirectory.dir("rust/jniLibs")
val rustBindings = layout.buildDirectory.dir("generated/uniffi")
val rustAbis = listOf("arm64-v8a", "armeabi-v7a", "x86_64")

val buildRust by tasks.registering(Exec::class) {
    description = "Builds memi-engine (ffi/) for Android with cargo-ndk."
    workingDir = rustDir
    inputs.files(fileTree(rootProject.file("ffi")) { exclude("target/**") }, fileTree(rootProject.file("engine")) { exclude("target/**") }, fileTree(rootProject.file("core")) { exclude("target/**") })
    outputs.dir(rustJniLibs)
    val out = rustJniLibs.get().asFile
    commandLine(listOf("cargo", "ndk") + rustAbis.flatMap { listOf("-t", it) } + listOf("--platform", "26", "-o", out.path, "build", "--release"))
}

val rustKotlin by tasks.registering(Exec::class) {
    description = "Generates memi-engine's Kotlin bindings with UniFFI (from a build for this computer, which keeps the symbols UniFFI reads; the Android libraries are stripped)."
    dependsOn(buildRust)
    workingDir = rustDir
    inputs.dir(rustJniLibs)
    outputs.dir(rustBindings)
    val out = rustBindings.get().asFile
    val ext = when {
        org.gradle.internal.os.OperatingSystem.current().isWindows -> "dll"
        org.gradle.internal.os.OperatingSystem.current().isMacOsX -> "dylib"
        else -> "so"
    }
    val prefix = if (ext == "dll") "" else "lib"
    commandLine("sh", "-c", "cargo build --lib && cargo run --features bindgen --bin uniffi-bindgen -- generate --library target/debug/${prefix}memi_ffi.$ext --language kotlin --out-dir '${out.path}' --no-format")
}

android.sourceSets.getByName("main") {
    jniLibs.srcDir(rustJniLibs.get().asFile)
    java.srcDir(rustBindings.get().asFile)
}

tasks.named("preBuild") { dependsOn(rustKotlin) }

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.lifecycle.viewmodel.compose)

    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.foundation)
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.compose.material.icons.extended)
    implementation(libs.androidx.compose.ui.tooling.preview)
    debugImplementation(libs.androidx.compose.ui.tooling)

    // memi-engine's bindings call into libmemi_ffi.so through JNA.
    implementation(libs.jna) { artifact { type = "aar" } }

    implementation(libs.androidx.datastore.preferences)
    implementation(libs.play.services.auth)
    implementation(libs.play.services.blockstore)
    implementation(libs.play.services.code.scanner)
    implementation(libs.androidx.exifinterface)

    // Installs Compose's ahead-of-time profiles on release builds, which makes scrolling and animations much smoother.
    implementation(libs.androidx.profileinstaller)

    // Merging, backups and encryption are tested in Rust (engine/tests, core); with ffi/tests checking the Kotlin bindings.
    testImplementation(libs.junit)

    // Screenshot tour (androidTest/ScreenTour.kt), run on an emulator in CI.
    androidTestImplementation(platform(libs.androidx.compose.bom))
    androidTestImplementation(libs.androidx.compose.ui.test.junit4)
    androidTestImplementation(libs.androidx.test.ext.junit)
    androidTestImplementation(libs.androidx.test.runner)
    debugImplementation(libs.androidx.compose.ui.test.manifest)
}

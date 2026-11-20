import org.jetbrains.compose.desktop.application.dsl.TargetFormat

plugins {
    alias(libs.plugins.kotlin.jvm)
    alias(libs.plugins.jetbrains.compose)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.ksp)
}

kotlin {
    jvmToolchain(17)
    // The same data, sync and screens as the Android app.
    sourceSets["main"].kotlin.srcDir("../shared/src/main/kotlin")
}

ksp {
    arg("room.schemaLocation", "$projectDir/schemas")
}

dependencies {
    implementation(compose.desktop.currentOs)
    implementation(compose.material3)
    implementation(compose.materialIconsExtended)
    implementation(libs.jb.lifecycle.viewmodel.compose)
    implementation(libs.jb.lifecycle.runtime.compose)
    implementation(libs.kotlinx.coroutines.swing)

    implementation(libs.desktop.room.runtime)
    implementation(libs.desktop.sqlite.bundled)
    ksp(libs.desktop.room.compiler)
    implementation(libs.desktop.datastore.preferences.core)

    implementation(libs.org.json)
    implementation(libs.java.keyring)
}

compose.desktop {
    application {
        mainClass = "app.memi.MainKt"

        nativeDistributions {
            // Installers are built on the system they're for: .msi on Windows, .deb on Linux.
            targetFormats(TargetFormat.Msi, TargetFormat.Deb)
            packageName = "Memi"
            packageVersion = "0.4.0"
            description = "Remember the little things about the people in your life"
            vendor = "Memi"
            // Bundles the whole Java runtime so sign-in (local web server) and HTTPS always work.
            includeAllModules = true

            windows {
                iconFile.set(project.file("icons/memi.ico"))
                menu = true
                shortcut = true
                perUserInstall = true
                // Keeps upgrades replacing the old version instead of installing beside it.
                upgradeUuid = "6f1d1d6e-6b8c-4c1b-9a0e-5a8f0d6b2c41"
            }
            linux {
                iconFile.set(project.file("icons/memi.png"))
                shortcut = true
                appCategory = "Office"
                debMaintainer = "memi@localhost"
            }
        }

        buildTypes.release.proguard {
            // Room finds its generated code by name, so keep the release build un-shrunk.
            isEnabled.set(false)
        }
    }
}

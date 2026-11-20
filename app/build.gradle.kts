plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.ksp)
}

android {
    namespace = "app.memi"
    compileSdk = 35

    defaultConfig {
        applicationId = "app.memi"
        minSdk = 26
        targetSdk = 35
        versionCode = 4
        versionName = "0.4.0"
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
    // Most of Memi (data, sync and all the screens) lives in ../shared and is also used by the desktop app.
    sourceSets["main"].java.srcDir("../shared/src/main/kotlin")
}

ksp {
    arg("room.schemaLocation", "$projectDir/schemas")
}

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

    implementation(libs.androidx.room.runtime)
    implementation(libs.androidx.room.ktx)
    ksp(libs.androidx.room.compiler)

    implementation(libs.androidx.datastore.preferences)
    implementation(libs.play.services.auth)
    implementation(libs.play.services.blockstore)
    implementation(libs.androidx.exifinterface)

    // Installs Compose's ahead-of-time profiles on release builds, which makes scrolling and animations much smoother.
    implementation(libs.androidx.profileinstaller)
}

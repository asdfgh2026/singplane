plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

val singBoxVersion = findProperty("singBoxVersion")?.toString()?.trim().orEmpty()
    .ifEmpty { "1.13.19" }
    .removePrefix("v")

android {
    namespace = "app.singplane"
    compileSdk = 35

    defaultConfig {
        applicationId = "app.singplane"
        minSdk = 26
        targetSdk = 35
        versionCode = appVersionCode()
        versionName = appVersionName()
        val coreVersion = singBoxVersion
            .replace("\\", "\\\\")
            .replace("\"", "\\\"")
        buildConfigField("String", "SING_BOX_VERSION", "\"$coreVersion\"")
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("debug")
        }
        debug {
            isDebuggable = true
            applicationIdSuffix = ".debug"
            versionNameSuffix = "-debug"
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    testOptions {
        unitTests.all {
            it.systemProperty("singBoxVersion", singBoxVersion)
        }
    }

    packaging {
        resources {
            excludes += "/META-INF/{AL2.0,LGPL2.1}"
        }
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
    }
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2024.12.01")
    implementation(composeBom)
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.7")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.7")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("org.apache.commons:commons-compress:1.27.1")
    implementation("androidx.work:work-runtime-ktx:2.9.1")
    implementation(files("libs/libbox.aar"))
    debugImplementation("androidx.compose.ui:ui-tooling")



    testImplementation("junit:junit:4.13.2")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.9.0")
    testImplementation("org.json:json:20240303")
    testImplementation("com.squareup.okhttp3:mockwebserver:4.12.0")
    testImplementation("com.google.truth:truth:1.4.4")
}

fun appVersionName(): String {
    val fromProp = findProperty("appVersion")?.toString()?.trim().orEmpty()
    val fromEnv = System.getenv("SINGPANEL_VERSION")?.trim().orEmpty()
    return when {
        fromProp.isNotEmpty() -> fromProp
        fromEnv.isNotEmpty() -> fromEnv
        else -> "0.0.1"
    }
}

fun appVersionCode(): Int {
    val raw = appVersionName().substringBefore('-')
    val parts = raw.split('.')
    val major = parts.getOrNull(0)?.toIntOrNull() ?: 0
    val minor = parts.getOrNull(1)?.toIntOrNull() ?: 0
    val patch = parts.getOrNull(2)?.toIntOrNull() ?: 0
    return major * 10000 + minor * 100 + patch
}

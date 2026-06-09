plugins {
  id("com.android.application")
  id("org.jetbrains.kotlin.android")
  id("org.jetbrains.kotlin.plugin.serialization")
  id("org.jetbrains.kotlin.plugin.compose")
}

android {
  namespace = "com.fin.client"
  compileSdk = 34

  val finVersionCode = providers.gradleProperty("finVersionCode")
    .map(String::toInt)
    .getOrElse(1)
  val finVersionName = providers.gradleProperty("finVersionName")
    .getOrElse("0.1.0")

  defaultConfig {
    applicationId = "com.fin.client"
    minSdk = 26
    targetSdk = 34
    versionCode = finVersionCode
    versionName = finVersionName
    testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
  }

  buildTypes {
    release {
      isMinifyEnabled = false
      proguardFiles(
        getDefaultProguardFile("proguard-android-optimize.txt"),
        "proguard-rules.pro"
      )
    }
  }
  compileOptions {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
  }
  kotlinOptions { jvmTarget = "17" }
  buildFeatures { compose = true }
}

dependencies {
  implementation("androidx.core:core-ktx:1.13.1")
  implementation("androidx.activity:activity-compose:1.9.2")
  implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.5")
  implementation("androidx.compose.ui:ui:1.7.1")
  implementation("androidx.compose.material3:material3:1.3.0")
  implementation("androidx.compose.ui:ui-tooling-preview:1.7.1")
  implementation("androidx.navigation:navigation-compose:2.8.0")
  implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.1")
  implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.8.1")
  implementation("com.squareup.okhttp3:okhttp:4.12.0")

  testImplementation("junit:junit:4.13.2")
}

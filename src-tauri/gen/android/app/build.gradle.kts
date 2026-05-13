import java.io.File
import java.util.Properties
import org.gradle.api.GradleException
import org.gradle.api.tasks.Sync
import org.gradle.api.tasks.Delete
import org.gradle.api.tasks.Copy

fun resolveKeystoreFile(rootProjectFile: File, storeFilePath: String): File {
    val configuredFile = File(storeFilePath)
    if (configuredFile.isAbsolute) {
        return configuredFile
    }

    return rootProjectFile.parentFile.resolve(storeFilePath)
}

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

val tauriStagingDir = layout.buildDirectory.dir("tauri-staging")
val tauriStagingAssetsDir = tauriStagingDir.map { it.dir("assets") }
val tauriStagingJniLibsDir = tauriStagingDir.map { it.dir("jniLibs") }

val generatedAssetsSourceDir = layout.projectDirectory.dir("src/main/assets")
val rustTargetDir = layout.projectDirectory.dir("../../../target")

val cleanTauriStaging by tasks.registering(Delete::class) {
    delete(tauriStagingDir)
}

val stageTauriAssets by tasks.registering(Sync::class) {
    dependsOn(cleanTauriStaging)
    from(generatedAssetsSourceDir) {
        include("tauri.conf.json")
    }
    into(tauriStagingAssetsDir)

    doFirst {
        val generatedConfig = generatedAssetsSourceDir.file("tauri.conf.json").asFile
        if (!generatedConfig.isFile) {
            throw GradleException("Missing generated Android asset: ${generatedConfig.path}")
        }
    }
}

val stageTauriJniLibsDebug by tasks.registering(Copy::class) {
    dependsOn(cleanTauriStaging)
    from(rustTargetDir.dir("aarch64-linux-android/debug")) {
        include("libcamera_ftp_companion_lib.so")
    }
    into(tauriStagingJniLibsDir.map { it.dir("arm64-v8a") })

    doFirst {
        val rustLibrary = rustTargetDir.file("aarch64-linux-android/debug/libcamera_ftp_companion_lib.so").asFile
        if (!rustLibrary.isFile) {
            throw GradleException("Missing debug Rust JNI library: ${rustLibrary.path}")
        }
    }
}

val stageTauriJniLibsRelease by tasks.registering(Copy::class) {
    dependsOn(cleanTauriStaging)
    from(rustTargetDir.dir("aarch64-linux-android/release")) {
        include("libcamera_ftp_companion_lib.so")
    }
    into(tauriStagingJniLibsDir.map { it.dir("arm64-v8a") })

    doFirst {
        val rustLibrary = rustTargetDir.file("aarch64-linux-android/release/libcamera_ftp_companion_lib.so").asFile
        if (!rustLibrary.isFile) {
            throw GradleException("Missing release Rust JNI library: ${rustLibrary.path}")
        }
    }
}

val validateStagedJniLibsDebug by tasks.registering {
    dependsOn(stageTauriJniLibsDebug)
    doLast {
        val packagedSoNames = fileTree(tauriStagingJniLibsDir.get().asFile)
            .matching { include("**/*.so") }
            .files
            .map { it.relativeTo(tauriStagingJniLibsDir.get().asFile).invariantSeparatorsPath }
            .toSet()
        val expectedSoNames = setOf("arm64-v8a/libcamera_ftp_companion_lib.so")
        if (packagedSoNames != expectedSoNames) {
            throw GradleException(
                "Unexpected JNI libraries in tauri staging: $packagedSoNames",
            )
        }
    }
}

val validateStagedJniLibsRelease by tasks.registering {
    dependsOn(stageTauriJniLibsRelease)
    doLast {
        val packagedSoNames = fileTree(tauriStagingJniLibsDir.get().asFile)
            .matching { include("**/*.so") }
            .files
            .map { it.relativeTo(tauriStagingJniLibsDir.get().asFile).invariantSeparatorsPath }
            .toSet()
        val expectedSoNames = setOf("arm64-v8a/libcamera_ftp_companion_lib.so")
        if (packagedSoNames != expectedSoNames) {
            throw GradleException(
                "Unexpected JNI libraries in tauri staging: $packagedSoNames",
            )
        }
    }
}

android {
    compileSdk = 36
    namespace = "com.gjk.cameraftpcompanion"
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        applicationId = "com.gjk.cameraftpcompanion"
        minSdk = 33
        targetSdk = 36
        versionCode = tauriProperties.getProperty("tauri.android.versionCode", "1").toInt()
        versionName = tauriProperties.getProperty("tauri.android.versionName", "1.0")
        ndk {
            abiFilters += listOf("arm64-v8a")
        }
    }
    signingConfigs {
        create("release") {
            val keystorePropertiesFile = rootProject.file("keystore.properties")
            if (keystorePropertiesFile.exists()) {
                val keystoreProperties = Properties()
                keystorePropertiesFile.inputStream().use { keystoreProperties.load(it) }
                storeFile = resolveKeystoreFile(
                    keystorePropertiesFile,
                    keystoreProperties.getProperty("storeFile"),
                )
                storePassword = keystoreProperties.getProperty("storePassword")
                keyAlias = keystoreProperties.getProperty("keyAlias")
                keyPassword = keystoreProperties.getProperty("keyPassword")
            }
        }
    }
    buildTypes {
        getByName("debug") {
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
            packaging {
                jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so")
            }
            // Debug 也使用 Release 签名，便于测试
            val keystorePropertiesFile = rootProject.file("keystore.properties")
            if (keystorePropertiesFile.exists()) {
                signingConfig = signingConfigs.getByName("release")
            }
        }
        getByName("release") {
            isMinifyEnabled = true
            val keystorePropertiesFile = rootProject.file("keystore.properties")
            if (keystorePropertiesFile.exists()) {
                signingConfig = signingConfigs.getByName("release")
            }
            proguardFiles(
                *fileTree(".") { include("**/*.pro") }
                    .plus(getDefaultProguardFile("proguard-android-optimize.txt"))
                    .toList().toTypedArray()
            )
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_21
        targetCompatibility = JavaVersion.VERSION_21
    }
    kotlinOptions {
        jvmTarget = "21"
    }
    buildFeatures {
        buildConfig = true
    }
    packaging {
        jniLibs {
            useLegacyPackaging = true
        }
    }
    sourceSets {
        getByName("main") {
            assets.setSrcDirs(listOf(
                tauriStagingAssetsDir.get().asFile,
                generatedAssetsSourceDir.asFile
            ))
            jniLibs.setSrcDirs(listOf(
                tauriStagingJniLibsDir.get().asFile,
                file("${project.projectDir}/extra-jniLibs")
            ))
        }
    }
}

tasks.matching { it.name.endsWith("Assets") && it.name.contains("merge") }.configureEach {
    dependsOn(stageTauriAssets)
    doFirst {
        // HACK: Delete to avoid "Duplicate resources" with the staging copy.
        // AGP's MergeSourceSetFolders has no public exclude API, so we remove
        // the source-tree copy before the merge. The Tauri CLI regenerates it
        // on each build, so this is safe for CI but fragile for bare Gradle.
        val srcAsset = file("src/main/assets/tauri.conf.json")
        if (srcAsset.exists()) srcAsset.delete()
    }
}

tasks.matching { it.name.contains("lint", ignoreCase = true) }.configureEach {
    dependsOn(stageTauriAssets)
}

rust {
    rootDirRel = "../../../"
}

dependencies {
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    implementation("com.davemorrissey.labs:subsampling-scale-image-view:3.10.0")
    implementation("androidx.viewpager2:viewpager2:1.1.0")
    implementation("androidx.exifinterface:exifinterface:1.4.1")
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.robolectric:robolectric:4.11.1")
    testImplementation("androidx.test:core:1.6.1")
    testImplementation("androidx.test.ext:junit:1.2.1")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

apply(from = "tauri.build.gradle.kts")

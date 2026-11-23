plugins {
    id("com.android.application") version "8.9.2"
    id("org.jetbrains.kotlin.android") version "2.1.20"
}

val webRoot = rootProject.projectDir.parentFile
val nativeScripts = listOf("countdown-format.js", "app-host.js")
val webFiles = listOf("ui-prototype.html", "prototype.css", "animated-texture.js", "reminder-schedule.js") + nativeScripts
val stagedWeb = layout.buildDirectory.dir("generated/countdown-web")
val stageWebAssets by tasks.registering(Sync::class) {
    from(webRoot) { include(webFiles) }
    from(webRoot.resolve("assets")) {
        into("assets")
        exclude("**/screenshots/**", "**/.*")
    }
    into(stagedWeb)
}
val verifyHostedAssets by tasks.registering {
    doLast {
        val missing = webFiles.filterNot { webRoot.resolve(it).isFile }
        check(missing.isEmpty()) { "Missing shared web assets: ${missing.joinToString()}" }
    }
}

val signing = listOf("ANDROID_KEYSTORE_PATH", "ANDROID_KEYSTORE_PASSWORD", "ANDROID_KEY_ALIAS", "ANDROID_KEY_PASSWORD")
    .associateWith { providers.environmentVariable(it).orNull }
val signedRelease = signing.values.any { !it.isNullOrEmpty() }
if (signedRelease) {
    val missing = signing.filterValues { it.isNullOrEmpty() }.keys
    check(missing.isEmpty()) { "Incomplete Android release signing environment; missing: ${missing.joinToString()}" }
}

android {
    namespace = "com.countdowntodeath.app"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.countdowntodeath.app"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "1.0.0"
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    sourceSets.named("main") { assets.srcDir(stagedWeb) }

    if (signedRelease) {
        signingConfigs.create("release") {
            storeFile = file(signing.getValue("ANDROID_KEYSTORE_PATH")!!)
            storePassword = signing.getValue("ANDROID_KEYSTORE_PASSWORD")
            keyAlias = signing.getValue("ANDROID_KEY_ALIAS")
            keyPassword = signing.getValue("ANDROID_KEY_PASSWORD")
        }
        buildTypes.named("release") { signingConfig = signingConfigs.getByName("release") }
    }
}

kotlin { jvmToolchain(17) }

tasks.named("preBuild") { dependsOn(stageWebAssets) }
tasks.matching { it.name == "preReleaseBuild" }.configureEach { dependsOn(verifyHostedAssets) }

dependencies {
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.json:json:20240303")
}

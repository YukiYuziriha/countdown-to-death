# Countdown to Death for Android

The Kotlin app requires Android 8.0 (API 26) or later and uses a local WebView. It does not request Internet access. `window.CountdownAndroid` returns an empty string from `getState()` until the web app saves its first state; the shared host script should treat that as a new install with no profiles. `getStatus()` returns `notifications`, `exactAlarms`, and a timing `statusText`. The activity emits `countdown-host-status-changed` on resume and after the notification permission callback; the web UI can reread status then.

## Build

Install JDK 17, Gradle 8.11.1, Android SDK Platform 35 and Build Tools 35.0.0. Point `JAVA_HOME` and `ANDROID_HOME` to those installations. From `android/`:

```sh
gradle :app:testDebugUnitTest :app:assembleDebug
```

The local debug APK is `app/build/outputs/apk/debug/app-debug.apk`. To support work on the native layer before the root web changes land, debug builds can run against the standalone prototype. For a usable native integration, include the shared `app-host.js` and `countdown-format.js` and load them from `ui-prototype.html`; `:app:verifyHostedAssets` checks the files exist, and CI and release builds require them.

For a signed local release, provide `ANDROID_KEYSTORE_PATH`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, and `ANDROID_KEY_PASSWORD` in the build environment and run `gradle --no-daemon :app:assembleRelease`. The keystore path should refer to a file outside the repository. An unsigned release is not installable as-is, and a debug-signed APK cannot update an APK signed with a different key.

The **Build Android APK** GitHub Actions workflow produces `countdown-to-death-android-debug` for pushes, pull requests and standard manual runs. For a release, dispatch the workflow on a trusted ref with `signed_release` enabled. Configure `ANDROID_KEYSTORE_BASE64` (the base64-encoded keystore bytes), `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, and `ANDROID_KEY_PASSWORD` as repository secrets. The job decodes the key into runner temp storage, verifies the signed APK, and publishes `countdown-to-death-android-release` as a workflow artifact, not a GitHub Release.

## Reminder behavior

- Schedules use the current device time zone. Monthly reminders clamp to the last day of shorter months; daylight-saving gaps shift forward and overlaps select the earlier instant. A scheduled reminder must be strictly before the target. The target notification is separate.
- Grant notification permission and leave the **Countdown reminders** channel enabled to receive updates. On Android 12+, enable **Alarms & reminders** access for precise scheduling. Without it, the app schedules inexact alarms that can arrive late. Late reminders display the remaining time as of the *scheduled* occurrence; missed repetitions are not replayed.
- The scheduler restores future reminders after boot and device clock or time-zone changes. Boot rescheduling happens after the first unlock. Android can delay delivery in idle mode; force-stopping the app suppresses alarms until it is launched again.

## Device smoke test

Install an integrated APK on Android 13 or later. Confirm the first run has no profiles; create a profile with a target and an enabled one-time reminder a few minutes ahead. Switch away and confirm both notifications (with target notification enabled), then check persistence after reopening. Also test denying and granting notification/exact-alarm access, editing or pausing a reminder, and rebooting or changing the time zone before delivery. Use a swipe-away rather than Android's **Force stop** for the background-delivery test.

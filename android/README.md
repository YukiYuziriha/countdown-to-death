# Countdown to Death for Android

The Kotlin app requires Android 8.0 (API 26) or later and uses a local WebView. It does not request Internet access. `window.CountdownAndroid` returns an empty string from `getState()` until the web app saves its first state; the shared host script should treat that as a new install with no profiles. `getStatus()` returns `notifications`, `exactAlarms`, and a `notificationBlock` reason when alerts are disabled. The activity emits `countdown-host-status-changed` on resume and after the notification permission callback; the web UI can reread status then.

## Build

Install JDK 17, Gradle 8.11.1, Android SDK Platform 35 and Build Tools 35.0.0. Point `JAVA_HOME` and `ANDROID_HOME` to those installations. From `android/`:

```sh
gradle :app:testDebugUnitTest :app:assembleDebug
```

The local debug APK is `app/build/outputs/apk/debug/app-debug.apk`. To support work on the native layer before the root web changes land, debug builds can run against the standalone prototype. For a usable native integration, include the shared `app-host.js` and `countdown-format.js` and load them from `ui-prototype.html`; `:app:verifyHostedAssets` checks the files exist, and CI and release builds require them.

For a signed local release, provide `ANDROID_KEYSTORE_PATH`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, and `ANDROID_KEY_PASSWORD` in the build environment and run `gradle --no-daemon :app:assembleRelease`. The keystore path should refer to a file outside the repository. An unsigned release is not installable as-is, and a debug-signed APK cannot update an APK signed with a different key.

The **Build Android APK** GitHub Actions workflow produces `countdown-to-death-android-debug` for pushes, pull requests and standard manual runs. For a release, dispatch the workflow on a trusted ref with `signed_release` enabled. Configure `ANDROID_KEYSTORE_BASE64` (the base64-encoded keystore bytes), `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, and `ANDROID_KEY_PASSWORD` as repository secrets. The job decodes the key into runner temp storage, verifies the signed APK, and publishes `countdown-to-death-android-release` as a workflow artifact, not a GitHub Release.

## Reminder delivery and power

- In **Settings → Notifications**, allow notifications. Android 13+ first shows the standard permission prompt; if blocked at the app or channel level, the button opens the corresponding Android settings. Alarms are suspended when the app detects that notifications are blocked. Return to the app after changing system notification settings so it can resume scheduling.
- **Precise alarms** uses Android’s **Alarms & reminders** special access on Android 12+. When available, the next batch uses `setExactAndAllowWhileIdle()`. Exact alarms can wake the phone and are subject to idle quotas. Without access, a batched inexact `RTC_WAKEUP` alarm is used; Android can defer it, especially in Doze. If access is revoked while an exact alarm is armed, a non-waking inexact backup can recover on a later system wake; it does not wake the phone on its own.
- The native receiver handles all reminders already due in that batch and arms the next one-shot alarm. While waiting, there is no background timer, polling, wake lock, foreground service, or active WebView timer. Only configured reminder occurrences are considered. A delayed message uses the scheduled time; regular reminders are dropped after countdown expiration, and deliveries more than six hours late are dropped. Repeated broadcasts and clock rewinds do not replay an already consumed occurrence.
- Future alarms are restored on reboot **after first unlock**, package replacement, clock/timezone changes, and exact-access grant. The app keeps definitions in credential-protected local storage and does not start the UI for those broadcasts. Monthly dates clamp to month end; local daylight-saving gaps shift forward and overlaps use the earlier offset.
- Powered-off, locked-before-first-unlock, or force-stopped devices cannot reliably deliver alarms. Android and OxygenOS can defer alarms or suppress notifications through Doze, idle quotas, notification settings, or other device policies. No unrestricted battery exemption is requested.

## Notification appearance and Live Alerts

The existing **Countdown reminders** channel controls sound and prominence. New notifications use a monochrome pixel skull, the saved Neon or Spectral accent, a standard compact card, and an optional expanded Android-decorated layout. Expanded text uses the system font and colors for contrast and falls back to standard Big Text at large accessibility font sizes. The lock-screen public version hides the countdown name and remaining time. System UI controls the outer card, colors it may override, and whether space is available to expand it; the notification cannot reproduce the WebView’s animated background.

[Android 16 Live Updates](https://developer.android.com/develop/ui/views/notifications/live-update) are for finite, ongoing, user-initiated and time-sensitive activities that warrant monitoring while they happen. An ordinary countdown to a date, a recurring reminder, and a brief target-time alert do not qualify as implemented here. The app therefore does not declare `POST_PROMOTED_NOTIFICATIONS`, request promotion, or post a `Notification.ProgressStyle` card merely to reach the island. Normal native notifications are the supported path. Eligible future activity would need a supported system template (`ProgressStyle` if appropriate); custom `RemoteViews` are not eligible for promotion.

OnePlus [lists the Nord 3 5G as OxygenOS 16 compatible](https://www.oneplus.com/global/oxygenos16) and [documents rollout variants](https://community.oneplus.com/thread/2198936327796293637). Actual camera-island placement is firmware- and settings-dependent. There is no confirmed public OnePlus-specific placement or styling API for this app. No island promotion was verified on a physical Nord 3, and these ordinary reminders should not be expected to appear in it.

The in-app pixel skull follows Neon or Spectral. The Android launcher uses a Neon-colored adaptive icon by default, with an Android 13+ monochrome layer for system themed icons. Launcher tint is controlled by Android, not by the in-app theme selector.

## Physical-device smoke test (ADB)

Use an APK built from these sources and a USB-debugging-enabled test phone. Use `adb shell dumpsys alarm` and `adb shell dumpsys notification` to inspect state; filter the output for `com.countdowntodeath.app` with `grep` on Unix or `findstr` on Windows.

1. In Settings, grant both notification and exact-alarm access. Schedule two reminders for the same time several minutes ahead; expect one wake-up batch and two separate notifications. Confirm the locked-screen redaction and expand both Neon and Spectral notifications. Try a large system font size to check the Big Text fallback.
2. Press Home or swipe the app out of Recents. `adb shell am kill com.countdowntodeath.app` can kill a cached process without force-stopping it. Do not use `am force-stop` for this test: that intentionally suppresses alarms until manual launch. Inspect scheduled alarms and notification delivery without reopening the app.
3. For Doze, unplug the device, turn off its display, and run `adb shell dumpsys battery unplug` and `adb shell dumpsys deviceidle force-idle`. Compare separate exact and inexact trials spaced far enough apart to avoid Android’s while-idle quota. Restore with `adb shell dumpsys deviceidle unforce` and `adb shell dumpsys battery reset` afterward.
4. Revoke notification permission (`adb shell pm revoke com.countdowntodeath.app android.permission.POST_NOTIFICATIONS` on Android 13+) and verify the app explains why no alerts appear. Regrant in the standard dialog. Revoke exact access in **Alarms & reminders** with a future alarm armed; inspect the non-waking backup, then regrant and check that the exact alarm returns. Verify edits, pause, and deletion remove the old pending occurrence.
5. Schedule future occurrences, `adb reboot`, unlock the phone once, and inspect alarms without opening the app. Repeat with `adb install -r` using a compatible, same-signature APK to test `MY_PACKAGE_REPLACED`. Change the device timezone before delivery and confirm the reminder follows the new local time without replaying a stale alert. Check that countdown completion stops recurring reminders.
6. On the Nord 3, record OxygenOS build/region and inspect the notification shade and camera island independently. Ordinary reminders are expected in the shade. Testing *promotion* requires a separate genuinely eligible Android Live Update; the settings toggle or the island's presence alone is not proof that this app can be promoted.
